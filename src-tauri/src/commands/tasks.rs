//! 统一任务入口：五大功能的调度、进度事件、取消与任务中心。
//!
//! ★ 扩展点：新增一个功能只需要
//! 1. 新建 `pdf/<op>.rs` 并实现 `run(ctx, out_dir, opts, progress)`；
//! 2. 在 [`TaskRequest`] 中加一个变体；
//! 3. 在 [`dispatch`] 中加一个 match 分支。
//!
//! 前端则新增 `features/<id>/meta.ts` 并在 `registry.ts` 注册一行。

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::error::AppError;
use crate::events::{self, FailedPayload, FinishedPayload, ProgressPayload};
use crate::pdf::export_images::{self, ExportImagesOptions};
use crate::pdf::images_to_pdf::{self, ImagesToPdfOptions};
use crate::pdf::merge::{self, MergeOptions};
use crate::pdf::rotate::{self, RotateOptions};
use crate::pdf::split::{self, SplitOptions};
use crate::pdf::thumbnails;
use crate::pdf::{OutputFile, ProgressSink, TaskCtx};
use crate::state::{AppState, TaskRecord};

/// 任务请求。`kind` 与前端 `features/registry.ts` 中的功能 id 完全一致，
/// 因此前端可以直接把 `feature.id` 当作 `kind` 发过来，无需映射表。
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", content = "options")]
pub enum TaskRequest {
    #[serde(rename = "merge")]
    Merge(MergeOptions),
    #[serde(rename = "split")]
    Split(SplitOptions),
    #[serde(rename = "rotate")]
    Rotate(RotateOptions),
    #[serde(rename = "pdf2img")]
    ExportImages(ExportImagesOptions),
    #[serde(rename = "img2pdf")]
    ImagesToPdf(ImagesToPdfOptions),
}

impl TaskRequest {
    /// 功能 id，用于事件 payload 与任务记录。
    pub fn feature_id(&self) -> &'static str {
        match self {
            TaskRequest::Merge(_) => "merge",
            TaskRequest::Split(_) => "split",
            TaskRequest::Rotate(_) => "rotate",
            TaskRequest::ExportImages(_) => "pdf2img",
            TaskRequest::ImagesToPdf(_) => "img2pdf",
        }
    }

    /// 用于系统通知文案的中文名称。
    fn feature_label(&self) -> &'static str {
        match self {
            TaskRequest::Merge(_) => "PDF 合并",
            TaskRequest::Split(_) => "PDF 拆分",
            TaskRequest::Rotate(_) => "PDF 旋转",
            TaskRequest::ExportImages(_) => "PDF 转图片",
            TaskRequest::ImagesToPdf(_) => "图片转 PDF",
        }
    }
}

/// 缓存清理结果。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    /// 缩略图缓存释放的字节数
    pub thumb_bytes: u64,
    /// 被清理的临时目录数量（上次任务异常退出留下的残留）
    pub temp_dirs: usize,
    /// 临时目录释放的字节数
    pub temp_bytes: u64,
}

/// 执行一次 PDF 处理任务。
///
/// 任务在阻塞线程池中运行，期间通过 `task:progress` 上报进度；
/// 结束时发送 `task:finished` 或 `task:failed`，并把最终记录作为返回值。
#[tauri::command]
pub async fn run_task(
    app: AppHandle,
    request: TaskRequest,
    out_dir: Option<String>,
    password: Option<String>,
    notify: Option<bool>,
) -> Result<TaskRecord, AppError> {
    let feature = request.feature_id();
    let label = request.feature_label();
    let task_id = uuid::Uuid::new_v4().to_string();

    let state = app.state::<AppState>();
    let cancel = state.start_task(&task_id, feature);
    // 立即回写一次「已启动」，让任务抽屉里能马上看到条目
    state.set_progress(&task_id, 0, 0, "任务已启动");

    let cache_dir = crate::commands::pdf::cache_dir(&app)?;
    let output_dir = resolve_output_dir(&app, out_dir.as_deref())?;
    let should_notify = notify.unwrap_or(true);

    // 闭包需要拥有自己的句柄与 id，主线程还要在 spawn 失败分支里继续使用原件
    let worker = app.clone();
    let worker_task_id = task_id.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let ctx = TaskCtx::new(cache_dir, password, cancel);
        let started = Instant::now();

        // 进度回调：既写入任务中心，也广播给所有监听者（任务抽屉、功能页进度条）
        let emitter = worker.clone();
        let progress_id = worker_task_id.clone();
        let progress_feature = feature.to_string();
        let mut progress = move |current: usize, total: usize, message: &str| {
            emitter
                .state::<AppState>()
                .set_progress(&progress_id, current, total, message);
            let _ = emitter.emit(
                events::TASK_PROGRESS,
                ProgressPayload {
                    task_id: progress_id.clone(),
                    feature: progress_feature.clone(),
                    current,
                    total,
                    message: message.to_string(),
                },
            );
        };

        let outputs = dispatch(&request, &ctx, &output_dir, &mut progress);
        let elapsed_ms = started.elapsed().as_millis() as u64;

        finalize(
            &worker,
            &worker_task_id,
            feature,
            label,
            outputs,
            elapsed_ms,
            should_notify,
        )
    })
    .await;

    match result {
        Ok(inner) => inner,
        // 阻塞线程 panic 时不会走到 finalize，这里补一个失败记录，避免任务永远停在「进行中」
        Err(_) => {
            let error = AppError::Internal("任务执行线程异常退出".to_string());
            let _ = app.state::<AppState>().fail_task(&task_id, &error, 0);
            let _ = app.emit(
                events::TASK_FAILED,
                FailedPayload {
                    task_id: task_id.clone(),
                    feature: feature.to_string(),
                    error: error.clone(),
                    elapsed_ms: 0,
                },
            );
            Err(error)
        }
    }
}

