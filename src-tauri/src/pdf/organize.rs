//! 可视化页面整理：按用户指定的页面顺序与单页旋转角重建 PDF（支持删页、拖拽换序、单页旋转）。

use std::path::{Path, PathBuf};

use pdfium_render::prelude::{PdfPageIndex, PdfPageRenderRotation};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::pdf::engine;
use crate::pdf::info::{rotation_degrees, rotation_from_degrees};
use crate::pdf::rotate::normalize_rotation;
use crate::pdf::types::{OutputFile, ProgressSink, TaskCtx};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizePageSpec {
    /// 源文档中的 0-based 页索引
    pub page_index: u32,
    /// 在原页面旋转角基础上叠加的顺时针角度（0 / 90 / 180 / 270）
    #[serde(default)]
    pub rotate_deg: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeOptions {
    pub source: PathBuf,
    pub password: Option<String>,
    /// 保留并重排后的页面列表；顺序即输出顺序
    pub pages: Vec<OrganizePageSpec>,
    pub output_name: Option<String>,
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &OrganizeOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    if opts.source.as_os_str().is_empty() {
        return Err(AppError::invalid("请先选择一个 PDF 文件"));
    }
    if opts.pages.is_empty() {
        return Err(AppError::invalid("至少需要保留 1 页才能导出"));
    }

    let pdfium = engine::get()?;
    let password = opts.password.clone().or_else(|| ctx.password.clone());
    let source_doc = pdfium.load_pdf_from_file(&opts.source, password.as_deref())?;

    let src_total = source_doc.pages().len() as u32;
    if src_total == 0 {
        return Err(AppError::Pdf("文档中没有任何页面".to_string()));
    }

    for spec in &opts.pages {
        if spec.page_index >= src_total {
            return Err(AppError::Range(format!(
                "页索引 {} 超出文档页数（共 {} 页）",
                spec.page_index + 1,
                src_total
            )));
        }
        rotation_from_degrees(spec.rotate_deg)?;
    }

    let mut target = pdfium.create_new_pdf()?;
    let out_total = opts.pages.len();

    for (dst_idx, spec) in opts.pages.iter().enumerate() {
        ctx.check_cancel()?;
        progress(
            dst_idx,
            out_total,
            &format!("正在整理第 {}/{} 页", dst_idx + 1, out_total),
        );

        target
            .pages_mut()
            .copy_page_from_document(&source_doc, spec.page_index as PdfPageIndex, dst_idx as PdfPageIndex)?;

        if spec.rotate_deg.rem_euclid(360) != 0 {
            let mut page = target.pages_mut().get(dst_idx as PdfPageIndex)?;
            let current = rotation_degrees(page.rotation().unwrap_or(PdfPageRenderRotation::None));
            let next = normalize_rotation(current as i32 + spec.rotate_deg);
            page.set_rotation(rotation_from_degrees(next as i32)?);
        }
    }

    ctx.check_cancel()?;
    progress(out_total, out_total, "正在写出 PDF");

    let stem = crate::pdf::file_stem(&opts.source);
    let fallback = format!("{stem}_organized.pdf");
    let out_path = crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
    crate::pdf::save_document(&target, &out_path, Some(&opts.source))?;

    let deleted = src_total.saturating_sub(out_total as u32);
    let summary = if deleted > 0 {
        format!("整理完成：保留 {out_total} 页（已删 {deleted} 页）")
    } else {
        format!("整理完成：共 {out_total} 页")
    };
    progress(out_total, out_total, &summary);
    ctx.set_summary(summary);

    Ok(vec![OutputFile::pdf(out_path, Some(out_total as u32))?])
}
