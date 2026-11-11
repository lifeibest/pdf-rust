//! 微信公众号文章导出 PDF（方案 A：WebView 原生打印）。
//!
//! 纯客户端流程：用一个隐藏的 [`WebviewWindow`] 加载 `mp.weixin.qq.com` 文章 →
//! 注入 JS 触发图片懒加载（微信正文图片是 `data-src`，且 `mmbiz.qpic.cn` 有防盗链，
//! 在 WebView 内加载天然带正确 Referer）→ 调平台原生「打印为 PDF」→ 移动到输出目录。
//!
//! 与其它 `pdf/*` 模块不同，本功能**不走 `run_task` 的 `spawn_blocking` 管线**：
//! 打印必须由内嵌 WebView 在 UI 线程发起（`with_webview`），产物由操作系统生成，
//! 因此这里以「事件轮询 + 原生回调」编排，PDF 引擎只在最后统计页数时参与。
//!
//! 平台差异：
//! - Windows：`ICoreWebView2_7::PrintToPdf` 直接落盘，输出**分页 A4 矢量 PDF**。
//! - macOS：`WKWebView createPDFWithConfiguration:completionHandler:` 返回 NSData，
//!   写出**单页超长矢量 PDF**（原生不分页；产品已确认接受此形态）。
//!
//! 就绪判定只用 host→page 的 [`WebviewWindow::eval_with_callback`]（不回传远程页 IPC），
//! 因此隐藏窗口无需被授予任何 capability 权限，远程脚本无法调用本应用命令。

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use tauri::{AppHandle, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::error::AppError;
use crate::pdf::{info, resolve_output_path, unique_temp_path, CancelToken, OutputFile};
use crate::state::ConfirmFlag;

#[cfg(target_os = "macos")]
mod mac;
#[cfg(windows)]
mod win;

/// 正文渲染宽度（CSS 像素），加载前的初始视口宽；真正打印前会按正文列实测宽修正。
const CONTENT_WIDTH: f64 = 677.0;
/// 初始视口高度，加载后按内容自然增长。
const CONTENT_HEIGHT: f64 = 1200.0;
/// 实测正文列宽的下限，低于此视为无效（避免把页矩形锁成 0 或异常窄）。
/// 不设上限：macOS 隐藏窗口的 bounds 宽度不受控，正文列可能很宽，按实测值裁剪即可。
const CONTENT_MIN_WIDTH: f64 = 200.0;

/// 单次 `eval_with_callback` 的最长等待。
const EVAL_WAIT: Duration = Duration::from_millis(1_500);
/// 轮询间隔。
const POLL_INTERVAL: Duration = Duration::from_millis(150);
/// 等待文档 `readyState == complete` 的上限。
const READY_DEADLINE: Duration = Duration::from_secs(12);
/// 等待所有图片加载完成的上限。
const IMAGE_DEADLINE: Duration = Duration::from_secs(8);
/// 图片就绪后的稳定化等待，留给异步渲染与字体。
const SETTLE: Duration = Duration::from_millis(600);
/// 原生打印完成的上限（含大文档渲染）。
const PRINT_DEADLINE: Duration = Duration::from_secs(30);
/// 手动模式下等待用户完成验证 / 确认继续的上限（留足人工操作时间）。
const MANUAL_DEADLINE: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WechatExportOptions {
    /// 公众号文章链接，必须以 `https://mp.weixin.qq.com/` 开头
    pub url: String,
    /// 自定义输出文件名（含 .pdf）；为空时按时间戳命名
    pub output_name: Option<String>,
    /// 手动模式：显示文章窗口供用户完成验证 / 等待加载，由用户确认后再导出。
    /// 适用于触发微信环境校验、反爬或网络较慢的文章。
    #[serde(default)]
    pub manual: bool,
}

/// 校验并归一化文章 URL，拒绝非微信域名。
fn validate_url(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid("请输入微信公众号文章链接"));
    }
    let url = trimmed
        .parse::<tauri::Url>()
        .map_err(|_| AppError::invalid("链接格式不合法"))?;
    if url.scheme() != "https"
        || url.host_str().unwrap_or_default() != "mp.weixin.qq.com"
    {
        return Err(AppError::invalid("仅支持微信公众号文章链接（mp.weixin.qq.com）"));
    }
    Ok(trimmed.to_string())
}