/// 把任务请求分派到具体功能模块。
fn dispatch(
    request: &TaskRequest,
    ctx: &TaskCtx,
    out_dir: &Path,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    match request {
        TaskRequest::Merge(opts) => merge::run(ctx, out_dir, opts, progress),
        TaskRequest::Split(opts) => split::run(ctx, out_dir, opts, progress),
        TaskRequest::Rotate(opts) => rotate::run(ctx, out_dir, opts, progress),
        TaskRequest::ExportImages(opts) => export_images::run(ctx, out_dir, opts, progress),
        TaskRequest::ImagesToPdf(opts) => images_to_pdf::run(ctx, out_dir, opts, progress),
    }
}

/// 写入终态记录并广播 `task:finished` / `task:failed`。
fn finalize(
    app: &AppHandle,
    task_id: &str,
    feature: &str,
    label: &str,
    outputs: Result<Vec<OutputFile>, AppError>,
    elapsed_ms: u64,
    should_notify: bool,
) -> Result<TaskRecord, AppError> {
    let state = app.state::<AppState>();

    match outputs {
        Ok(files) => {
            let record = state
                .finish_task(task_id, files.clone(), elapsed_ms)
                .ok_or_else(|| AppError::Internal("任务记录已丢失".to_string()))?;

            let _ = app.emit(
                events::TASK_FINISHED,
                FinishedPayload {
                    task_id: task_id.to_string(),
                    feature: feature.to_string(),
                    outputs: files.clone(),
                    elapsed_ms,
                },
            );

            if should_notify {
                notify_success(app, label, &files, elapsed_ms);
            }

            Ok(record)
        }
        Err(error) => {
            state.fail_task(task_id, &error, elapsed_ms);
            let _ = app.emit(
                events::TASK_FAILED,
                FailedPayload {
                    task_id: task_id.to_string(),
                    feature: feature.to_string(),
                    error: error.clone(),
                    elapsed_ms,
                },
            );

            // 用户主动取消不打扰，只有真失败才发通知
            if should_notify && !matches!(error, AppError::Cancelled) {
                let _ = app
                    .notification()
                    .builder()
                    .title(format!("{label} 失败"))
                    .body(error.brief())
                    .show();
            }

            log::warn!("任务 {task_id} 失败: {}", error.brief());
            Err(error)
        }
    }
}

fn notify_success(app: &AppHandle, label: &str, files: &[OutputFile], elapsed_ms: u64) {
    let summary = match files.len() {
        0 => "没有产出文件".to_string(),
        1 => files[0].name.clone(),
        count => format!("{count} 个文件"),
    };

    let _ = app
        .notification()
        .builder()
        .title(format!("{label} 完成"))
        .body(format!(
            "{summary}（耗时 {:.1}s）",
            elapsed_ms as f64 / 1000.0
        ))
        .show();
}

/// 解析输出目录：显式指定时优先，否则退回「下载目录/应用名」。
fn resolve_output_dir(app: &AppHandle, out_dir: Option<&str>) -> Result<PathBuf, AppError> {
    let target = match out_dir.map(str::trim).filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => default_output_dir(app)?,
    };

    std::fs::create_dir_all(&target)?;
    Ok(target)
}

