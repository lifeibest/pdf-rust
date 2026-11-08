//! 页面缩略图渲染与磁盘缓存。
//!
//! 缩略图写入 app cache 目录，前端通过 Tauri 的 asset 协议（`convertFileSrc`）直接读取，
//! 避免把 base64 图片塞进 IPC 消息。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use pdfium_render::prelude::{PdfPageIndex, PdfPageRenderRotation, PdfRenderConfig};
use serde::Serialize;

use crate::error::AppError;
use crate::pdf::engine;
use crate::pdf::imaging;
use crate::pdf::info::rotation_degrees;

/// 单页缩略图的产出信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbOut {
    /// 0-based 页索引
    pub page: u32,
    /// 缩略图 PNG 的绝对路径
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    /// 页面自带的旋转角，前端可据此决定是否需要额外旋转预览
    pub rotation_deg: u16,
}

/// 渲染指定页面的缩略图。`pages` 为空时表示渲染全部页面。
///
/// 命中缓存的页面不会重复渲染，因此重复打开同一文档几乎零成本。
pub fn render_thumbs(
    source: &Path,
    password: Option<&str>,
    pages: &[u32],
    max_px: u32,
    cache_root: &Path,
) -> Result<Vec<ThumbOut>, AppError> {
    let pdfium = engine::get()?;
    let document = pdfium.load_pdf_from_file(source, password)?;

    let page_count = document.pages().len().max(0) as u32;
    if page_count == 0 {
        return Err(AppError::Pdf("文档中没有任何页面".to_string()));
    }

    let targets: Vec<u32> = if pages.is_empty() {
        (0..page_count).collect()
    } else {
        let mut cleaned = pages
            .iter()
            .copied()
            .filter(|index| *index < page_count)
            .collect::<Vec<_>>();
        cleaned.sort_unstable();
        cleaned.dedup();
        if cleaned.is_empty() {
            return Err(AppError::Range("请求的页面索引全部超出范围".to_string()));
        }
        cleaned
    };

    let dir = cache_dir_for(cache_root, source, max_px);
    std::fs::create_dir_all(&dir)?;

    let max_px = max_px.clamp(32, 2048);
    let mut results = Vec::with_capacity(targets.len());

    for index in targets {
        let page = document.pages().get(index as PdfPageIndex)?;
        let rotation = page.rotation().unwrap_or(PdfPageRenderRotation::None);
        let rotation_deg = rotation_degrees(rotation);

        let out_path = dir.join(format!("p{index}.png"));
        if let Some(cached) = read_cached(&out_path) {
            results.push(ThumbOut {
                page: index,
                path: out_path,
                width: cached.0,
                height: cached.1,
                rotation_deg,
            });
            continue;
        }

        // 依据「计入旋转后的」宽高比推导目标宽度，使缩略图长边不超过 max_px
        let (width_pt, height_pt) =
            oriented_size(page.width().value, page.height().value, rotation_deg);
        let ratio = if height_pt > 0.0 {
            width_pt / height_pt
        } else {
            1.0
        };
        let target_width = if ratio >= 1.0 {
            max_px as f32
        } else {
            (max_px as f32 * ratio).round().max(16.0)
        };

        let config = PdfRenderConfig::new()
            .set_target_width(target_width.round() as i32)
            .use_print_quality(true);

        let bitmap = page.render_with_config(&config)?;
        let image = bitmap.as_image()?;
        let (width, height) = (image.width(), image.height());
        image.save_with_format(&out_path, image::ImageFormat::Png)?;

        results.push(ThumbOut {
            page: index,
            path: out_path,
            width,
            height,
            rotation_deg,
        });
    }

    Ok(results)
}