/// 默认输出名：`wechat_<yyyyMMdd_HHmmss>.pdf`（读取远程 `<title>` 需额外通道，成本高，v1 用时间戳）。
fn fallback_name() -> String {
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    format!("wechat_{stamp}.pdf")
}

/// 触发图片懒加载并滚动全文的注入脚本：把 `data-src` 提到 `src`，再分步滚到底。
fn force_load_js() -> &'static str {
    r#"(function(){
        try{
            var imgs = Array.prototype.slice.call(document.querySelectorAll('img[data-src]'));
            imgs.forEach(function(i){
                var d = i.getAttribute('data-src');
                if(d && (!i.src || i.src.indexOf('data:') === 0)){ try{ i.src = d; }catch(e){} }
            });
            var step = window.innerHeight || 1000, y = 0, h = document.body.scrollHeight;
            var tick = function(){ window.scrollTo(0, y); y += step; if(y < h){ setTimeout(tick, 60); } else { window.scrollTo(0, 0); } };
            tick();
            return 'ok';
        }catch(e){ return String(e); }
    })()"#
}

/// 统计尚未加载完成的图片数量；无正文（`#js_content` 缺失）时返回 -1，供上层提示异常页。
fn pending_images_js() -> &'static str {
    r#"(function(){
        try{
            var box = document.getElementById('js_content');
            if(!box){ return -1; }
            var imgs = Array.prototype.slice.call(box.querySelectorAll('img'));
            var pending = imgs.filter(function(i){ return !i.complete || i.naturalWidth === 0; }).length;
            return pending;
        }catch(e){ return -2; }
    })()"#
}

/// 量取正文的**实际可见内容**水平范围与整页高。微信桌面版常把 `#js_content` 渲染成
/// 铺满视口的宽块、内容却左对齐，若直接量容器会得到视口宽（等于没裁剪）。
/// 故这里量标题文本 + 正文文本 + 所有图片的并集矩形（跳过近乎满宽的容器），
/// 得到真正的内容左右边界。返回 `"left,width,height"`（四舍五入整数）；失败返回 `"0,0,0"`。
fn content_size_js() -> &'static str {
    r#"(function(){
        try{
            var vw = document.documentElement.clientWidth || window.innerWidth || 0;
            var minL = Infinity, maxR = -Infinity;
            var add = function(r){
                if(!r || r.width < 2 || r.height < 2) return;
                if(vw && r.width > vw * 0.95) return;
                if(r.left < minL) minL = r.left;
                if(r.right > maxR) maxR = r.right;
            };
            var textRect = function(el){
                if(!el) return null;
                try{ var rng = document.createRange(); rng.selectNodeContents(el); return rng.getBoundingClientRect(); }catch(e){ return null; }
            };
            var box = document.querySelector('#js_content') || document.querySelector('.rich_media_content') || document.querySelector('#content');
            var title = document.querySelector('#activity-name') || document.querySelector('.rich_media_title');
            add(textRect(title));
            add(title && title.getBoundingClientRect());
            add(textRect(box));
            var imgs = document.querySelectorAll('#js_content img, .rich_media_content img');
            for(var i=0;i<imgs.length;i++){ add(imgs[i].getBoundingClientRect()); }
            if(minL === Infinity || maxR === -Infinity){
                if(box){ var br = box.getBoundingClientRect(); minL = br.left; maxR = br.right; }
                else { minL = 0; maxR = vw; }
            }
            var left = Math.round(minL), width = Math.round(maxR - minL);
            var h = Math.max(document.body.scrollHeight || 0, document.documentElement.scrollHeight || 0, 1);
            return left + ',' + width + ',' + h;
        }catch(e){ return '0,0,0'; }
    })()"#
}

/// 打印前的版式归一化：裁掉横向溢出、等比缩小超宽媒体元素，避免个别元素撑破正文列
/// 导致按正文宽裁剪时右侧内容被切。
fn normalize_layout_js() -> &'static str {
    r#"(function(){
        try{
            var css = [
                'html,body{overflow-x:hidden!important;}',
                'img,video,iframe,svg,figure,section,pre{max-width:100%!important;}',
                'img{height:auto!important;}'
            ].join('');
            var old = document.getElementById('pdfai-print-normalize');
            if(old && old.parentNode){ old.parentNode.removeChild(old); }
            var s = document.createElement('style');
            s.id = 'pdfai-print-normalize';
            s.textContent = css;
            (document.head || document.documentElement).appendChild(s);
            return 'ok';
        }catch(e){ return String(e); }
    })()"#
}

