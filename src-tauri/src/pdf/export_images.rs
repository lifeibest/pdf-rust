//! PDF 转图片：把指定页面按 DPI 栅格化为 PNG / JPEG / WebP。
//!
//! 输出目录直接落盘；勾选打包时先写入临时目录再压成单个 zip，避免在用户目录里
//! 散落几十上百个图片文件。

use std::path::{Path, PathBuf};

use pdfium_render::prelude::{PdfPage, PdfPageIndex, PdfRenderConfig};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::pdf::engine;
use crate::pdf::imaging;
use crate::pdf::ranges;
use crate::pdf::types::{ImageFormatOpt, OutputFile, PageScope, ProgressSink, TaskCtx};

/// 单页栅格化的像素上限，超过该值的位图会被等比缩小。
/// 用于防止「A0 图纸 + 1200 DPI」这类组合把内存打爆。
pub const DEFAULT_MAX_EDGE_PX: u32 = 8000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportImagesOptions {
    /// 源 PDF 绝对路径
    pub source: PathBuf,
    /// 源文档密码；为空时回退到任务上下文的密码
    pub password: Option<String>,
    /// 导出页范围
    pub scope: PageScope,
    /// 渲染 DPI，取值被夹在 `[24, 1200]`
    pub dpi: u32,
    pub format: ImageFormatOpt,
    /// JPEG 质量（1-100）；PNG 与 WebP 忽略该值
    pub quality: u8,
    /// 长边像素上限；`None` 表示使用 [`DEFAULT_MAX_EDGE_PX`]
    pub max_edge_px: Option<u32>,
    /// 是否把全部图片打包为单个 zip
    pub zip: bool,
    /// 是否将所选页面竖向拼接为一张长图（适合微信/小红书分享）
    #[serde(default)]
    pub stitch: bool,
    /// 输出文件名（zip 模式下为 .zip；长图模式下为对应图片名）
    pub output_name: Option<String>,
}

impl Default for ExportImagesOptions {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            password: None,
            scope: PageScope::All,
            dpi: 150,
            format: ImageFormatOpt::Png,
            quality: 92,
            max_edge_px: None,
            zip: false,
            stitch: false,
            output_name: None,
        }
    }
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &ExportImagesOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    if opts.source.as_os_str().is_empty() {
        return Err(AppError::invalid("请先选择一个 PDF 文件"));
    }

    let pdfium = engine::get()?;
    let password = opts.password.clone().or_else(|| ctx.password.clone());
    let document = pdfium.load_pdf_from_file(&opts.source, password.as_deref())?;

    let total = document.pages().len().max(0) as usize;
    if total == 0 {
        return Err(AppError::Pdf("文档中没有任何页面".to_string()));
    }

    // 保持用户书写顺序（例如 `5,1-2` 就先导出第 5 页）
    let selected = ranges::parse_ranges(&opts.scope.as_spec(), total)?;
    let stem = crate::pdf::file_stem(&opts.source);
    let extension = opts.format.extension();
    let max_edge = opts
        .max_edge_px
        .unwrap_or(DEFAULT_MAX_EDGE_PX)
        .clamp(64, 20_000);

    let count = selected.len();

    // 长图拼接模式：逐页渲染后垂直拼接为一张连续长图，方便微信/手机阅读
    if opts.stitch {
        let mut rendered = Vec::with_capacity(count);
        for (index, page_number) in selected.iter().enumerate() {
            ctx.check_cancel()?;
            progress(index, count, &format!("正在渲染第 {page_number} 页"));
            let page = document.pages().get((*page_number - 1) as PdfPageIndex)?;
            rendered.push(render_page(&page, opts.dpi, max_edge)?);
        }

        ctx.check_cancel()?;
        progress(count, count, "正在拼接长图");
        let stitched = stitch_images_vertically(&rendered);
        let (sw, sh) = (stitched.width(), stitched.height());
        let fallback = format!("{stem}_long.{extension}");
        let out_path =
            crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
        imaging::encode_image(&stitched, &out_path, opts.format, opts.quality)?;

        let summary = format!("已将 {count} 页拼接为一张长图（{sw} × {sh} 像素）");
        progress(count, count, &summary);
        ctx.set_summary(summary);
        return Ok(vec![OutputFile::image(&out_path)?]);
    }

    let work_dir = if opts.zip {
        Some(crate::pdf::unique_temp_dir(&ctx.cache_dir, "export")?)
    } else {
        None
    };
    let target_dir = work_dir.as_deref().unwrap_or(out_dir);

    let mut outputs = Vec::with_capacity(count);

    for (index, page_number) in selected.iter().enumerate() {
        ctx.check_cancel()?;
        progress(index, count, &format!("正在渲染第 {page_number} 页"));

        let page = document.pages().get((*page_number - 1) as PdfPageIndex)?;
        let image = render_page(&page, opts.dpi, max_edge)?;

        let file_name = format!("{stem}_p{page_number:03}.{extension}");
        let out_path = crate::pdf::resolve_batch_path(target_dir, &file_name);
        imaging::encode_image(&image, &out_path, opts.format, opts.quality)?;

        if !opts.zip {
            outputs.push(OutputFile::image(&out_path)?);
        }
    }

    ctx.check_cancel()?;

    if let Some(dir) = &work_dir {
        progress(count, count, "正在打包 zip");
        let fallback = format!("{stem}_images.zip");
        let zip_path =
            crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
        let archive = crate::pdf::zip_directory(dir, &zip_path)?;
        crate::pdf::remove_dir_quiet(dir);
        return Ok(vec![archive]);
    }

    progress(count, count, "导出完成");
    Ok(outputs)
}

