//! PDF 删除空白页：低分辨率渲染每页，统计「非白像素」占比，低于阈值的页判定为空白并删除。
//!
//! 检测只看像素占比，72 DPI 足够区分内容与空白，同时把大文档的内存与耗时压在低位。
//! 灰度 ≥ [`WHITE_LUMA`] 视为白，容忍扫描噪点与偏白纸张；阈值（百分比）可由用户调节，
//! 宽松些可清掉带零星噪点的扫描空白页，严格些可保留只有少量内容的页。
//! 支持「原地覆盖」（经临时文件原子替换）或另存为新文件。

use std::path::{Path, PathBuf};

use pdfium_render::prelude::{PdfPage, PdfPageIndex, PdfRenderConfig};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::pdf::types::{OutputFile, ProgressSink, TaskCtx};
use crate::pdf::{engine, imaging};

/// 检测渲染分辨率：空白判定只需像素占比，72 DPI 已足够，且内存可控。
const DETECT_DPI: u32 = 72;

/// 灰度达到该值即视为白色（容忍扫描噪点与偏白纸张底色）。
const WHITE_LUMA: u8 = 250;

/// 默认阈值：非白像素占比低于 0.5% 判定为空白页。
pub const DEFAULT_THRESHOLD_PCT: f32 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveBlankOptions {
    /// 源 PDF 绝对路径
    pub source: PathBuf,
    /// 源文档密码；为空时回退到任务上下文的密码
    pub password: Option<String>,
    /// 空白判定阈值（非白像素占比百分比），取值被夹在 `[0.01, 50]`
    pub threshold: f32,
    /// 为真时直接覆盖源文件（经由临时文件原子替换，不会边读边写损坏源文件）
    pub in_place: bool,
    /// 另存时的输出文件名（含 .pdf）
    pub output_name: Option<String>,
}

impl Default for RemoveBlankOptions {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            password: None,
            threshold: DEFAULT_THRESHOLD_PCT,
            in_place: false,
            output_name: None,
        }
    }
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &RemoveBlankOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    if opts.source.as_os_str().is_empty() {
        return Err(AppError::invalid("请先选择一个 PDF 文件"));
    }

    let threshold = opts.threshold.clamp(0.01, 50.0);
    let pdfium = engine::get()?;
    let password = opts.password.clone().or_else(|| ctx.password.clone());
    let mut document = pdfium.load_pdf_from_file(&opts.source, password.as_deref())?;

    let total = document.pages().len().max(0) as usize;
    if total == 0 {
        return Err(AppError::Pdf("文档中没有任何页面".to_string()));
    }

    // 检测阶段：收集空白页的 0 基索引
    let mut blank: Vec<usize> = Vec::new();
    for index in 0..total {
        ctx.check_cancel()?;
        progress(index, total, &format!("正在检测第 {} 页", index + 1));

        let page = document.pages().get(index as PdfPageIndex)?;
        if is_blank_page(&page, threshold)? {
            blank.push(index);
        }
    }

    if blank.is_empty() {
        return Err(AppError::Pdf("未检测到空白页，无需处理".to_string()));
    }
    if blank.len() == total {
        return Err(AppError::Pdf(
            "所有页面均被判定为空白，无法生成空文档".to_string(),
        ));
    }

    // 倒序删除：高索引先删，低索引不受位移影响
    for &index in blank.iter().rev() {
        ctx.check_cancel()?;
        document.pages_mut().get(index as PdfPageIndex)?.delete()?;
    }
    let remaining = (total - blank.len()) as u32;

    ctx.check_cancel()?;
    progress(total, total, "正在写出 PDF");

    let stem = crate::pdf::file_stem(&opts.source);
    let out_path = if opts.in_place {
        opts.source.clone()
    } else {
        let fallback = format!("{stem}_noblank.pdf");
        crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback)
    };
    crate::pdf::save_document(&document, &out_path, Some(&opts.source))?;

    // 完成消息带上被删页码（1 基），过多时截断，避免任务中心文案过长
    let pages_label = blank
        .iter()
        .take(8)
        .map(|index| (index + 1).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let ellipsis = if blank.len() > 8 { "…" } else { "" };
    progress(
        total,
        total,
        &format!(
            "已删除 {} 个空白页（第 {pages_label}{ellipsis} 页）",
            blank.len()
        ),
    );

    Ok(vec![OutputFile::pdf(out_path, Some(remaining))?])
}

/// 低分辨率渲染单页并按非白像素占比判定空白。注释一并渲染（印章 / 批注也算内容）。
fn is_blank_page(page: &PdfPage<'_>, threshold_pct: f32) -> Result<bool, AppError> {
    let target_width = imaging::pixels_for_dpi(page.width().value, DETECT_DPI);
    let config = PdfRenderConfig::new()
        .set_target_width(target_width)
        .render_annotations(true);
    let image = page.render_with_config(&config)?.as_image()?;
    Ok(ink_ratio_percent(&image) < f64::from(threshold_pct))
}

/// 非白像素占比（0-100）。纯函数，便于单元测试。
pub fn ink_ratio_percent(image: &image::DynamicImage) -> f64 {
    let luma = image.to_luma8();
    let total = u64::from(luma.width()) * u64::from(luma.height());
    if total == 0 {
        return 0.0;
    }
    let dark = luma.pixels().filter(|pixel| pixel.0[0] < WHITE_LUMA).count() as u64;
    dark as f64 / total as f64 * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, Luma};

    fn luma_image(width: u32, height: u32, fill: u8) -> DynamicImage {
        DynamicImage::ImageLuma8(ImageBuffer::from_pixel(width, height, Luma([fill])))
    }

    #[test]
    fn pure_white_has_zero_ink() {
        assert_eq!(ink_ratio_percent(&luma_image(100, 100, 255)), 0.0);
        // 接近白的扫描底色同样算白
        assert_eq!(ink_ratio_percent(&luma_image(100, 100, WHITE_LUMA)), 0.0);
    }

    #[test]
    fn pure_black_is_full_ink() {
        assert_eq!(ink_ratio_percent(&luma_image(100, 100, 0)), 100.0);
    }

    #[test]
    fn half_dark_image_is_about_fifty_percent() {
        let mut buffer = ImageBuffer::from_pixel(100, 100, Luma([255u8]));
        for y in 0..50 {
            for x in 0..100 {
                buffer.put_pixel(x, y, Luma([0]));
            }
        }
        let ratio = ink_ratio_percent(&DynamicImage::ImageLuma8(buffer));
        assert!((ratio - 50.0).abs() < 1e-9);
    }

    #[test]
    fn empty_image_does_not_panic() {
        assert_eq!(ink_ratio_percent(&luma_image(0, 0, 0)), 0.0);
    }

    #[test]
    fn default_options_are_usable() {
        let opts = RemoveBlankOptions::default();
        assert_eq!(opts.threshold, DEFAULT_THRESHOLD_PCT);
        assert!(!opts.in_place);
    }
}