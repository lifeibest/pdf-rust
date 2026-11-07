//! 应用级共享状态：任务记录表与取消令牌表。
//!
//! 作为 Tauri managed state 注入（`app.manage(AppState::default())`），
//! 命令通过 `State<'_, AppState>` 或 `app.state::<AppState>()` 访问。

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;

use crate::error::AppError;
use crate::pdf::{CancelToken, OutputFile};

/// 任务中心保留的历史记录上限，超出后按开始时间淘汰最旧的条目。
const MAX_TASK_RECORDS: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Running,
    Finished,
    Failed,
    Cancelled,
}

/// 单个任务的状态快照，同时作为 `run_task` 的返回值与 `list_tasks` 的元素。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    /// 功能 id，与前端 `features/registry.ts` 中的 `id` 一致
    pub feature: String,
    pub status: TaskStatus,
    pub current: usize,
    pub total: usize,
    pub message: String,
    pub outputs: Vec<OutputFile>,
    /// 失败时的可读描述
    pub error: Option<String>,
    /// 失败时的结构化错误 kind，前端据此映射 i18n 文案
    pub error_kind: Option<String>,
    /// Unix 毫秒时间戳
    pub started_at: i64,
    pub elapsed_ms: u64,
}

impl TaskRecord {
    pub fn new(id: String, feature: String) -> Self {
        Self {
            id,
            feature,
            status: TaskStatus::Running,
            current: 0,
            total: 0,
            message: String::new(),
            outputs: Vec::new(),
            error: None,
            error_kind: None,
            started_at: now_ms(),
            elapsed_ms: 0,
        }
    }

    pub fn is_running(&self) -> bool {
        self.status == TaskStatus::Running
    }
}

/// 全局状态。所有字段都用 `Mutex` 而非 `RwLock`：临界区极短，
/// 且 `Mutex` 的写优先语义更适合「进度高频更新」的场景。
#[derive(Debug, Default)]
pub struct AppState {
    tasks: Mutex<HashMap<String, TaskRecord>>,
    cancels: Mutex<HashMap<String, CancelToken>>,
}

impl AppState {
    /// 登记一个新任务，返回其取消令牌。
    pub fn start_task(&self, id: &str, feature: &str) -> CancelToken {
        let token = CancelToken::new();
        self.tasks.lock().expect("tasks 锁中毒").insert(
            id.to_string(),
            TaskRecord::new(id.to_string(), feature.to_string()),
        );
        self.cancels
            .lock()
            .expect("cancels 锁中毒")
            .insert(id.to_string(), token.clone());
        token
    }

    /// 更新进度；任务不存在时静默忽略（可能已被清理）。
    pub fn set_progress(&self, id: &str, current: usize, total: usize, message: &str) {
        if let Some(record) = self.tasks.lock().expect("tasks 锁中毒").get_mut(id) {
            record.current = current;
            record.total = total;
            record.message = message.to_string();
        }
    }

    /// 标记任务成功，返回最终记录。
    pub fn finish_task(
        &self,
        id: &str,
        outputs: Vec<OutputFile>,
        elapsed_ms: u64,
    ) -> Option<TaskRecord> {
        self.complete(id, elapsed_ms, |record| {
            record.status = TaskStatus::Finished;
            record.outputs = outputs;
            record.message = "已完成".to_string();
        })
    }

    /// 标记任务失败；取消被单独识别为 `Cancelled` 状态。
    pub fn fail_task(&self, id: &str, error: &AppError, elapsed_ms: u64) -> Option<TaskRecord> {
        let cancelled = matches!(error, AppError::Cancelled);
        let message = error.brief();
        let kind = error_kind(error);

        self.complete(id, elapsed_ms, move |record| {
            record.status = if cancelled {
                TaskStatus::Cancelled
            } else {
                TaskStatus::Failed
            };
            record.error = Some(message);
            record.error_kind = Some(kind);
        })
    }

    fn complete(
        &self,
        id: &str,
        elapsed_ms: u64,
        update: impl FnOnce(&mut TaskRecord),
    ) -> Option<TaskRecord> {
        let snapshot = {
            let mut tasks = self.tasks.lock().expect("tasks 锁中毒");
            let record = tasks.get_mut(id)?;
            update(record);
            record.elapsed_ms = elapsed_ms;
            let snapshot = record.clone();
            self.prune(&mut tasks);
            snapshot
        };

        // 取消令牌在任务结束后不再有用，及时释放
        self.cancels.lock().expect("cancels 锁中毒").remove(id);
        Some(snapshot)
    }

