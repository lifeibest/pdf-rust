//! 微信公众号文章导出 PDF 的专用命令。
//!
//! 与其它功能不同，本流程**不走 [`super::tasks`] 的 `TaskRequest` 管线**：
//! 打印必须由内嵌 WebView 在 UI 线程发起（`with_webview` / `eval_with_callback`），
//! 产物由操作系统生成，因此这里以专用命令编排，但仍复用 [`AppState`] 的
//! 任务登记与取消令牌，让任务中心、进度事件与取消机制保持一致。

use std::path::PathBuf;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppError;
use crate::events::{self, FailedPayload, FinishedPayload, ProgressPayload};
use crate::pdf::wechat::{self, WechatExportOptions};
use crate::pdf::OutputFile;
use crate::state::{AppState, TaskRecord};

/// 导出微信公众号文章为 PDF。
///
/// `task_id` 由前端生成并传入，便于在返回前即可通过 [`cancel_wechat_export`] 取消。
/// 任务在阻塞线程中运行，期间通过 `task:progress` 上报阶段文案；
/// 结束时发送 `task:finished` / `task:failed`，并把最终记录作为返回值。
#[tauri::command]
pub async fn export_wechat_pdf(
    app: AppHandle,
    task_id: String,
    options: WechatExportOptions,
    out_dir: Option<String>,
) -> Result<TaskRecord, AppError> {
    // 配额门控：与 run_task 一致，未激活且超额时直接返回 QuotaExceeded。
    crate::commands::license::gate(&app)?;

    let state = app.state::<AppState>();
    let cancel = state.start_task(&task_id, "wechat");
    let confirm = state
        .confirm_flag(&task_id)
        .ok_or_else(|| AppError::Internal("任务状态初始化失败".to_string()))?;
    state.set_progress(&task_id, 0, 0, "任务已启动");

    let output_dir = resolve_output_dir(&app, out_dir.as_deref())?;

    let worker = app.clone();
    let worker_task_id = task_id.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let emitter = worker.clone();
        let progress_id = worker_task_id.clone();
        let mut progress = move |current: usize, total: usize, message: &str| {
            emitter
                .state::<AppState>()
                .set_progress(&progress_id, current, total, message);
            let _ = emitter.emit(
                events::TASK_PROGRESS,
                ProgressPayload {
                    task_id: progress_id.clone(),
                    feature: "wechat".to_string(),
                    current,
                    total,
                    message: message.to_string(),
                },
            );
        };

        let started = Instant::now();
        let outputs = wechat::run(&worker, &output_dir, &options, &cancel, &confirm, &mut progress);
        let elapsed_ms = started.elapsed().as_millis() as u64;

        finalize(&worker, &worker_task_id, outputs, elapsed_ms)
    })
    .await;

    match result {
        Ok(inner) => inner,
        // 阻塞线程 panic 时不会走到 finalize，补一条失败记录，避免任务永远停在「进行中」
        Err(_) => {
            let error = AppError::Internal("导出线程异常退出".to_string());
            let _ = app.state::<AppState>().fail_task(&task_id, &error, 0);
            let _ = app.emit(
                events::TASK_FAILED,
                FailedPayload {
                    task_id: task_id.clone(),
                    feature: "wechat".to_string(),
                    error: error.clone(),
                    elapsed_ms: 0,
                },
            );
            Err(error)
        }
    }
}

/// 请求取消正在运行的导出。命中取消令牌后，[`wechat::run`] 的轮询会退出并关闭隐藏窗口。
/// 任务已结束或不存在时返回 false。
#[tauri::command]
pub fn cancel_wechat_export(app: AppHandle, task_id: String) -> bool {
    app.state::<AppState>().cancel_task(&task_id)
}

/// 手动模式下确认「页面已就绪，继续导出」。置位后 [`wechat::run`] 的等待循环退出并进入打印。
/// 任务不存在或已结束时返回 false。
#[tauri::command]
pub fn confirm_wechat_export(app: AppHandle, task_id: String) -> bool {
    app.state::<AppState>().confirm_task(&task_id)
}

/// 写入终态记录并广播 `task:finished` / `task:failed`。
fn finalize(
    app: &AppHandle,
    task_id: &str,
    outputs: Result<Vec<OutputFile>, AppError>,
    elapsed_ms: u64,
) -> Result<TaskRecord, AppError> {
    let state = app.state::<AppState>();

    match outputs {
        Ok(files) => {
            let record = state
                .finish_task(task_id, files.clone(), elapsed_ms, None)
                .ok_or_else(|| AppError::Internal("任务记录已丢失".to_string()))?;

            let _ = app.emit(
                events::TASK_FINISHED,
                FinishedPayload {
                    task_id: task_id.to_string(),
                    feature: "wechat".to_string(),
                    outputs: files,
                    elapsed_ms,
                    summary: None,
                },
            );

            Ok(record)
        }
        Err(error) => {
            state.fail_task(task_id, &error, elapsed_ms);
            let _ = app.emit(
                events::TASK_FAILED,
                FailedPayload {
                    task_id: task_id.to_string(),
                    feature: "wechat".to_string(),
                    error: error.clone(),
                    elapsed_ms,
                },
            );
            log::warn!("微信导出 {task_id} 失败: {}", error.brief());
            Err(error)
        }
    }
}

/// 解析输出目录：显式指定时优先，否则退回「下载目录/应用名」。
fn resolve_output_dir(app: &AppHandle, out_dir: Option<&str>) -> Result<PathBuf, AppError> {
    let target = match out_dir.map(str::trim).filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => super::tasks::default_output_dir(app)?,
    };

    std::fs::create_dir_all(&target)?;
    Ok(target)
}
