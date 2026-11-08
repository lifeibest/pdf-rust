//! 文档探测与缩略图命令。

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::pdf::imaging;
use crate::pdf::info::{self, DocInfo};
use crate::pdf::thumbnails::{self, ThumbOut};

/// 缩略图默认长边像素；256 在 2x 屏上足够清晰，又不会让首次打开太慢。
const DEFAULT_THUMB_PX: u32 = 256;

/// 探测文档信息（页数、尺寸、旋转、元数据、是否加密）。
///
/// 加密文档不报错，而是返回 `encrypted = true`，前端据此弹出密码输入框后重试。
#[tauri::command]
pub async fn probe_document(
    path: String,
    password: Option<String>,
    include_pages: Option<bool>,
) -> Result<DocInfo, AppError> {
    let include_pages = include_pages.unwrap_or(true);

    tauri::async_runtime::spawn_blocking(move || {
        info::probe_with_pages(PathBuf::from(&path), password.as_deref(), include_pages)
    })
    .await
    .map_err(|_| AppError::Internal("文档探测线程异常退出".to_string()))?
}

/// 渲染缩略图，返回写入磁盘的 PNG 路径。
///
/// PDF 走 pdfium 逐页渲染；图片走 `render_image_thumb`（解码 + 等比缩小）。
/// 两者形状一致，前端同一个命令即可处理两类来源。
/// 前端拿到路径后通过 `convertFileSrc()` 经 asset 协议加载，
/// 因此 `tauri.conf.json` 中必须开启 `assetProtocol` 且 scope 覆盖 `$APPCACHE/**`。
#[tauri::command]
pub async fn render_thumbnails(
    app: AppHandle,
    path: String,
    password: Option<String>,
    pages: Option<Vec<u32>>,
    max_px: Option<u32>,
) -> Result<Vec<ThumbOut>, AppError> {
    let cache_dir = cache_dir(&app)?;
    let source = PathBuf::from(&path);
    let pages = pages.unwrap_or_default();
    let max_px = max_px.unwrap_or(DEFAULT_THUMB_PX);

    tauri::async_runtime::spawn_blocking(move || {
        if imaging::is_image(&source) {
            thumbnails::render_image_thumb(&source, max_px, &cache_dir)
        } else {
            thumbnails::render_thumbs(&source, password.as_deref(), &pages, max_px, &cache_dir)
        }
    })
    .await
    .map_err(|_| AppError::Internal("缩略图渲染线程异常退出".to_string()))?
}

/// 清空缩略图缓存，返回释放的字节数。设置页的「清理缓存」按钮使用。
#[tauri::command]
pub async fn clear_thumb_cache(app: AppHandle) -> Result<u64, AppError> {
    let cache_dir = cache_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || thumbnails::clear_cache(&cache_dir))
        .await
        .map_err(|_| AppError::Internal("缓存清理线程异常退出".to_string()))?
}

/// 缩略图缓存根目录（app cache dir）。
pub fn cache_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_cache_dir()
        .map_err(|err| AppError::Io(format!("无法定位应用缓存目录：{err}")))
}