/// 渲染图片缩略图：解码 → 等比缩小到长边不超过 `max_px` → 写 PNG 到缓存目录。
///
/// 与 PDF 缩略图共用 `cache_dir_for` / `read_cached`，因此同样享受磁盘缓存与「清理缓存」。
/// 返回单元素 `Vec`，形状与 `render_thumbs` 一致，前端无需区分来源。
/// 注：`image` 0.25 不按 EXIF 自动旋转，手机竖拍照片可能侧躺，属已知取舍。
pub fn render_image_thumb(
    source: &Path,
    max_px: u32,
    cache_root: &Path,
) -> Result<Vec<ThumbOut>, AppError> {
    let max_px = max_px.clamp(32, 2048);
    let dir = cache_dir_for(cache_root, source, max_px);
    std::fs::create_dir_all(&dir)?;

    let out_path = dir.join("p0.png");
    if let Some((width, height)) = read_cached(&out_path) {
        return Ok(vec![ThumbOut {
            page: 0,
            path: out_path,
            width,
            height,
            rotation_deg: 0,
        }]);
    }

    // thumbnail() 等比缩放并适配到 max_px × max_px 内，使用快速采样，适合预览
    let image = imaging::load_image(source)?;
    let thumb = image.thumbnail(max_px, max_px);
    let (width, height) = (thumb.width(), thumb.height());
    thumb.save_with_format(&out_path, image::ImageFormat::Png)?;

    Ok(vec![ThumbOut {
        page: 0,
        path: out_path,
        width,
        height,
        rotation_deg: 0,
    }])
}

/// 计入 `/Rotate` 后的有效宽高。
fn oriented_size(width_pt: f32, height_pt: f32, rotation_deg: u16) -> (f32, f32) {
    if rotation_deg == 90 || rotation_deg == 270 {
        (height_pt, width_pt)
    } else {
        (width_pt, height_pt)
    }
}

/// 读取已缓存的缩略图尺寸；文件不存在或损坏时返回 `None` 触发重新渲染。
fn read_cached(path: &Path) -> Option<(u32, u32)> {
    if !path.exists() {
        return None;
    }
    let reader = image::ImageReader::open(path).ok()?;
    let (width, height) = reader.into_dimensions().ok()?;
    Some((width, height))
}

/// 依据「路径 + 修改时间 + 大小 + 目标尺寸」生成稳定的缓存目录。
fn cache_dir_for(cache_root: &Path, source: &Path, max_px: u32) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    source.display().to_string().hash(&mut hasher);
    max_px.hash(&mut hasher);
    if let Ok(metadata) = std::fs::metadata(source) {
        metadata.len().hash(&mut hasher);
        if let Ok(modified) = metadata.modified() {
            // 用 duration 作为可哈希的时间表示，避免 SystemTime 本身不可 Hash
            modified
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .hash(&mut hasher);
        }
    }
    cache_root
        .join("thumbs")
        .join(format!("{:016x}", hasher.finish()))
}

/// 清空缩略图缓存，返回释放的字节数。
pub fn clear_cache(cache_root: &Path) -> Result<u64, AppError> {
    let dir = cache_root.join("thumbs");
    if !dir.exists() {
        return Ok(0);
    }

    let removed = directory_size(&dir);
    std::fs::remove_dir_all(&dir)?;
    Ok(removed)
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
    fn swaps_dimensions_for_quarter_turn_rotations() {
        assert_eq!(oriented_size(100.0, 200.0, 0), (100.0, 200.0));
        assert_eq!(oriented_size(100.0, 200.0, 180), (100.0, 200.0));
        assert_eq!(oriented_size(100.0, 200.0, 90), (200.0, 100.0));
        assert_eq!(oriented_size(100.0, 200.0, 270), (200.0, 100.0));
    }

    #[test]
    fn cache_dir_is_stable_for_same_inputs_and_changes_with_size() {
        let root = Path::new("/tmp/cache");
        let source = Path::new("/tmp/sample.pdf");

        let a = cache_dir_for(root, source, 256);
        let b = cache_dir_for(root, source, 256);
        let c = cache_dir_for(root, source, 512);

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with(root.join("thumbs")));
    }
}
