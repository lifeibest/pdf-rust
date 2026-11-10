//! 发票合并：把多张（通常是 A5 的）发票按 N-up 网格拼版到一张 A4 上，方便打印。
//!
//! 与 `merge` 的区别：`merge` 是整页原尺寸顺序拼接，无法缩放定位；这里把每张发票
//! 先栅格化为位图（PDF 逐页渲染、图片直接解码），再等比缩放居中贴到 A4 页面的网格
//! 单元里，从而实现「2 张 A5 → 1 张 A4」这类排版。支持一次合并任意多张发票：
//! 每 `per_sheet` 张排满一页 A4，超出部分自动续到下一页。

use std::path::Path;

use image::DynamicImage;
use pdfium_render::prelude::{
    PdfPageImageObject, PdfPageObjectsCommon, PdfPagePaperSize, PdfPoints,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::pdf::export_images::{self, DEFAULT_MAX_EDGE_PX};
use crate::pdf::merge::MergeSource;
use crate::pdf::types::{
    ImageFitMode, OrientationOpt, OutputFile, PageSizeOpt, ProgressSink, TaskCtx,
};
use crate::pdf::{engine, imaging};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceMergeOptions {
    /// 发票来源，顺序即拼版顺序；PDF 的每一页都算作一张发票
    pub sources: Vec<MergeSource>,
    /// 每张 A4 上排布几张发票，支持 1 / 2 / 4，其余值向 4 归一
    pub per_sheet: u8,
    /// 页面方向；`auto` 依据首张发票的宽高比自动选择，让发票尽量占满单元格
    pub orientation: OrientationOpt,
    /// 每张发票四周留白（毫米），同时充当相邻发票之间的裁切间隙
    pub margin_mm: f32,
    /// PDF 发票的渲染 DPI，取值被夹在 `[24, 1200]`
    pub dpi: u32,
    /// 输出文件名（含 .pdf）
    pub output_name: Option<String>,
}

impl Default for InvoiceMergeOptions {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            per_sheet: 2,
            orientation: OrientationOpt::Auto,
            margin_mm: 5.0,
            dpi: 200,
            output_name: None,
        }
    }
}

/// 把 `per_sheet` 归一到受支持的档位（1 / 2 / 4）。
fn normalize_per_sheet(value: u8) -> u8 {
    if value <= 1 {
        1
    } else if value <= 2 {
        2
    } else {
        4
    }
}

/// 依据每页张数与页面方向决定网格 (列数, 行数)。
///
/// 纵向 A4 上下堆叠（2 张时 1 列 2 行）适合横向发票；横向 A4 左右并排
/// （2 张时 2 列 1 行）适合纵向发票；4 张统一 2×2。
fn grid_for(per_sheet: u8, page_landscape: bool) -> (usize, usize) {
    match per_sheet {
        1 => (1, 1),
        2 => {
            if page_landscape {
                (2, 1)
            } else {
                (1, 2)
            }
        }
        _ => (2, 2),
    }
}

