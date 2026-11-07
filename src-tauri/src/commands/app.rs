//! 应用信息与引擎诊断。

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::pdf::engine::{self, EngineStatus};
use crate::pdf::imaging::IMAGE_EXTENSIONS;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub identifier: String,
    /// Tauri 框架版本
    pub tauri_version: String,
    pub os: String,
    pub arch: String,
    pub resource_dir: Option<String>,
    pub cache_dir: Option<String>,
    pub config_dir: Option<String>,
    /// 默认输出目录（下载目录下的应用子目录）
    pub default_output_dir: Option<String>,
    pub pdfium: EngineStatus,
    /// 支持的图片扩展名，供前端做拖拽过滤
    pub image_extensions: Vec<String>,
}

/// 关于页与诊断面板使用的应用信息快照。
#[tauri::command]
pub fn app_info(app: AppHandle) -> Result<AppInfo, AppError> {
    let package = app.package_info();
    let path = |result: tauri::Result<std::path::PathBuf>| {
        result.ok().map(|dir| dir.display().to_string())
    };

    Ok(AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
        identifier: app.config().identifier.clone(),
        tauri_version: tauri::VERSION.to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        resource_dir: path(app.path().resource_dir()),
        cache_dir: path(app.path().app_cache_dir()),
        config_dir: path(app.path().app_config_dir()),
        default_output_dir: crate::commands::tasks::default_output_dir(&app)
            .ok()
            .map(|dir| dir.display().to_string()),
        pdfium: engine::status(),
        image_extensions: IMAGE_EXTENSIONS.iter().map(|ext| ext.to_string()).collect(),
    })
}

/// 单独暴露引擎状态，便于「重新检测 PDFium」按钮在不重启应用的情况下刷新。
#[tauri::command]
pub fn pdfium_status() -> EngineStatus {
    engine::status()
}

/// 触发一次引擎初始化并把状态返回给前端。
///
/// 首次绑定失败后（例如用户手动把 libpdfium 放到位），可以借此在不重启的前提下恢复。
#[tauri::command]
pub fn reload_pdfium(app: AppHandle) -> Result<EngineStatus, AppError> {
    let extra = app
        .path()
        .resource_dir()
        .map(|dir| vec![dir])
        .unwrap_or_default();
    engine::init(&extra)?;
    Ok(engine::status())
}
