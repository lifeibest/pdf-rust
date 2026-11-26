//! 图片转 PDF：每张图片生成一页。
//!
//! 同时被「图片转 PDF」功能与「合并」功能中的图片来源复用。

use std::io::Cursor;
use std::path::{Path, PathBuf};

use pdfium_render::prelude::{
    PdfDocument, PdfPageImageObject, PdfPageObjectsCommon, PdfPagePaperSize, PdfPoints,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::pdf::types::{
    ImageEnhanceOpt, ImageFitMode, OrientationOpt, OutputFile, PageSizeOpt, ProgressSink, TaskCtx,
};
use crate::pdf::{engine, imaging};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagesToPdfOptions {
    /// 图片绝对路径，顺序即页面顺序
    pub sources: Vec<PathBuf>,
    /// 页面尺寸；`auto` 表示按图片自身像素与 `image_dpi` 推导
    pub page_size: PageSizeOpt,
    /// 页面方向；`auto` 表示依据图片宽高比
    pub orientation: OrientationOpt,
    /// 图片在页面上的排版方式
    pub image_fit: ImageFitMode,
    /// 四边留白（毫米）
    pub margin_mm: f32,
    /// `auto` 页面尺寸与 `native_dpi` 排版模式共用的像素-点换算基准
    pub image_dpi: u32,
    /// 手机拍图文档画面增强：原图 / 去灰底漂白 / 黑白扫描
    #[serde(default)]
    pub enhance: ImageEnhanceOpt,
    /// 输出文件名（含 .pdf）；为空时按首个图片名推导
    pub output_name: Option<String>,
}

impl Default for ImagesToPdfOptions {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            page_size: PageSizeOpt::A4,
            orientation: OrientationOpt::Auto,
            image_fit: ImageFitMode::Fit,
            margin_mm: 0.0,
            image_dpi: 96,
            enhance: ImageEnhanceOpt::None,
            output_name: None,
        }
    }
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &ImagesToPdfOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    if opts.sources.is_empty() {
        return Err(AppError::invalid("请至少选择一张图片"));
    }

    for source in &opts.sources {
        if !imaging::is_image(source) {
            return Err(AppError::Unsupported(format!(
                "不支持的图片格式：{}",
                source.display()
            )));
        }
    }

    let pdfium = engine::get()?;
    let mut document = pdfium.create_new_pdf()?;
    let total = opts.sources.len();

    for (index, source) in opts.sources.iter().enumerate() {
        ctx.check_cancel()?;
        let name = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| source.display().to_string());
        progress(index, total, &format!("正在排版 {name}"));

        append_image_page(&mut document, source, opts)?;
    }

    ctx.check_cancel()?;
    progress(total, total, "正在写出 PDF");

    let fallback = format!("{}_converted.pdf", imaging_stem(&opts.sources[0]));
    let out_path = crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
    crate::pdf::save_document(&document, &out_path, None)?;

    Ok(vec![OutputFile::pdf(out_path, Some(total as u32))?])
}

