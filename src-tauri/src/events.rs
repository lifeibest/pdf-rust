//! 前后端事件契约。
//!
//! 事件名与 payload 字段是前端 `hooks/useTaskRunner.ts` 的唯一依赖，
//! 修改这里必须同步修改前端，并更新 `docs/ARCHITECTURE.md` 中的事件表。

use serde::Serialize;

use crate::error::AppError;

/// 任务进度：每处理一个单位（页 / 文件）发一次。
pub const TASK_PROGRESS: &str = "task:progress";
/// 任务日志：告警与非致命提示。
pub const TASK_LOG: &str = "task:log";
/// 任务成功结束。
pub const TASK_FINISHED: &str = "task:finished";
/// 任务失败或被取消。
pub const TASK_FAILED: &str = "task:failed";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub task_id: String,
    pub feature: String,
    /// 已完成数量
    pub current: usize,
    /// 总数量；未知时为 0
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPayload {
    pub task_id: String,
    /// `info` / `warn` / `error`
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishedPayload {
    pub task_id: String,
    pub feature: String,
    pub outputs: Vec<crate::pdf::OutputFile>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedPayload {
    pub task_id: String,
    pub feature: String,
    /// 结构化错误，前端按 `kind` 映射 i18n 文案
    pub error: AppError,
    pub elapsed_ms: u64,
}