/// 解析正文打印区域字符串 `"left,width,height"`。
/// `eval_with_callback` 回传的是 JSON 字符串（外层带引号），先剥掉非数字字符再按逗号切分；
/// 宽度低于下限或高度无效时返回 `None`（调用方回退用 bounds）。
fn parse_content_size(raw: &str) -> Option<(f64, f64, f64)> {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == ',')
        .collect();
    let mut parts = cleaned.split(',');
    let left: f64 = parts.next()?.trim().parse().ok()?;
    let width: f64 = parts.next()?.trim().parse().ok()?;
    let height: f64 = parts.next()?.trim().parse().ok()?;
    if width >= CONTENT_MIN_WIDTH && height >= 1.0 {
        Some((left.max(0.0), width, height))
    } else {
        None
    }
}

/// 量取正文打印区域 `(left, width, height)`；测不到有效区域时返回 `None`。
fn measure_content_size(win: &WebviewWindow) -> Option<(f64, f64, f64)> {
    let raw = eval_once(win, content_size_js())?;
    parse_content_size(&raw)
}

/// 执行一段 JS 并等待其返回值（JSON 字符串）。超时或出错返回 `None`，调用方自行重试。
fn eval_once(win: &WebviewWindow, js: &str) -> Option<String> {
    let (tx, rx) = mpsc::channel::<String>();
    win.eval_with_callback(js.to_string(), move |result| {
        let _ = tx.send(result);
    })
    .ok()?;
    rx.recv_timeout(EVAL_WAIT).ok()
}

fn check_cancel(cancel: &CancelToken) -> Result<(), AppError> {
    if cancel.is_cancelled() {
        Err(AppError::Cancelled)
    } else {
        Ok(())
    }
}

/// 手动模式：轮询等待用户点击「继续导出」（置位 `confirm`），或取消 / 超时退出。
fn wait_manual_confirm(cancel: &CancelToken, confirm: &ConfirmFlag) -> Result<(), AppError> {
    let deadline = Instant::now() + MANUAL_DEADLINE;
    while Instant::now() < deadline {
        check_cancel(cancel)?;
        if confirm.load(Ordering::SeqCst) {
            return Ok(());
        }
        sleep(POLL_INTERVAL);
    }
    Err(AppError::Internal(
        "等待手动确认超时，请重新导出".to_string(),
    ))
}

/// 轮询直到文档加载完成，或超过 `READY_DEADLINE`（超时不视为致命，继续尝试打印）。
fn wait_document_ready(win: &WebviewWindow, cancel: &CancelToken) -> Result<(), AppError> {
    let deadline = Instant::now() + READY_DEADLINE;
    while Instant::now() < deadline {
        check_cancel(cancel)?;
        if let Some(value) = eval_once(win, "document.readyState")
            && value.contains("complete")
        {
            return Ok(());
        }
        sleep(POLL_INTERVAL);
    }
    Ok(())
}

/// 触发懒加载并等待图片加载完成。正文缺失时：自动模式给出可读错误（多为验证页 / 需登录 / 已删除）；
/// 手动模式由用户已确认页面就绪，即使无 `#js_content` 也按“导出即所见”继续。
fn load_images(win: &WebviewWindow, cancel: &CancelToken, manual: bool) -> Result<(), AppError> {
    let _ = win.eval(force_load_js());
    sleep(POLL_INTERVAL);

    let deadline = Instant::now() + IMAGE_DEADLINE;
    while Instant::now() < deadline {
        check_cancel(cancel)?;
        if let Some(value) = eval_once(win, pending_images_js()) {
            let digits: String = value.chars().filter(|c| c.is_ascii_digit() || *c == '-').collect();
            match digits.parse::<i64>() {
                Ok(-1) => {
                    if manual {
                        // 用户已确认页面可导出，不再因缺少 `#js_content` 而硬失败
                        break;
                    }
                    return Err(AppError::Unsupported(
                        "未能读取到文章正文，可能是需登录 / 付费 / 已删除或环境校验页面，可改用手动模式重试".to_string(),
                    ));
                }
                Ok(0) => break,
                _ => {}
            }
        }
        sleep(POLL_INTERVAL);
    }

    // 图片基本就绪后再做一次全文滚动，兜底触发视口外懒加载，然后稳定化
    let _ = win.eval(force_load_js());
    sleep(SETTLE);
    Ok(())
}