/// 向文档末尾追加一页，并把图片按选项排版到该页上。
///
/// 供「图片转 PDF」与「合并」共用，因此 `document` 由调用方持有。
/// 对无需旋转的标准 JPEG 直接内嵌原始 JPEG 字节流，避免解成位图再写回导致 PDF 体积暴涨 5~10 倍。
pub fn append_image_page(
    document: &mut PdfDocument<'_>,
    image_path: &Path,
    opts: &ImagesToPdfOptions,
) -> Result<(), AppError> {
    // 尝试 JPEG 零重编码直嵌（仅当不开启画面增强、扩展名为 jpg/jpeg 且 EXIF 方向为 1 时启用）
    let raw_jpeg = if opts.enhance == ImageEnhanceOpt::None {
        try_load_passthrough_jpeg(image_path)
    } else {
        None
    };

    let (image_width, image_height, decoded_image) = match &raw_jpeg {
        Some((_, w, h)) => (*w, *h, None),
        None => {
            let mut image = imaging::load_image(image_path)?;
            if opts.enhance != ImageEnhanceOpt::None {
                image = imaging::enhance_document_image(&image, opts.enhance);
            }
            let (w, h) = (image.width(), image.height());
            (w, h, Some(image))
        }
    };

    let base = match opts.page_size {
        PageSizeOpt::Auto => {
            auto_paper_size(image_width, image_height, opts.image_dpi, opts.margin_mm)
        }
        fixed => fixed.resolve(PdfPagePaperSize::a4()),
    };

    let paper = opts
        .orientation
        .apply(base, imaging::is_landscape(image_width, image_height));

    let page_width = paper.width().value;
    let page_height = paper.height().value;
    let placement = imaging::place_image(
        page_width,
        page_height,
        opts.margin_mm,
        image_width,
        image_height,
        opts.image_fit,
        opts.image_dpi,
    );

    let mut page = document.pages_mut().create_page_at_end(paper)?;

    let mut object = if let Some((jpeg_bytes, _, _)) = raw_jpeg {
        let mut obj = PdfPageImageObject::new_from_jpeg_reader(document, Cursor::new(jpeg_bytes))?;
        obj.scale(placement.width, placement.height)?;
        obj
    } else {
        let image = decoded_image.expect("非直嵌分支一定已解码图片");
        PdfPageImageObject::new_with_size(
            document,
            &image,
            PdfPoints::new(placement.width),
            PdfPoints::new(placement.height),
        )?
    };

    // 顺序很重要：先缩放，再平移。若先平移，平移量会被缩放一并放大。
    object.translate(PdfPoints::new(placement.x), PdfPoints::new(placement.y))?;
    page.objects_mut().add_image_object(object)?;

    Ok(())
}

/// 若文件是标准方向（EXIF orientation == 1）的 JPEG，返回 `(原始字节, 宽, 高)` 供直接内嵌。
fn try_load_passthrough_jpeg(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    if !matches!(imaging::extension_of(path).as_str(), "jpg" | "jpeg") {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    if imaging::jpeg_exif_orientation(&bytes) != 1 {
        return None;
    }
    let reader = image::ImageReader::with_format(Cursor::new(&bytes), image::ImageFormat::Jpeg);
    let (w, h) = reader.into_dimensions().ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    Some((bytes, w, h))
}

/// `auto` 页面尺寸：图片像素按 `dpi` 换算为点，再加上四边留白。
fn auto_paper_size(
    image_width: u32,
    image_height: u32,
    dpi: u32,
    margin_mm: f32,
) -> PdfPagePaperSize {
    let dpi = if dpi == 0 { 96 } else { dpi } as f32;
    let margin_pt = margin_mm.max(0.0) * imaging::POINTS_PER_MM;

    let width = image_width.max(1) as f32 * imaging::POINTS_PER_INCH / dpi + 2.0 * margin_pt;
    let height = image_height.max(1) as f32 * imaging::POINTS_PER_INCH / dpi + 2.0 * margin_pt;

    PdfPagePaperSize::from_points(
        PdfPoints::new(width.max(1.0)),
        PdfPoints::new(height.max(1.0)),
    )
}

fn imaging_stem(path: &Path) -> String {
    crate::pdf::file_stem(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_paper_size_converts_pixels_with_margin() {
        // 96 dpi 下 96px = 72pt；两侧各 10mm 留白 = 2 * 28.346pt
        let paper = auto_paper_size(96, 192, 96, 10.0);
        let margin_pt = 10.0 * imaging::POINTS_PER_MM;

        assert!((paper.width().value - (72.0 + 2.0 * margin_pt)).abs() < 1e-2);
        assert!((paper.height().value - (144.0 + 2.0 * margin_pt)).abs() < 1e-2);
    }

    #[test]
    fn auto_paper_size_falls_back_to_96_dpi_and_clamps_zero_pixels() {
        let paper = auto_paper_size(0, 0, 0, 0.0);
        assert!(paper.width().value > 0.0);
        assert!(paper.height().value > 0.0);

        let paper = auto_paper_size(96, 96, 0, 0.0);
        assert!((paper.width().value - 72.0).abs() < 1e-3);
    }
}
