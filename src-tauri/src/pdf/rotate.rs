//! PDF 旋转：按 90 度的整数倍旋转指定页面，支持原地保存或另存为新文件。
//!
//! 旋转量与页面已有的 `/Rotate` 叠加后归一化到 `[0, 360)`，因此对同一文档反复旋转
//! 不会累积出 450 度这类非法值。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pdfium_render::prelude::{PdfPageIndex, PdfPageRenderRotation};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::pdf::engine;
use crate::pdf::info::{rotation_degrees, rotation_from_degrees};
use crate::pdf::ranges;
use crate::pdf::types::{OutputFile, PageScope, ProgressSink, TaskCtx};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotateOptions {
    /// 源 PDF 绝对路径
    pub source: PathBuf,
    /// 源文档密码；为空时回退到任务上下文的密码
    pub password: Option<String>,
    /// 顺时针旋转角度，必须是 90 的整数倍；负值表示逆时针
    pub degrees: i32,
    /// 作用页范围
    pub scope: PageScope,
    /// 为真时直接覆盖源文件（经由临时文件原子替换，不会边读边写损坏源文件）
    pub in_place: bool,
    /// 另存时的输出文件名（含 .pdf）
    pub output_name: Option<String>,
}

impl Default for RotateOptions {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            password: None,
            degrees: 90,
            scope: PageScope::All,
            in_place: false,
            output_name: None,
        }
    }
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &RotateOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    if opts.source.as_os_str().is_empty() {
        return Err(AppError::invalid("请先选择一个 PDF 文件"));
    }

    // 先校验角度，避免旋转了大半文档才报参数错误
    let delta = rotation_from_degrees(opts.degrees)?;
    if matches!(delta, PdfPageRenderRotation::None) {
        return Err(AppError::invalid("旋转角度为 0，没有需要处理的页面"));
    }

    let pdfium = engine::get()?;
    let password = opts.password.clone().or_else(|| ctx.password.clone());
    let mut document = pdfium.load_pdf_from_file(&opts.source, password.as_deref())?;

    let total = document.pages().len().max(0) as usize;
    if total == 0 {
        return Err(AppError::Pdf("文档中没有任何页面".to_string()));
    }

    let selected = ranges::parse_ranges(&opts.scope.as_spec(), total)?;
    let targets: HashSet<usize> = selected.into_iter().collect();

    let mut rotated = 0usize;
    for index in 0..total {
        ctx.check_cancel()?;

        if !targets.contains(&(index + 1)) {
            continue;
        }

        let mut page = document.pages_mut().get(index as PdfPageIndex)?;
        let current = rotation_degrees(page.rotation().unwrap_or(PdfPageRenderRotation::None));
        // 与页面已有角度叠加后归一化，避免反复旋转累出 450 度这类非法值
        let next = normalize_rotation(current as i32 + opts.degrees);
        page.set_rotation(rotation_from_degrees(next as i32)?);
        rotated += 1;

        progress(
            rotated,
            targets.len(),
            &format!("已旋转第 {} 页", index + 1),
        );
    }

    if rotated == 0 {
        return Err(AppError::Range("页码范围没有匹配到任何页面".to_string()));
    }

    ctx.check_cancel()?;
    progress(targets.len(), targets.len(), "正在写出 PDF");

    let stem = crate::pdf::file_stem(&opts.source);
    if opts.in_place {
        crate::pdf::save_document(&document, &opts.source, Some(&opts.source))?;
        return Ok(vec![OutputFile::pdf(&opts.source, Some(total as u32))?]);
    }

    let fallback = format!("{stem}_rotated.pdf");
    let out_path = crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
    crate::pdf::save_document(&document, &out_path, Some(&opts.source))?;

    Ok(vec![OutputFile::pdf(out_path, Some(total as u32))?])
}

/// 把任意角度归一化到 `[0, 360)` 且为 90 的整数倍；不足 90 度的余量向下舍去。
///
/// 调用方（[`run`]）已保证输入是 90 的整数倍，这里只做防御性处理。
pub fn normalize_rotation(degrees: i32) -> u16 {
    let normalized = degrees.rem_euclid(360);
    (normalized / 90 * 90) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_positive_and_negative_angles() {
        assert_eq!(normalize_rotation(90), 90);
        assert_eq!(normalize_rotation(180 + 180), 0);
        assert_eq!(normalize_rotation(450), 90);
        assert_eq!(normalize_rotation(-90), 270);
        assert_eq!(normalize_rotation(-450), 270);
        assert_eq!(normalize_rotation(0), 0);
    }

    #[test]
    fn stacks_with_existing_rotation() {
        // 已有 270 度再顺时针 90 度应回到 0 度
        assert_eq!(normalize_rotation(270 + 90), 0);
        assert_eq!(normalize_rotation(90 + 270), 0);
        assert_eq!(normalize_rotation(90 + 90), 180);
    }

    #[test]
    fn rounds_down_to_quarter_turn() {
        // 实际调用方已在 run() 开头校验过角度，这里的取整只是最后一道保险
        assert_eq!(normalize_rotation(95), 90);
        assert_eq!(normalize_rotation(179), 90);
        assert_eq!(normalize_rotation(265), 180);
        assert_eq!(normalize_rotation(-95), 180);
    }
}