/// 把产物从缓存目录移动到输出目录（跨卷时 `rename` 会失败，退回复制 + 删除）。
fn relocate(src: &Path, dst: &Path) -> Result<(), AppError> {
    std::fs::rename(src, dst).or_else(|_| -> Result<(), AppError> {
        std::fs::copy(src, dst)?;
        std::fs::remove_file(src)?;
        Ok(())
    })
}

/// 在指定平台把当前 WebView 内容打印为 PDF 写入 `out_path`。
/// `size` 为正文打印区域 `(left, width, height)`：macOS 用它把 PDF 页矩形锁成正文列，
/// 消除左右空白；Windows 由 `PrintToPdf` 自行按 A4 排版，忽略此参数。
fn platform_print(
    win: &WebviewWindow,
    out_path: &Path,
    cancel: &CancelToken,
    size: Option<(f64, f64, f64)>,
) -> Result<(), AppError> {
    #[cfg(target_os = "macos")]
    {
        mac::print(win, out_path, cancel, size)
    }
    #[cfg(windows)]
    {
        let _ = size;
        win::print(win, out_path, cancel)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (win, out_path, cancel, size);
        Err(AppError::Unsupported("当前平台暂不支持导出微信文章".to_string()))
    }
}

/// 导出微信文章为 PDF。在阻塞线程中运行，`progress` 用于上报阶段文案，
/// `confirm` 为手动模式下的「继续导出」信号。
pub fn run(
    app: &AppHandle,
    out_dir: &Path,
    opts: &WechatExportOptions,
    cancel: &CancelToken,
    confirm: &ConfirmFlag,
    progress: &mut dyn FnMut(usize, usize, &str),
) -> Result<Vec<OutputFile>, AppError> {
    let url = validate_url(&opts.url)?;

    let cache_dir = crate::commands::pdf::cache_dir(app)?;
    std::fs::create_dir_all(&cache_dir)?;

    // 1. 加载文章。窗口标签不进任何 capability，远程页零 IPC 权限。
    //    自动模式用隐藏窗口；手动模式显示窗口，供用户完成验证 / 等待加载。
    let label = format!("wechat-export-{}", uuid::Uuid::new_v4().simple());
    progress(0, 0, "正在打开文章页面");
    let external = WebviewUrl::External(url.parse().map_err(|_| AppError::invalid("链接格式不合法"))?);
    let builder = WebviewWindowBuilder::new(app, &label, external)
        .inner_size(CONTENT_WIDTH, CONTENT_HEIGHT)
        .decorations(false)
        .focused(opts.manual);
    let window = if opts.manual {
        builder.visible(true).center().build()
    } else {
        builder.visible(false).build()
    }
    .map_err(|err| AppError::Internal(format!("无法创建文章窗口：{err}")))?;

    let result = export_in_window(&window, &cache_dir, out_dir, opts, cancel, confirm, progress);

    let _ = window.close();
    result
}

