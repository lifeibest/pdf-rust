//! Windows：`ICoreWebView2_7::PrintToPdf` 直接落盘，输出分页 A4 矢量 PDF。
//!
//! 通过 [`WebviewWindow::with_webview`] 在 UI 线程拿到 `PlatformWebview::controller()`
//! （`ICoreWebView2Controller`），取 `CoreWebView2()` → `cast::<ICoreWebView2_7>()`，
//! 调 `PrintToPdf` 把当前页面打印到指定路径。完成回调 `PrintToPdfCompletedHandler`
//! 在 UI 线程异步触发，经 `mpsc` 通知命令线程；期间轮询取消与超时。

use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Instant;

use tauri::WebviewWindow;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2PrintSettings, ICoreWebView2_7,
};
use webview2_com::PrintToPdfCompletedHandler;
use windows::core::{Interface, PCWSTR};

use super::{check_cancel, PRINT_DEADLINE, POLL_INTERVAL};
use crate::error::AppError;
use crate::pdf::CancelToken;

pub fn print(win: &WebviewWindow, out_path: &Path, cancel: &CancelToken) -> Result<(), AppError> {
    let path_buf = out_path.to_path_buf();
    let (tx, rx) = mpsc::channel::<Result<(), String>>();

    win.with_webview(move |platform| {
        unsafe {
            let controller = platform.controller();

            let core = match controller.CoreWebView2() {
                Ok(core) => core,
                Err(err) => {
                    let _ = tx.send(Err(format!("获取 CoreWebView2 失败：{err}")));
                    return;
                }
            };

            // PrintToPdf 属于 ICoreWebView2_7，旧版运行时可能 cast 失败
            let webview7: ICoreWebView2_7 = match core.cast() {
                Ok(value) => value,
                Err(err) => {
                    let _ = tx.send(Err(format!(
                        "当前 WebView2 运行库版本过低，不支持导出（请更新 Edge WebView2 Runtime）：{err}"
                    )));
                    return;
                }
            };

            // PCWSTR 指向的缓冲区须在 PrintToPdf 同步读取期间保持存活，故在作用域内构造
            let wide: Vec<u16> = path_buf
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            // 完成回调会 move 捕获一个 Sender；克隆一份给它，原 `tx` 留给 PrintToPdf 同步失败分支
            let tx_done = tx.clone();
            let handler = PrintToPdfCompletedHandler::create(Box::new(
                move |result: windows::core::Result<()>, success: bool| {
                    let message = match (result.is_ok(), success) {
                        (true, true) => Ok(()),
                        (true, false) => Err("生成 PDF 失败".to_string()),
                        (false, _) => Err("生成 PDF 出错".to_string()),
                    };
                    let _ = tx_done.send(message);
                    Ok(())
                },
            ));

            if let Err(err) = webview7.PrintToPdf(
                PCWSTR::from_raw(wide.as_ptr()),
                // Param 仅对 `Option<&T>` 实现，这里传 `None` 表示不设自定义打印设置
                None::<&ICoreWebView2PrintSettings>,
                &handler,
            ) {
                let _ = tx.send(Err(format!("调用 PrintToPdf 失败：{err}")));
            }
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
