//! 身份证 / 证件 A4 1:1 拼版：把 1~2 张证件照片（支持 JPG/PNG/HEIC）
//! 按国家标准身份证物理尺寸（85.6mm × 54.0mm）居中排布到一张标准 A4 纸上，
//! 可选黑白复印效果与防盗文字水印（如「仅供办理 XX 业务使用，他用无效」）。

use std::path::{Path, PathBuf};

use pdfium_render::prelude::{PdfPageImageObject, PdfPageObjectsCommon, PdfPagePaperSize, PdfPoints};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::pdf::types::{ImageEnhanceOpt, OutputFile, PageSizeOpt, ProgressSink, TaskCtx};
use crate::pdf::watermark::{
    self, WatermarkColor, WatermarkLayout, WatermarkStyle,
};
use crate::pdf::{engine, imaging};

/// 标准身份证物理宽度（毫米，ISO/IEC 7810 ID-1）
const ID_CARD_WIDTH_MM: f32 = 85.6;
/// 标准身份证物理高度（毫米）
const ID_CARD_HEIGHT_MM: f32 = 54.0;
/// 正反面两张卡之间的垂直间距（毫米）
const CARD_GAP_MM: f32 = 16.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdCardOptions {
    /// 证件照片路径（1~2 张：正面 / 反面）
    pub sources: Vec<PathBuf>,
    /// 画面效果：原图 / 去灰底漂白 / 黑白复印效果
    #[serde(default)]
    pub enhance: ImageEnhanceOpt,
    /// 可选防盗水印文字；留空则不加水印
    #[serde(default)]
    pub watermark_text: Option<String>,
    /// 水印不透明度（0.05 ~ 1.0）
    #[serde(default = "default_watermark_opacity")]
    pub watermark_opacity: f32,
    /// 输出文件名（含 .pdf）
    pub output_name: Option<String>,
}

fn default_watermark_opacity() -> f32 {
    0.25
}

impl Default for IdCardOptions {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            enhance: ImageEnhanceOpt::None,
            watermark_text: None,
            watermark_opacity: default_watermark_opacity(),
            output_name: None,
        }
    }
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &IdCardOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    if opts.sources.is_empty() {
        return Err(AppError::invalid("请至少选择一张证件照片"));
    }
    if opts.sources.len() > 2 {
        return Err(AppError::invalid("一次最多支持拼版 2 张证件照（正面与反面）"));
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
    let a4 = PageSizeOpt::A4.resolve(PdfPagePaperSize::a4()).portrait();
    let page_w = a4.width().value;
    let page_h = a4.height().value;

    let card_w = ID_CARD_WIDTH_MM * imaging::POINTS_PER_MM;
    let card_h = ID_CARD_HEIGHT_MM * imaging::POINTS_PER_MM;
    let gap_pt = CARD_GAP_MM * imaging::POINTS_PER_MM;

    let count = opts.sources.len();
    let total_h = card_h * count as f32 + gap_pt * (count.saturating_sub(1)) as f32;
    let start_top_y = (page_h + total_h) / 2.0;

    let mut page = document.pages_mut().create_page_at_end(a4)?;

    for (index, source) in opts.sources.iter().enumerate() {
        ctx.check_cancel()?;
        progress(index, count, &format!("正在处理第 {} 张证件照", index + 1));

        let mut img = imaging::load_image(source)?;
        // 竖拍的证件照自动顺时针旋转 90° 摆正为横向，贴合身份证物理比例
        if img.height() > img.width() {
            img = img.rotate90();
        }
        if opts.enhance != ImageEnhanceOpt::None {
            img = imaging::enhance_document_image(&img, opts.enhance);
        }

        // 在 85.6×54mm 卡片框内等比完整放入并居中
        let scale = (card_w / img.width().max(1) as f32).min(card_h / img.height().max(1) as f32);
        let draw_w = img.width().max(1) as f32 * scale;
        let draw_h = img.height().max(1) as f32 * scale;

        let slot_top = start_top_y - index as f32 * (card_h + gap_pt);
        let slot_bottom = slot_top - card_h;
        let x = (page_w - draw_w) / 2.0;
        let y = slot_bottom + (card_h - draw_h) / 2.0;

        let mut object = PdfPageImageObject::new_with_size(
            &document,
            &img,
            PdfPoints::new(draw_w),
            PdfPoints::new(draw_h),
        )?;
        object.translate(PdfPoints::new(x), PdfPoints::new(y))?;
        page.objects_mut().add_image_object(object)?;
    }

    // 可选防盗水印：平铺叠加在整页 A4 上，保护证件不被挪作他用
    if let Some(wm_text) = opts.watermark_text.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        ctx.check_cancel()?;
        progress(count, count, "正在添加防盗水印");
        let (px_w, px_h, dpi_scale) = watermark::watermark_canvas_size(page_w, page_h);
        let style = WatermarkStyle {
            text: wm_text.to_string(),
            layout: WatermarkLayout::Tile,
            color: WatermarkColor::Gray,
            font_size: 24.0 * dpi_scale,
            opacity: opts.watermark_opacity.clamp(0.05, 1.0),
            angle: -30.0,
        };
        let layer = watermark::render_watermark_layer(px_w, px_h, &style)?;
        let wm_obj = PdfPageImageObject::new_with_size(
            &document,
            &layer,
            PdfPoints::new(page_w),
            PdfPoints::new(page_h),
        )?;
        page.objects_mut().add_image_object(wm_obj)?;
    }

    ctx.check_cancel()?;
    progress(count, count, "正在写出 PDF");

    let fallback = format!("{}_idcard.pdf", crate::pdf::file_stem(&opts.sources[0]));
    let out_path = crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
    crate::pdf::save_document(&document, &out_path, None)?;

    let summary = format!("已按 1:1 尺寸将 {count} 张证件照排布到 A4 页面");
    progress(count, count, &summary);
    ctx.set_summary(summary);

    Ok(vec![OutputFile::pdf(out_path, Some(1))?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_are_usable() {
        let opts = IdCardOptions::default();
        assert!(opts.sources.is_empty());
        assert_eq!(opts.enhance, ImageEnhanceOpt::None);
        assert!(opts.watermark_opacity > 0.0 && opts.watermark_opacity <= 1.0);
    }
}
