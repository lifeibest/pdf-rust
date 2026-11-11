//! macOS：`WKWebView createPDFWithConfiguration:completionHandler:` 打印为 PDF。
//!
//! 通过 [`WebviewWindow::with_webview`] 在主线程拿到 `PlatformWebview::inner()`
//! （WKWebView 指针），调原生 `createPDF`。completionHandler 在 WebKit 内部线程异步回调，
//! 拿到 `NSData` 后落盘，再经 `mpsc` 通知命令线程。
//!
//! 页矩形开关：`size` 有值时用 [`WKPDFConfiguration::setRect`] 把 PDF 页矩形显式锁成
//! 正文列 `(left, width, height)`，裁掉隐藏窗口那不受控的宽 bounds 带来的左右空白；
//! `size` 为 `None` 时回退传 `None`，由 WebKit 用当前 bounds 决定页宽。
//! 长文仍得到一个单页超长矢量 PDF（产品已确认接受）。

use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Instant;

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_core_foundation::{CGRect, CGPoint, CGSize};
use objc2_foundation::{NSData, NSError};
use objc2_web_kit::{WKPDFConfiguration, WKWebView};
use tauri::WebviewWindow;

use super::{check_cancel, PRINT_DEADLINE, POLL_INTERVAL};
use crate::error::AppError;
use crate::pdf::CancelToken;

/// 裁剪正文时左右各保留的固定白边（CSS 像素）。用固定值而非比例，保证不同宽度的
/// 文章边距一致、左右对称（比例留白在正文贴边时会被 `max(0.0)` 截断导致左右不对称）。
const PAGE_PAD: f64 = 50.0;

pub fn print(
    win: &WebviewWindow,
    out_path: &Path,
    cancel: &CancelToken,
    size: Option<(f64, f64, f64)>,
) -> Result<(), AppError> {
    let path_buf = out_path.to_path_buf();
    let (tx, rx) = mpsc::channel::<Result<(), String>>();

    win.with_webview(move |platform| {
        unsafe {
            let ns_view = platform.inner();
            let webview: &WKWebView = &*(ns_view as *const WKWebView);
            let block = RcBlock::new(move |data: *mut NSData, err: *mut NSError| {
                if !err.is_null() {
                    let _ = tx.send(Err("createPDF 返回错误".to_string()));
                    return;
                }
                if data.is_null() {
                    let _ = tx.send(Err("createPDF 返回空数据".to_string()));
                    return;
                }
                let bytes = (&*data).to_vec();
                let _ = tx.send(std::fs::write(&path_buf, bytes).map_err(|e| e.to_string()));
            });
            // 有实测正文区域时，把 PDF 页矩形锁成正文列（左右各留 PAGE_PAD 白边），裁掉
            // 隐藏窗口那不受控的宽 bounds 带来的空白；否则回退 bounds。
            let cfg = size.and_then(|(left, width, height)| {
                MainThreadMarker::new().map(|mtm| {
                    let config = WKPDFConfiguration::new(mtm);
                    let x = (left - PAGE_PAD).max(0.0);
                    let right = left + width + PAGE_PAD;
                    config.setRect(CGRect::new(
                        CGPoint::new(x, 0.0),
                        CGSize::new(right - x, height),
                    ));
                    config
                })
            });
            webview.createPDFWithConfiguration_completionHandler(cfg.as_deref(), &block);
        }
    })
    .map_err(|err| AppError::Internal(format!("with_webview 失败：{err}")))?;

    wait_for_print(rx, cancel)
}

fn wait_for_print(
    rx: mpsc::Receiver<Result<(), String>>,
    cancel: &CancelToken,
) -> Result<(), AppError> {
    let deadline = Instant::now() + PRINT_DEADLINE;
    loop {
        check_cancel(cancel)?;
        match rx.recv_timeout(POLL_INTERVAL) {
            Ok(result) => return result.map_err(AppError::Internal),
            Err(RecvTimeoutError::Timeout) => {
                if Instant::now() >= deadline {
                    return Err(AppError::Internal("生成 PDF 超时".to_string()));
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(AppError::Internal("生成 PDF 回调已中断".to_string()));
            }
        }
    }
}