/// 按 DPI 渲染单页为位图，并施加长边上限。
pub fn render_page(
    page: &PdfPage<'_>,
    dpi: u32,
    max_edge_px: u32,
) -> Result<image::DynamicImage, AppError> {
    let target_width = imaging::pixels_for_dpi(page.width().value, dpi);
    let config = PdfRenderConfig::new()
        .set_target_width(target_width)
        .set_maximum_width(max_edge_px as i32)
        .set_maximum_height(max_edge_px as i32)
        .render_annotations(true)
        .use_print_quality(true);

    let bitmap = page.render_with_config(&config)?;
    Ok(bitmap.as_image()?)
}

/// 将多张页面图像垂直拼接为一张长图（白底、页间留 12px 浅灰分隔线，总高上限 30000px 防爆内存）。
fn stitch_images_vertically(pages: &[image::DynamicImage]) -> image::DynamicImage {
    use image::imageops::{self, FilterType};

    const MAX_TOTAL_HEIGHT: u32 = 30_000;
    const GAP_PX: u32 = 12;

    if pages.is_empty() {
        return image::DynamicImage::new_rgb8(1, 1);
    }

    let raw_max_w = pages.iter().map(|img| img.width()).max().unwrap_or(1).max(1);
    let raw_total_h: u64 = pages.iter().map(|img| u64::from(img.height())).sum::<u64>()
        + u64::from(GAP_PX) * (pages.len().saturating_sub(1) as u64);

    // 超长文档等比缩小，避免超出 JPEG/PNG 常规高度上限或占满内存
    let scale = if raw_total_h > u64::from(MAX_TOTAL_HEIGHT) {
        MAX_TOTAL_HEIGHT as f64 / raw_total_h as f64
    } else {
        1.0
    };

    let canvas_w = ((raw_max_w as f64 * scale).round() as u32).max(1);
    let gap = ((GAP_PX as f64 * scale).round() as u32).max(2);

    let resized: Vec<image::RgbImage> = pages
        .iter()
        .map(|img| {
            let rgb = img.to_rgb8();
            if scale < 0.999 {
                let nw = ((rgb.width() as f64 * scale).round() as u32).max(1);
                let nh = ((rgb.height() as f64 * scale).round() as u32).max(1);
                imageops::resize(&rgb, nw, nh, FilterType::Triangle)
            } else {
                rgb
            }
        })
        .collect();

    let total_h: u32 = resized
        .iter()
        .map(|img| img.height())
        .sum::<u32>()
        .saturating_add(gap.saturating_mul(resized.len().saturating_sub(1) as u32))
        .max(1);

    let mut canvas = image::RgbImage::from_pixel(canvas_w, total_h, image::Rgb([255, 255, 255]));
    let mut y = 0u32;
    for (idx, page) in resized.iter().enumerate() {
        let x = (canvas_w.saturating_sub(page.width())) / 2;
        imageops::replace(&mut canvas, page, i64::from(x), i64::from(y));
        y = y.saturating_add(page.height());
        if idx + 1 < resized.len() {
            // 画一条柔和的浅灰分隔线，帮助区分相邻页边界
            let line_y = y + gap / 2;
            if line_y < total_h {
                for lx in 0..canvas_w {
                    canvas.put_pixel(lx, line_y, image::Rgb([230, 232, 236]));
                }
            }
            y = y.saturating_add(gap);
        }
    }

    image::DynamicImage::ImageRgb8(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_are_usable() {
        let opts = ExportImagesOptions::default();
        assert_eq!(opts.format.extension(), "png");
        assert!(opts.dpi >= 24 && opts.dpi <= 1200);
        assert!(opts.quality >= 1 && opts.quality <= 100);
        assert_eq!(opts.max_edge_px, None);
    }

    #[test]
    fn max_edge_default_is_bounded() {
        // 默认上限是编译期常量，断言也应在编译期完成
        const {
            assert!(DEFAULT_MAX_EDGE_PX >= 64);
            assert!(DEFAULT_MAX_EDGE_PX <= 20_000);
        }
    }
}