fn export_in_window(
    window: &WebviewWindow,
    cache_dir: &Path,
    out_dir: &Path,
    opts: &WechatExportOptions,
    cancel: &CancelToken,
    confirm: &ConfirmFlag,
    progress: &mut dyn FnMut(usize, usize, &str),
) -> Result<Vec<OutputFile>, AppError> {
    check_cancel(cancel)?;

    // 手动模式：先把窗口交给用户，直到其确认页面已就绪再继续。
    if opts.manual {
        progress(0, 0, "请在打开的窗口中完成验证或等待加载，然后点「继续导出」");
        wait_manual_confirm(cancel, confirm)?;
    }

    wait_document_ready(window, cancel)?;

    progress(0, 0, "正在加载正文图片");
    load_images(window, cancel, opts.manual)?;

    check_cancel(cancel)?;
    // 归一化版式：裁掉横向溢出、缩放超宽媒体，避免个别元素撑破正文列；短暂稳定化
    let _ = window.eval(normalize_layout_js());
    sleep(SETTLE);

    // 量取正文打印区域，供 macOS 把 PDF 页矩形锁成正文列（消除左右空白）
    let size = measure_content_size(window);

    progress(0, 0, "正在生成 PDF");
    let temp_pdf: PathBuf = unique_temp_path(cache_dir, "wechat", "pdf");
    platform_print(window, &temp_pdf, cancel, size)?;

    if !temp_pdf.exists() {
        return Err(AppError::Internal("打印未生成 PDF 文件".to_string()));
    }

    // 统计页数（Windows 分页后 > 1；macOS 单页超长 = 1）。失败不致命。
    let pages = info::probe(&temp_pdf, None)
        .map(|doc| doc.page_count)
        .unwrap_or(1);

    progress(1, 1, "正在保存文件");
    let final_path = resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback_name());
    relocate(&temp_pdf, &final_path)?;

    let output = OutputFile::pdf(&final_path, Some(pages))?;
    Ok(vec![output])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_non_wechat_and_empty() {
        assert!(validate_url("").is_err());
        assert!(validate_url("not a url").is_err());
        assert!(validate_url("https://example.com/s/abc").is_err());
        assert!(validate_url("http://mp.weixin.qq.com/s/abc").is_err());
    }

    #[test]
    fn validate_accepts_wechat_https() {
        assert!(validate_url("https://mp.weixin.qq.com/s/ABCDEFG").is_ok());
        assert!(validate_url("  https://mp.weixin.qq.com/s/xyz?src=1  ").is_ok());
    }

    #[test]
    fn pending_js_extracts_integer_token() {
        // 模拟 eval_with_callback 返回 JSON 字符串的解析口径
        for (raw, expect) in [("\"0\"", 0i64), ("\"12\"", 12), ("\"-1\"", -1), ("\"-2\"", -2)] {
            let digits: String = raw.chars().filter(|c| c.is_ascii_digit() || *c == '-').collect();
            assert_eq!(digits.parse::<i64>().unwrap(), expect, "raw={raw}");
        }
    }

    #[test]
    fn fallback_name_has_pdf_suffix() {
        assert!(fallback_name().ends_with(".pdf"));
        assert!(fallback_name().starts_with("wechat_"));
    }

    #[test]
    fn force_load_js_contains_lazy_swap() {
        let js = force_load_js();
        assert!(js.contains("data-src"));
        assert!(js.contains("scrollTo"));
    }

    #[test]
    fn normalize_js_clamps_overflow_and_media() {
        let js = normalize_layout_js();
        assert!(js.contains("overflow-x:hidden"));
        assert!(js.contains("max-width:100%"));
    }

    #[test]
    fn content_size_js_targets_wechat_container() {
        let js = content_size_js();
        assert!(js.contains("#js_content"));
        assert!(js.contains("getBoundingClientRect"));
        assert!(js.contains("scrollHeight"));
        // 量的是真实内容（标题文本 + 图片），而非可能铺满视口的容器
        assert!(js.contains("createRange"));
        assert!(js.contains("#activity-name"));
    }

    #[test]
    fn parse_content_size_strips_json_quotes() {
        // eval_with_callback 回传 JSON 字符串，外层带引号；宽正文（>1200）也必须能解析
        assert_eq!(
            parse_content_size("\"90,1480,5230\""),
            Some((90.0, 1480.0, 5230.0))
        );
        // 宽度过窄视为无效（回退用 bounds）
        assert_eq!(parse_content_size("\"0,120,500\""), None);
        // JS 异常回退 "0,0,0"
        assert_eq!(parse_content_size("\"0,0,0\""), None);
        // 完全无法解析
        assert_eq!(parse_content_size("garbage"), None);
    }

    #[test]
    fn options_manual_defaults_to_false() {
        // 旧前端不传 manual 时应回退为自动模式（隐藏窗口）
        let opts: WechatExportOptions =
            serde_json::from_str(r#"{"url":"https://mp.weixin.qq.com/s/x","outputName":null}"#)
                .expect("应当解析成功");
        assert!(!opts.manual);
    }
}