/// 计算某个槽位在页面上的单元格矩形 `(x, y, w, h)`，坐标原点为页面左下角（PDF 用户空间）。
///
/// `slot` 从 0 开始，按「先行后列」填充：`row = slot / cols`，`col = slot % cols`，
/// 第 0 行位于页面顶部。
fn cell_rect(page_height: f32, cell_w: f32, cell_h: f32, cols: usize, slot: usize) -> (f32, f32, f32, f32) {
    let cols = cols.max(1);
    let col = slot % cols;
    let row = slot / cols;
    let x = col as f32 * cell_w;
    let y = page_height - (row + 1) as f32 * cell_h;
    (x, y, cell_w, cell_h)
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &InvoiceMergeOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    if opts.sources.is_empty() {
        return Err(AppError::invalid("请至少选择一个发票文件"));
    }

    let pdfium = engine::get()?;
    let per_sheet = normalize_per_sheet(opts.per_sheet);
    let per = per_sheet as usize;

    // 1. 先把所有发票页栅格化为位图：图片直接解码，PDF 逐页渲染。
    //    源文档在此阶段用完即弃，避免与目标文档同时持有借用。
    let mut invoices: Vec<DynamicImage> = Vec::new();
    for source in &opts.sources {
        ctx.check_cancel()?;
        let path = source.path.as_path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        progress(invoices.len(), 0, &format!("正在读取 {name}"));

        if imaging::is_image(path) {
            invoices.push(imaging::load_image(path)?);
        } else if imaging::is_pdf(path) {
            let password = source.password.clone().or_else(|| ctx.password.clone());
            let document = pdfium.load_pdf_from_file(path, password.as_deref())?;
            for page in document.pages().iter() {
                ctx.check_cancel()?;
                let image = export_images::render_page(&page, opts.dpi, DEFAULT_MAX_EDGE_PX)?;
                invoices.push(image);
            }
        } else {
            return Err(AppError::Unsupported(format!("不支持的文件类型：{name}")));
        }
    }

    if invoices.is_empty() {
        return Err(AppError::Pdf("没有可合并的发票页面，请检查输入文件".to_string()));
    }

    // 2. 决定纸张方向与网格。`auto` 时横向发票用纵向 A4（上下堆叠），纵向发票用横向 A4（左右并排）。
    let base = PageSizeOpt::A4.resolve(PdfPagePaperSize::a4());
    let page_landscape = match opts.orientation {
        OrientationOpt::Portrait => false,
        OrientationOpt::Landscape => true,
        OrientationOpt::Auto => {
            let first = &invoices[0];
            !imaging::is_landscape(first.width(), first.height())
        }
    };
    let paper = if page_landscape {
        base.landscape()
    } else {
        base.portrait()
    };

    let (cols, rows) = grid_for(per_sheet, page_landscape);
    let page_width = paper.width().value;
    let page_height = paper.height().value;
    let cell_w = page_width / cols as f32;
    let cell_h = page_height / rows as f32;

    // 3. 逐页拼版：每 per 张发票排满一张 A4，超出自动续页。
    let mut target = pdfium.create_new_pdf()?;
    let total = invoices.len();
    let sheet_count = total.div_ceil(per);

    for (sheet, chunk) in invoices.chunks(per).enumerate() {
        ctx.check_cancel()?;
        progress(
            sheet * per,
            total,
            &format!("正在拼版第 {}/{} 页 A4", sheet + 1, sheet_count),
        );

        let mut page = target.pages_mut().create_page_at_end(paper)?;
        for (slot, image) in chunk.iter().enumerate() {
            let (cell_x, cell_y, cw, ch) = cell_rect(page_height, cell_w, cell_h, cols, slot);
            // 以单元格为「页面」计算等比居中落位，margin 同时充当发票之间的间隙
            let placement = imaging::place_image(
                cw,
                ch,
                opts.margin_mm,
                image.width(),
                image.height(),
                ImageFitMode::Fit,
                opts.dpi,
            );

            let mut object = PdfPageImageObject::new_with_size(
                &target,
                image,
                PdfPoints::new(placement.width),
                PdfPoints::new(placement.height),
            )?;
            // 顺序很重要：先缩放，再平移到「单元格原点 + 单元格内落位」。
            object.translate(
                PdfPoints::new(cell_x + placement.x),
                PdfPoints::new(cell_y + placement.y),
            )?;
            page.objects_mut().add_image_object(object)?;
        }
    }

    ctx.check_cancel()?;
    progress(total, total, "正在写出 PDF");

    let fallback = format!(
        "{}_invoice.pdf",
        crate::pdf::file_stem(&opts.sources[0].path)
    );
    let out_path = crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
    crate::pdf::save_document(&target, &out_path, None)?;

    Ok(vec![OutputFile::pdf(out_path, Some(sheet_count as u32))?])
}

#[cfg(test)]
mod tests {
    use super::*;

    // A4: 595.276 x 841.89 pt
    const A4_W: f32 = 595.276;
    const A4_H: f32 = 841.89;

    #[test]
    fn per_sheet_normalizes_to_supported_steps() {
        assert_eq!(normalize_per_sheet(0), 1);
        assert_eq!(normalize_per_sheet(1), 1);
        assert_eq!(normalize_per_sheet(2), 2);
        assert_eq!(normalize_per_sheet(3), 4);
        assert_eq!(normalize_per_sheet(4), 4);
        assert_eq!(normalize_per_sheet(9), 4);
    }

    #[test]
    fn grid_matches_orientation() {
        assert_eq!(grid_for(1, false), (1, 1));
        assert_eq!(grid_for(1, true), (1, 1));
        // 纵向页：上下堆叠；横向页：左右并排
        assert_eq!(grid_for(2, false), (1, 2));
        assert_eq!(grid_for(2, true), (2, 1));
        assert_eq!(grid_for(4, false), (2, 2));
        assert_eq!(grid_for(4, true), (2, 2));
    }

    #[test]
    fn two_up_portrait_stacks_top_then_bottom() {
        let (cols, rows) = grid_for(2, false);
        let cell_w = A4_W / cols as f32;
        let cell_h = A4_H / rows as f32;

        // 第 0 槽在顶部，第 1 槽在底部，二者水平方向铺满整页宽
        let (x0, y0, w0, h0) = cell_rect(A4_H, cell_w, cell_h, cols, 0);
        assert!((x0 - 0.0).abs() < 1e-3);
        assert!((y0 - cell_h).abs() < 1e-3);
        assert!((w0 - A4_W).abs() < 1e-3);
        assert!((h0 - cell_h).abs() < 1e-3);

        let (x1, y1, _, _) = cell_rect(A4_H, cell_w, cell_h, cols, 1);
        assert!((x1 - 0.0).abs() < 1e-3);
        assert!(y1.abs() < 1e-3, "第二张应贴到页面底部");
    }

    #[test]
    fn four_up_fills_quadrants_without_overlap() {
        let (cols, rows) = grid_for(4, false);
        let cell_w = A4_W / cols as f32;
        let cell_h = A4_H / rows as f32;

        let mut seen = Vec::new();
        for slot in 0..4 {
            let (x, y, w, h) = cell_rect(A4_H, cell_w, cell_h, cols, slot);
            assert!(x >= -1e-3 && y >= -1e-3);
            assert!(x + w <= A4_W + 1e-3);
            assert!(y + h <= A4_H + 1e-3);
            seen.push((x.round(), y.round()));
        }
        seen.sort_by(|a, b| a.partial_cmp(b).unwrap());
        seen.dedup();
        assert_eq!(seen.len(), 4, "四个单元格应互不重合");
    }
}
