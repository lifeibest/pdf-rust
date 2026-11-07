//! Tauri command 层。
//!
//! 按职责拆分为四个子模块：
//! - [`app`]：应用与引擎信息（关于页）
//! - [`fs`]：文件 / 目录选择、在访达中定位
//! - [`pdf`]：文档探测与缩略图
//! - [`tasks`]：统一任务入口（进度事件、取消、任务中心）

pub mod app;
pub mod fs;
pub mod pdf;
pub mod tasks;
