//! PDF 拆分：按固定页数、逐页、页码范围或提取模式把一个 PDF 拆成多个 PDF。
//!
//! 分组逻辑集中在纯函数 [`plan_groups`] 中，便于单元测试覆盖边界（页数不整除、
//! 单页文档、超大 N 等），而真正的页面复制由 [`copy_pages`] 完成。

use std::path::{Path, PathBuf};

use pdfium_render::prelude::{PdfDocument, PdfPageIndex};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::pdf::engine;
use crate::pdf::ranges;
use crate::pdf::types::{OutputFile, ProgressSink, TaskCtx};

/// 拆分模式。serde 采用内部标签（`{ "mode": "every_n", "size": 3 }`），
/// 与前端 `features/split/meta.ts` 的取值一一对应。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum SplitMode {
    /// 每 `size` 页产出一个文件；不足 `size` 页的尾组也会产出
    EveryN { size: usize },
    /// 每页产出一个文件
    EachPage,
    /// 按 `spec` 的每个逗号分组产出一个文件，如 `1-3,4-6` → 2 个文件
    ByRanges { spec: String },
    /// 只把 `spec` 命中的页面提取为单个文件（可重排顺序）
    Extract { spec: String },
}

impl Default for SplitMode {
    fn default() -> Self {
        SplitMode::EveryN { size: 1 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitOptions {
    /// 源 PDF 绝对路径
    pub source: PathBuf,
    /// 源文档密码；为空时回退到任务上下文的密码
    pub password: Option<String>,
    pub mode: SplitMode,
    /// 产出多于一个文件时是否打包为单个 zip；用户在界面上打开该开关后，
    /// 即使只产出一个文件也会打包，行为保持可预期
    pub zip: bool,
    /// zip 文件名（含 .zip）；仅在 `zip` 为真时生效
    pub output_name: Option<String>,
}

impl Default for SplitOptions {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            password: None,
            mode: SplitMode::default(),
            zip: false,
            output_name: None,
        }
    }
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &SplitOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    if opts.source.as_os_str().is_empty() {
        return Err(AppError::invalid("请先选择一个 PDF 文件"));
    }

    let pdfium = engine::get()?;
    let password = opts.password.clone().or_else(|| ctx.password.clone());
    let source_document = pdfium.load_pdf_from_file(&opts.source, password.as_deref())?;

    let total = source_document.pages().len().max(0) as usize;
    let groups = plan_groups(&opts.mode, total)?;
    if groups.is_empty() {
        return Err(AppError::Pdf("拆分结果为空，请检查页码范围".to_string()));
    }

    let stem = crate::pdf::file_stem(&opts.source);
    // 要求打包时先写入临时目录，最后统一压成 zip
    let work_dir = if opts.zip {
        Some(crate::pdf::unique_temp_dir(&ctx.cache_dir, "split")?)
    } else {
        None
    };
    let target_dir = work_dir.as_deref().unwrap_or(out_dir);

    let group_total = groups.len();
    let mut outputs = Vec::with_capacity(group_total);

    for (index, group) in groups.iter().enumerate() {
        ctx.check_cancel()?;
        let label = group_label(group);
        progress(index, group_total, &format!("正在导出 {label}"));

        let mut target = pdfium.create_new_pdf()?;
        copy_pages(&mut target, &source_document, group)?;

        let file_name = format!("{stem}_part{:03}_{label}.pdf", index + 1);
        let out_path = crate::pdf::resolve_batch_path(target_dir, &file_name);
        crate::pdf::save_document(&target, &out_path, None)?;
        outputs.push(OutputFile::pdf(&out_path, Some(group.len() as u32))?);
    }

    ctx.check_cancel()?;

    if let Some(dir) = &work_dir {
        progress(group_total, group_total, "正在打包 zip");
        let fallback = format!("{stem}_split.zip");
        let zip_path =
            crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
        let archive = crate::pdf::zip_directory(dir, &zip_path)?;
        crate::pdf::remove_dir_quiet(dir);
        return Ok(vec![archive]);
    }

    progress(group_total, group_total, "拆分完成");
    Ok(outputs)
}

/// 计算每种模式下的输出分组（1-based 页码列表，顺序即输出页面顺序）。
///
/// 这是纯函数，不触碰 PDFium，因此可以独立做边界测试。
pub fn plan_groups(mode: &SplitMode, total: usize) -> Result<Vec<Vec<usize>>, AppError> {
    if total == 0 {
        return Err(AppError::Pdf("文档中没有任何页面".to_string()));
    }

    let groups = match mode {
        SplitMode::EachPage => (1..=total).map(|page| vec![page]).collect(),
        SplitMode::EveryN { size } => ranges::chunk_pages(total, *size)
            .into_iter()
            .map(|(start, end)| (start..=end).collect::<Vec<usize>>())
            .collect(),
        SplitMode::ByRanges { spec } => ranges::parse_ranges_to_groups(spec, total)?,
        SplitMode::Extract { spec } => vec![ranges::parse_ranges(spec, total)?],
    };

    if groups.is_empty() {
        return Err(AppError::Range("页码范围没有匹配到任何页面".to_string()));
    }
    Ok(groups)
}

/// 把选中的页面复制到新文档。
///
/// 页码连续且升序时走一次区间复制（FFI 调用更少、速度更快），
/// 否则逐页复制，以支持乱序与不连续的选择（`Extract` 模式）。
fn copy_pages(
    target: &mut PdfDocument<'_>,
    source: &PdfDocument<'_>,
    pages: &[usize],
) -> Result<(), AppError> {
    if pages.is_empty() {
        return Err(AppError::Range("待复制的页面列表为空".to_string()));
    }

    let destination = target.pages().len() as PdfPageIndex;
    let contiguous = pages.windows(2).all(|window| window[1] == window[0] + 1);

    if contiguous {
        let start = (pages[0] - 1) as PdfPageIndex;
        let end = (pages[pages.len() - 1] - 1) as PdfPageIndex;
        target
            .pages_mut()
            .copy_page_range_from_document(source, start..=end, destination)?;
    } else {
        for page in pages {
            // 先把插入位置算出来，避免在同一个表达式里同时可变与不可变地借用 target
            let at = target.pages().len() as PdfPageIndex;
            target
                .pages_mut()
                .copy_page_from_document(source, (*page - 1) as PdfPageIndex, at)?;
        }
    }

    Ok(())
}

/// 分组标签，用于文件名与进度文案：`[1,2,3]` → `p1-3`，`[4]` → `p4`，
/// 不连续或乱序时退化为页数描述。
fn group_label(pages: &[usize]) -> String {
    match (pages.first(), pages.last()) {
        (Some(&first), Some(&last)) if last == first + pages.len() - 1 && pages.len() > 1 => {
            format!("p{first}-{last}")
        }
        (Some(&only), Some(&last)) if only == last && pages.len() == 1 => format!("p{only}"),
        _ => format!("{}p", pages.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_page_produces_one_group_per_page() {
        let groups = plan_groups(&SplitMode::EachPage, 3).expect("应当成功");
        assert_eq!(groups, vec![vec![1], vec![2], vec![3]]);
    }

    #[test]
    fn every_n_handles_indivisible_totals() {
        let groups = plan_groups(&SplitMode::EveryN { size: 2 }, 5).expect("应当成功");
        assert_eq!(groups, vec![vec![1, 2], vec![3, 4], vec![5]]);
    }

    #[test]
    fn every_n_with_oversized_size_yields_single_group() {
        let groups = plan_groups(&SplitMode::EveryN { size: 999 }, 4).expect("应当成功");
        assert_eq!(groups, vec![vec![1, 2, 3, 4]]);
    }

    #[test]
    fn every_n_with_zero_size_falls_back_to_per_page() {
        let groups = plan_groups(&SplitMode::EveryN { size: 0 }, 3).expect("应当成功");
        assert_eq!(groups, vec![vec![1], vec![2], vec![3]]);
    }

    #[test]
    fn by_ranges_keeps_token_grouping() {
        let mode = SplitMode::ByRanges {
            spec: "1-2,4".to_string(),
        };
        let groups = plan_groups(&mode, 6).expect("应当成功");
        assert_eq!(groups, vec![vec![1, 2], vec![4]]);
    }

    #[test]
    fn extract_flattens_all_matches_into_one_group() {
        let mode = SplitMode::Extract {
            spec: "5,1-2".to_string(),
        };
        let groups = plan_groups(&mode, 8).expect("应当成功");
        assert_eq!(groups, vec![vec![5, 1, 2]]);
    }

    #[test]
    fn rejects_empty_document_and_invalid_spec() {
        assert!(matches!(
            plan_groups(&SplitMode::EachPage, 0),
            Err(AppError::Pdf(_))
        ));
        assert!(matches!(
            plan_groups(&SplitMode::Extract { spec: "99".into() }, 3),
            Err(AppError::Range(_))
        ));
    }

    #[test]
    fn formats_group_labels() {
        assert_eq!(group_label(&[1, 2, 3]), "p1-3");
        assert_eq!(group_label(&[4]), "p4");
        assert_eq!(group_label(&[5, 1, 2]), "3p");
    }
}
