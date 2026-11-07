//! PDF 合并：把多个 PDF 与图片按给定顺序合并为一个 PDF。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::pdf::images_to_pdf::{self, ImagesToPdfOptions};
use crate::pdf::types::{
    ImageFitMode, OrientationOpt, OutputFile, PageSizeOpt, ProgressSink, TaskCtx,
};
use crate::pdf::{engine, imaging};

/// 单个合并来源。图片不需要密码，`password` 仅对 PDF 生效。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeSource {
    pub path: PathBuf,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeOptions {
    /// 合并来源，顺序即输出页面顺序
    pub sources: Vec<MergeSource>,
    /// 页面尺寸，仅作用于图片来源；PDF 页面保持原始尺寸
    pub page_size: PageSizeOpt,
    /// 页面方向，仅作用于图片来源
    pub orientation: OrientationOpt,
    /// 图片排版方式，仅作用于图片来源
    pub image_fit: ImageFitMode,
    /// 图片页四边留白（毫米）
    pub margin_mm: f32,
    /// 图片像素到点的换算基准
    pub image_dpi: u32,
    /// 复制 PDF 页面前先把注释/表单压平，缓解 PDFium 跨文档复制丢失注释的问题
    pub flatten_annotations: bool,
    /// 输出文件名（含 .pdf）
    pub output_name: Option<String>,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            page_size: PageSizeOpt::A4,
            orientation: OrientationOpt::Auto,
            image_fit: ImageFitMode::Fit,
            margin_mm: 0.0,
            image_dpi: 96,
            flatten_annotations: false,
            output_name: None,
        }
    }
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &MergeOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    if opts.sources.is_empty() {
        return Err(AppError::invalid("请至少选择一个 PDF 或图片"));
    }

    let pdfium = engine::get()?;
    let mut target = pdfium.create_new_pdf()?;
    let total = opts.sources.len();

    // 图片来源复用「图片转 PDF」的排版逻辑，保证两个功能的输出表现一致
    let image_opts = ImagesToPdfOptions {
        sources: Vec::new(),
        page_size: opts.page_size,
        orientation: opts.orientation,
        image_fit: opts.image_fit,
        margin_mm: opts.margin_mm,
        image_dpi: opts.image_dpi,
        output_name: None,
    };

    for (index, source) in opts.sources.iter().enumerate() {
        ctx.check_cancel()?;

        let path = source.path.as_path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        progress(index, total, &format!("正在合并 {name}"));

        if imaging::is_image(path) {
            images_to_pdf::append_image_page(&mut target, path, &image_opts)?;
        } else if imaging::is_pdf(path) {
            let password = source.password.clone().or_else(|| ctx.password.clone());
            let source_document = pdfium.load_pdf_from_file(path, password.as_deref())?;

            if source_document.pages().is_empty() {
                log::warn!("跳过空文档: {}", path.display());
                continue;
            }

            if opts.flatten_annotations {
                for mut page in source_document.pages().iter() {
                    // 压平失败不应中断整个合并，只记录告警
                    if let Err(err) = page.flatten() {
                        log::warn!("压平注释失败 ({}): {err}", path.display());
                    }
                }
            }

            target.pages_mut().append(&source_document)?;
        } else {
            return Err(AppError::Unsupported(format!("不支持的文件类型：{name}")));
        }
    }

    if target.pages().is_empty() {
        return Err(AppError::Pdf("合并结果为空，请检查输入文件".to_string()));
    }

    ctx.check_cancel()?;
    progress(total, total, "正在写出 PDF");

    let fallback = format!(
        "{}_merged.pdf",
        crate::pdf::file_stem(&opts.sources[0].path)
    );
    let out_path = crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
    let page_count = target.pages().len().max(0) as u32;
    crate::pdf::save_document(&target, &out_path, None)?;

    Ok(vec![OutputFile::pdf(out_path, Some(page_count))?])
}