    /// 请求取消；任务不存在或已结束时返回 false。
    pub fn cancel_task(&self, id: &str) -> bool {
        match self.cancels.lock().expect("cancels 锁中毒").get(id) {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    pub fn is_cancelled(&self, id: &str) -> bool {
        self.cancels
            .lock()
            .expect("cancels 锁中毒")
            .get(id)
            .is_some_and(CancelToken::is_cancelled)
    }

    /// 任务列表，按开始时间倒序（最新在前）。
    pub fn list_tasks(&self) -> Vec<TaskRecord> {
        let mut records = self
            .tasks
            .lock()
            .expect("tasks 锁中毒")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by_key(|record| std::cmp::Reverse(record.started_at));
        records
    }

    /// 清空全部历史记录，返回被清理的条数。
    pub fn clear_tasks(&self) -> usize {
        let mut tasks = self.tasks.lock().expect("tasks 锁中毒");
        let running = tasks
            .values()
            .filter(|record| record.is_running())
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();

        let removed = tasks.len() - running.len();
        tasks.retain(|id, _| running.contains(id));

        let mut cancels = self.cancels.lock().expect("cancels 锁中毒");
        cancels.retain(|id, _| running.contains(id));
        removed
    }

    /// 超出上限时淘汰最旧的「非运行中」记录。
    fn prune(&self, tasks: &mut HashMap<String, TaskRecord>) {
        if tasks.len() <= MAX_TASK_RECORDS {
            return;
        }

        let mut finished = tasks
            .values()
            .filter(|record| !record.is_running())
            .map(|record| (record.id.clone(), record.started_at))
            .collect::<Vec<_>>();
        finished.sort_by_key(|(_, started_at)| *started_at);

        let excess = tasks.len() - MAX_TASK_RECORDS;
        for (id, _) in finished.into_iter().take(excess) {
            tasks.remove(&id);
            self.cancels.lock().expect("cancels 锁中毒").remove(&id);
        }
    }
}

/// 把结构化错误映射为稳定的机器可读 kind，与 `AppError` 的 serde 标签保持一致。
pub fn error_kind(error: &AppError) -> String {
    match error {
        AppError::PdfiumNotFound(_) => "pdfiumNotFound",
        AppError::PasswordRequired(_) => "passwordRequired",
        AppError::Pdf(_) => "pdf",
        AppError::Io(_) => "io",
        AppError::Image(_) => "image",
        AppError::Archive(_) => "archive",
        AppError::InvalidParams(_) => "invalidParams",
        AppError::Range(_) => "range",
        AppError::Unsupported(_) => "unsupported",
        AppError::Cancelled => "cancelled",
        AppError::Internal(_) => "internal",
    }
    .to_string()
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(path: &str) -> OutputFile {
        OutputFile {
            path: std::path::PathBuf::from(path),
            name: "a.pdf".to_string(),
            kind: crate::pdf::OutputKind::Pdf,
            size_bytes: 0,
            pages: Some(1),
        }
    }

    #[test]
    fn tracks_task_lifecycle() {
        let state = AppState::default();
        let token = state.start_task("t1", "merge");
        assert!(!token.is_cancelled());

        state.set_progress("t1", 2, 5, "处理中");
        let record = state
            .finish_task("t1", vec![output("/tmp/a.pdf")], 120)
            .expect("应当存在");
        assert_eq!(record.status, TaskStatus::Finished);
        assert_eq!(record.current, 2);
        assert_eq!(record.elapsed_ms, 120);
        assert_eq!(record.outputs.len(), 1);
        // 结束后取消令牌被释放，无法再取消
        assert!(!state.cancel_task("t1"));
    }

    #[test]
    fn distinguishes_cancel_from_failure() {
        let state = AppState::default();
        state.start_task("c1", "split");
        assert!(state.cancel_task("c1"));
        assert!(state.is_cancelled("c1"));

        let record = state
            .fail_task("c1", &AppError::Cancelled, 10)
            .expect("应当存在");
        assert_eq!(record.status, TaskStatus::Cancelled);
        assert_eq!(record.error_kind.as_deref(), Some("cancelled"));

        state.start_task("f1", "split");
        let record = state
            .fail_task("f1", &AppError::Range("页码越界".to_string()), 10)
            .expect("应当存在");
        assert_eq!(record.status, TaskStatus::Failed);
        assert_eq!(record.error_kind.as_deref(), Some("range"));
    }

    #[test]
    fn keeps_running_tasks_when_clearing() {
        let state = AppState::default();
        state.start_task("live", "merge");
        state.start_task("done", "merge");
        state.finish_task("done", vec![], 1);

        assert_eq!(state.clear_tasks(), 1);
        let ids = state
            .list_tasks()
            .into_iter()
            .map(|record| record.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["live"]);
    }

    #[test]
    fn prunes_oldest_finished_records() {
        let state = AppState::default();
        for index in 0..(MAX_TASK_RECORDS + 10) {
            let id = format!("t{index}");
            state.start_task(&id, "merge");
            state.finish_task(&id, vec![], index as u64);
        }
        assert!(state.list_tasks().len() <= MAX_TASK_RECORDS);
    }
}
