//! PDF 提取图片：抽取页面内嵌的原始图片对象（原始分辨率，不做页面栅格化）。
//!
//! 与 `export_images`（整页渲染）互补：这里遍历页面对象树（含 Form XObject 嵌套），
//! 把嵌入的图片按原始像素导出。`Auto` 格式下，仅经 DCTDecode 滤镜的 JPEG 直接
//! 写出原始字节流（无损、零重编码）；其余图片解码后编为 PNG。
//! 同一图片被多页引用时按原始数据哈希去重，只导出首次出现的一份。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pdfium_render::prelude::{
    PdfPageIndex, PdfPageImageObject, PdfPageObject, PdfPageObjectsCommon,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::AppError;
use crate::pdf::types::{ImageFormatOpt, OutputFile, PageScope, ProgressSink, TaskCtx};
use crate::pdf::{engine, imaging, ranges};

/// Form XObject 递归深度上限，防止畸形文档的循环引用导致无限递归。
const MAX_FORM_DEPTH: u32 = 8;

/// 宽或高低于该像素数的图片视为装饰性小点，直接跳过。
const MIN_EDGE_PX: i32 = 2;

/// 提取输出格式。`Auto` 保留原格式：JPEG 无损直写，其余统一 PNG。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExtractFormatOpt {
    #[default]
    Auto,
    Png,
    Jpeg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractImagesOptions {
    /// 源 PDF 绝对路径
    pub source: PathBuf,
    /// 源文档密码；为空时回退到任务上下文的密码
    pub password: Option<String>,
    /// 提取页范围
    pub scope: PageScope,
    /// 最小图片体积（KB），按嵌入原始数据字节数判断；0 表示不过滤
    pub min_size_kb: u32,
    pub format: ExtractFormatOpt,
    /// JPEG 质量（1-100）；仅在 `format` 为 `Jpeg` 时生效
    pub quality: u8,
    /// 是否把全部图片打包为单个 zip
    pub zip: bool,
    /// zip 文件名（含 .zip）；仅在 `zip` 为真时生效
    pub output_name: Option<String>,
}