/// 默认输出目录：`下载目录/<应用名>`，逐级退回到文档目录、用户主目录。
///
/// 这里只计算路径、不创建目录，创建动作交给 [`resolve_output_dir`]，
/// 以免「打开关于页」这类只读操作产生副作用。
pub fn default_output_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let resolver = app.path();
    let base = resolver
        .download_dir()
        .or_else(|_| resolver.document_dir())
        .or_else(|_| resolver.home_dir())
        .map_err(|err| AppError::Io(format!("无法定位默认输出目录：{err}")))?;

    Ok(base.join(app.package_info().name.clone()))
}

/// 请求取消正在运行的任务。任务已结束或不存在时返回 false。
#[tauri::command]
pub fn cancel_task(app: AppHandle, task_id: String) -> Result<bool, AppError> {
    Ok(app.state::<AppState>().cancel_task(&task_id))
}

/// 任务中心列表，按开始时间倒序。
#[tauri::command]
pub fn list_tasks(app: AppHandle) -> Vec<TaskRecord> {
    app.state::<AppState>().list_tasks()
}

/// 清空已完成的任务记录（运行中的任务会被保留）。
#[tauri::command]
pub fn clear_tasks(app: AppHandle) -> usize {
    app.state::<AppState>().clear_tasks()
}

/// 清理缩略图缓存与上次异常退出留下的临时目录。
#[tauri::command]
pub async fn clear_temp(app: AppHandle) -> Result<CleanupReport, AppError> {
    let cache_dir = crate::commands::pdf::cache_dir(&app)?;

    tauri::async_runtime::spawn_blocking(move || {
        let thumb_bytes = thumbnails::clear_cache(&cache_dir).unwrap_or(0);

        // 任务被强杀时可能残留 `.split-<uuid>` / `.export-<uuid>` 隐藏目录
        let mut temp_dirs = 0usize;
        let mut temp_bytes = 0u64;
        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_temp_dir = path.is_dir()
                    && path
                        .file_name()
                        .map(|name| name.to_string_lossy().starts_with('.'))
                        .unwrap_or(false);
                if !is_temp_dir {
                    continue;
                }

                temp_bytes += directory_size(&path);
                if std::fs::remove_dir_all(&path).is_ok() {
                    temp_dirs += 1;
                }
            }
        }

        Ok(CleanupReport {
            thumb_bytes,
            temp_dirs,
            temp_bytes,
        })
    })
    .await
    .map_err(|_| AppError::Internal("缓存清理线程异常退出".to_string()))?
}

fn directory_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                directory_size(&path)
            } else {
                std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_request_deserializes_from_frontend_shape() {
        let raw = r#"{"kind":"rotate","options":{"source":"/tmp/a.pdf","password":null,"degrees":90,"scope":"all","inPlace":false,"outputName":null}}"#;
        let request: TaskRequest = serde_json::from_str(raw).expect("应当解析成功");
        assert_eq!(request.feature_id(), "rotate");
        assert_eq!(request.feature_label(), "PDF 旋转");
        match request {
            TaskRequest::Rotate(opts) => assert_eq!(opts.degrees, 90),
            other => panic!("变体不匹配: {other:?}"),
        }
    }

    #[test]
    fn task_request_supports_internally_tagged_mode() {
        let raw = r#"{"kind":"split","options":{"source":"/tmp/a.pdf","mode":{"mode":"every_n","size":2},"zip":true}}"#;
        let request: TaskRequest = serde_json::from_str(raw).expect("应当解析成功");
        assert_eq!(request.feature_id(), "split");
    }

    #[test]
    fn every_feature_id_is_stable() {
        let ids = [
            TaskRequest::Merge(MergeOptions::default()).feature_id(),
            TaskRequest::Split(SplitOptions::default()).feature_id(),
            TaskRequest::Rotate(RotateOptions::default()).feature_id(),
            TaskRequest::ExportImages(ExportImagesOptions::default()).feature_id(),
            TaskRequest::ImagesToPdf(ImagesToPdfOptions::default()).feature_id(),
        ];
        assert_eq!(ids, ["merge", "split", "rotate", "pdf2img", "img2pdf"]);
    }

    #[test]
    fn directory_size_of_missing_dir_is_zero() {
        assert_eq!(directory_size(Path::new("/definitely/not/here")), 0);
    }
}
