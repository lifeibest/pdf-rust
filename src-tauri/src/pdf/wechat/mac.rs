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

/// 裁剪正文时左右各保留的目标白边（CSS 像素）。
/// 实际留白取 `PAGE_PAD.min(left)`，保证即使 `left < PAGE_PAD` 时左右白边也严格相等、绝不偏左。
const PAGE_PAD: f64 = 50.0;

/// 计算对称裁剪后的页矩形 `(x, rect_width)`：
/// 左白边 `left - x` 与右白边 `(x + rect_width) - (left + width)` 恒等于 `pad`。
fn symmetric_crop(left: f64, width: f64) -> (f64, f64) {
    let safe_left = left.max(0.0);
    let pad = PAGE_PAD.min(safe_left);
    let x = safe_left - pad;
    let rect_width = width + pad * 2.0;
    (x, rect_width)
}

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
                    let (x, rect_width) = symmetric_crop(left, width);
                    config.setRect(CGRect::new(
                        CGPoint::new(x, 0.0),
                        CGSize::new(rect_width, height),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_crop_keeps_left_and_right_margins_equal() {
        for (left, width) in [(0.0, 680.0), (20.0, 637.0), (50.0, 680.0), (160.0, 680.0)] {
            let (x, rect_width) = symmetric_crop(left, width);
            let left_margin = left - x;
            let right_margin = (x + rect_width) - (left + width);
            assert!(x >= 0.0);
            assert!(
                (left_margin - right_margin).abs() < 1e-6,
                "left={left}, width={width} -> left_margin={left_margin}, right_margin={right_margin}"
            );
        }
    }
}