impl Default for ExtractImagesOptions {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            password: None,
            scope: PageScope::All,
            min_size_kb: 0,
            format: ExtractFormatOpt::Auto,
            quality: 92,
            zip: false,
            output_name: None,
        }
    }
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &ExtractImagesOptions,
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

    let selected = ranges::parse_ranges(&opts.scope.as_spec(), total)?;
    let stem = crate::pdf::file_stem(&opts.source);

    let work_dir = if opts.zip {
        Some(crate::pdf::unique_temp_dir(&ctx.cache_dir, "extract")?)
    } else {
        None
    };
    let target_dir = work_dir.as_deref().unwrap_or(out_dir);
    // 无损直写分支不经 imaging::encode_image，目录需在这里先行创建
    std::fs::create_dir_all(target_dir)?;

    let count = selected.len();
    let mut outputs = Vec::new();
    // 跨页去重：同一份原始数据只导出一次（记首次出现的页码）
    let mut seen: HashSet<[u8; 32]> = HashSet::new();

    for (index, page_number) in selected.iter().enumerate() {
        ctx.check_cancel()?;
        progress(index, count, &format!("正在提取第 {page_number} 页"));

        let page = document.pages().get((*page_number - 1) as PdfPageIndex)?;
        // 页内导出序号，只统计真正写出的图片
        let mut image_index = 0usize;

        let mut visit = |image: &PdfPageImageObject<'_>| -> Result<(), AppError> {
            let (Ok(width), Ok(height)) = (image.width(), image.height()) else {
                return Ok(()); // 元数据缺失的对象无法导出，跳过
            };
            if width < MIN_EDGE_PX || height < MIN_EDGE_PX {
                return Ok(());
            }

            let raw = image.get_raw_image_data()?;
            if raw.is_empty() {
                return Ok(());
            }
            if opts.min_size_kb > 0
                && (raw.len() as u64) < u64::from(opts.min_size_kb) * 1024
            {
                return Ok(());
            }

            let digest: [u8; 32] = Sha256::digest(&raw).into();
            if !seen.insert(digest) {
                return Ok(());
            }

            image_index += 1;
            // Auto 且为纯 DCTDecode 时原始字节就是完整 JPEG，直写即无损；
            // 注意：若图片带软掩码（SMask），提取的是不含透明度的底图，
            // 这与「提取原始嵌入图」的语义一致（pdfium-render 未暴露 SMask 检测）。
            let passthrough =
                opts.format == ExtractFormatOpt::Auto && is_plain_jpeg(image, &raw);
            let extension = if passthrough {
                "jpg"
            } else {
                match opts.format {
                    ExtractFormatOpt::Jpeg => "jpg",
                    _ => "png",
                }
            };

            let file_name = format!("{stem}_p{page_number:03}_i{image_index:02}.{extension}");
            let out_path = crate::pdf::resolve_batch_path(target_dir, &file_name);

            if passthrough {
                std::fs::write(&out_path, &raw)?;
            } else {
                let decoded = image.get_raw_image()?;
                let format = match opts.format {
                    ExtractFormatOpt::Jpeg => ImageFormatOpt::Jpeg,
                    _ => ImageFormatOpt::Png,
                };
                imaging::encode_image(&decoded, &out_path, format, opts.quality)?;
            }

            if !opts.zip {
                outputs.push(OutputFile::image(&out_path)?);
            }
            Ok(())
        };

        let objects = page.objects();
        for object_index in 0..objects.len() {
            ctx.check_cancel()?;
            walk_object(objects.get(object_index)?, 0, &mut visit)?;
        }
    }

    if seen.is_empty() {
        return Err(AppError::Pdf("文档中没有可提取的图片".to_string()));
    }

    ctx.check_cancel()?;

    if let Some(dir) = &work_dir {
        progress(count, count, "正在打包 zip");
        let fallback = format!("{stem}_extracted.zip");
        let zip_path =
            crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
        let archive = crate::pdf::zip_directory(dir, &zip_path)?;
        crate::pdf::remove_dir_quiet(dir);
        return Ok(vec![archive]);
    }

    progress(count, count, "提取完成");
    Ok(outputs)
}

/// 原始数据是否为可直接落盘的完整 JPEG：唯一滤镜是 DCTDecode，且以 SOI 标记开头。
/// 链式滤镜（如 FlateDecode 包裹 DCTDecode）的原始数据不是 JPEG 文件，必须先解码。
fn is_plain_jpeg(image: &PdfPageImageObject<'_>, raw: &[u8]) -> bool {
    let filters = image.filters();
    let mut iter = filters.iter();
    iter.next()
        .is_some_and(|filter| filter.name() == "DCTDecode")
        && iter.next().is_none()
        && raw.starts_with(&[0xFF, 0xD8])
}

/// 深度优先遍历页面对象树，对每个图片对象调用 `visit`。
/// Form XObject 内部可能还嵌着图片（模板 / 印章类文档常见），递归展开，
/// 深度受 [`MAX_FORM_DEPTH`] 约束。
fn walk_object<'a, F>(
    object: PdfPageObject<'a>,
    depth: u32,
    visit: &mut F,
) -> Result<(), AppError>
where
    F: FnMut(&PdfPageImageObject<'a>) -> Result<(), AppError>,
{
    match object {
        PdfPageObject::Image(image) => visit(&image),
        PdfPageObject::XObjectForm(form) if depth < MAX_FORM_DEPTH => {
            for index in 0..form.len() {
                walk_object(form.get(index)?, depth + 1, visit)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_are_usable() {
        let opts = ExtractImagesOptions::default();
        assert_eq!(opts.format, ExtractFormatOpt::Auto);
        assert_eq!(opts.min_size_kb, 0);
        assert!(opts.quality >= 1 && opts.quality <= 100);
        assert!(!opts.zip);
    }
}