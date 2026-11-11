//! PDF 压缩：整页栅格化为 JPEG 后按原始页面尺寸重建文档（「图像化压缩」）。
//!
//! 选型说明：PDFium 没有公开的图片对象 JPEG 重注入 API（`set_image` 走
//! `FPDFImageObj_SetBitmap`，以 Flate 无损存储，照片反而会变大），因此采用与
//! 服务端 compress_pdf 一致的栅格化路线：渲染 → JPEG 重编码 → 重建。
//! 代价是文字层丢失（不可再选择 / 搜索），对扫描件与图片型文档收益最大。
//!
//! 兜底策略：压缩后体积不小于原文件时，直接保留原始字节（复制源文件），
//! 保证「压缩」永远不会让文件变大。

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType, ImageEncoder};
use pdfium_render::prelude::{
    PdfPageIndex, PdfPageImageObject, PdfPageObjectsCommon, PdfPagePaperSize, PdfPoints,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::pdf::export_images::{render_page, DEFAULT_MAX_EDGE_PX};
use crate::pdf::types::{OutputFile, ProgressSink, TaskCtx};
use crate::pdf::engine;

/// 默认渲染分辨率：屏幕阅读足够，体积收益明显。
pub const DEFAULT_DPI: u32 = 150;

/// 默认 JPEG 质量。
pub const DEFAULT_QUALITY: u8 = 70;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressOptions {
    /// 源 PDF 绝对路径
    pub source: PathBuf,
    /// 源文档密码；为空时回退到任务上下文的密码
    pub password: Option<String>,
    /// 栅格化 DPI，取值被夹在 `[72, 600]`
    pub dpi: u32,
    /// JPEG 质量（1-100）
    pub quality: u8,
    /// 输出文件名（含 .pdf）
    pub output_name: Option<String>,
}

impl Default for CompressOptions {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            password: None,
            dpi: DEFAULT_DPI,
            quality: DEFAULT_QUALITY,
            output_name: None,
        }
    }
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &CompressOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    if opts.source.as_os_str().is_empty() {
        return Err(AppError::invalid("请先选择一个 PDF 文件"));
    }

    let dpi = opts.dpi.clamp(72, 600);
    let quality = opts.quality.clamp(1, 100);
    let original_size = std::fs::metadata(&opts.source)?.len();

    let pdfium = engine::get()?;
    let password = opts.password.clone().or_else(|| ctx.password.clone());
    let document = pdfium.load_pdf_from_file(&opts.source, password.as_deref())?;

    let total = document.pages().len().max(0) as usize;
    if total == 0 {
        return Err(AppError::Pdf("文档中没有任何页面".to_string()));
    }

    let mut compressed = pdfium.create_new_pdf()?;
    for index in 0..total {
        ctx.check_cancel()?;
        progress(index, total, &format!("正在压缩第 {} 页", index + 1));

        let page = document.pages().get(index as PdfPageIndex)?;
        let (width, height) = (page.width().value, page.height().value);
        let image = render_page(&page, dpi, DEFAULT_MAX_EDGE_PX)?;
        let jpeg = jpeg_bytes(&image, quality)?;

        // 新页与原始页面同尺寸（含旋转后的可视尺寸），JPEG 铺满整页
        let paper = PdfPagePaperSize::from_points(PdfPoints::new(width), PdfPoints::new(height));
        let mut new_page = compressed.pages_mut().create_page_at_end(paper)?;
        let mut object = PdfPageImageObject::new_from_jpeg_reader(&compressed, Cursor::new(jpeg))?;
        object.scale(width, height)?;
        new_page.objects_mut().add_image_object(object)?;
    }

    ctx.check_cancel()?;
    progress(total, total, "正在写出 PDF");

    let stem = crate::pdf::file_stem(&opts.source);
    let fallback = format!("{stem}_compressed.pdf");
    let out_path = crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
    crate::pdf::save_document(&compressed, &out_path, Some(&opts.source))?;

    // 兜底：压缩后反而更大（常见于纯文字 / 矢量文档）时保留原始字节
    let new_size = std::fs::metadata(&out_path)?.len();
    let summary = if new_size >= original_size {
        std::fs::copy(&opts.source, &out_path)?;
        format!(
            "压缩后体积未减小（{}），已保留原始文件",
            human_size(original_size)
        )
    } else {
        let percent = (100.0 * (1.0 - new_size as f64 / original_size as f64)).round() as u32;
        format!(
            "压缩完成：{} → {}（减小 {percent}%）",
            human_size(original_size),
            human_size(new_size)
        )
    };
    progress(total, total, &summary);
    // 完成摘要随 finished 事件下发，结果区常驻展示压缩成果
    ctx.set_summary(summary);

    Ok(vec![OutputFile::pdf(out_path, Some(total as u32))?])
}

/// 把渲染结果编码为内存中的 JPEG 字节（供 `new_from_jpeg_reader` 直接嵌入）。
fn jpeg_bytes(image: &DynamicImage, quality: u8) -> Result<Vec<u8>, AppError> {
    // JPEG 不支持 alpha 通道，必须先转 RGB
    let rgb = image.to_rgb8();
    let mut buffer = Vec::new();
    JpegEncoder::new_with_quality(&mut buffer, quality).write_image(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        ExtendedColorType::Rgb8,
    )?;
    Ok(buffer)
}

/// 人类可读的体积文案（进度消息用）。纯函数，便于单元测试。
fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let value = bytes as f64;
    if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    #[test]
    fn default_options_are_usable() {
        let opts = CompressOptions::default();
        assert_eq!(opts.dpi, DEFAULT_DPI);
        assert_eq!(opts.quality, DEFAULT_QUALITY);
    }

    #[test]
    fn human_size_picks_reasonable_unit() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn jpeg_bytes_produces_decodable_jpeg() {
        let buffer = ImageBuffer::from_pixel(16, 16, Rgb([120u8, 60, 200]));
        let bytes = jpeg_bytes(&DynamicImage::ImageRgb8(buffer), 70).expect("编码应当成功");
        assert!(bytes.starts_with(&[0xFF, 0xD8]), "应当以 JPEG SOI 开头");
        let decoded = image::load_from_memory(&bytes).expect("应当能解码");
        assert_eq!((decoded.width(), decoded.height()), (16, 16));
    }
}