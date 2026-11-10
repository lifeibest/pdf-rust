//! Tauri command 层。
//!
//! 按职责拆分为五个子模块：
//! - [`app`]：应用与引擎信息（关于页）
//! - [`fs`]：文件 / 目录选择、在访达中定位
//! - [`license`]：激活码校验/存取与配额门控
//! - [`pdf`]：文档探测与缩略图
//! - [`tasks`]：统一任务入口（进度事件、取消、任务中心）

pub mod app;
pub mod fs;
pub mod license;
pub mod pdf;
pub mod tasks;
