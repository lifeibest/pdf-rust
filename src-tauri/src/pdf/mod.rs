//! PDF 处理能力层。
//!
//! 约定：
//! - 每个功能一个模块（`merge` / `split` / `rotate` / `export_images` / `extract_images` /
//!   `images_to_pdf` / `invoice`），
//!   对外暴露 `run(...)`，统一签名风格 `fn run(ctx, opts, progress) -> Result<Vec<OutputFile>, AppError>`。
//! - 引擎只允许通过 [`engine::get()`] 获取，禁止各模块自行绑定 libpdfium。
//! - 长循环必须调用 `ctx.check_cancel()?` 以支持取消。
//!
//! 新增功能的步骤见 `docs/ADD-FEATURE.md`。

pub mod engine;
pub mod export_images;
pub mod extract_images;
pub mod images_to_pdf;
pub mod imaging;
pub mod info;
pub mod invoice;
pub mod merge;
pub mod ranges;
pub mod rotate;
pub mod split;
pub mod thumbnails;
pub mod types;
pub mod wechat;

pub use types::{
    CancelToken, ImageFitMode, ImageFormatOpt, OrientationOpt, OutputFile, OutputKind, PageScope,
    PageSizeOpt, ProgressSink, TaskCtx, noop_progress,
};

use std::path::{Path, PathBuf};

use pdfium_render::prelude::PdfDocument;

use crate::error::AppError;

/// 保存文档。
///
/// 当目标路径与源文件为同一个文件时（例如原地旋转），先写入同目录的隐藏临时文件再
/// 原子重命名，避免 PDFium 边读边写损坏源文件。
pub fn save_document(
    document: &PdfDocument,
    target: &Path,
    source: Option<&Path>,
) -> Result<(), AppError> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let in_place = source.is_some_and(|src| is_same_file(src, target));
    if in_place {
        let parent = target.parent().unwrap_or_else(|| Path::new("."));
        let temp = unique_temp_path(parent, "pdfzt", "pdf");
        let result = document.save_to_file(&temp);
        match result {
            Ok(()) => std::fs::rename(&temp, target)?,
            Err(err) => {
                let _ = std::fs::remove_file(&temp);
                return Err(err.into());
            }
        }
    } else {
        document.save_to_file(target)?;
    }

    Ok(())
}

/// 判断两个路径是否指向同一个文件（先做规范化比较，失败时退回字面比较）。
pub fn is_same_file(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// 生成一个唯一的隐藏临时文件路径。
pub fn unique_temp_path(dir: &Path, prefix: &str, ext: &str) -> PathBuf {
    dir.join(format!(".{prefix}-{}.{ext}", uuid::Uuid::new_v4().simple()))
}

/// 在 `parent` 下创建一个唯一的隐藏临时目录，用于「先产出多个文件再打包」的场景。
pub fn unique_temp_dir(parent: &Path, prefix: &str) -> Result<PathBuf, AppError> {
    std::fs::create_dir_all(parent)?;
    let dir = parent.join(format!(".{prefix}-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 静默删除目录，用于清理临时产物。
pub fn remove_dir_quiet(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

/// 把 `dir` 下的所有文件（不递归）打包为 zip。
pub fn zip_directory(dir: &Path, zip_path: &Path) -> Result<OutputFile, AppError> {
    use std::io::Write;

    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    // 打包模式下输出目录可能从未被创建（文件都写在临时目录里），这里补齐
    if let Some(parent) = zip_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut entries = std::fs::read_dir(dir)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    // 按文件名排序，保证压缩包内顺序与页面顺序一致
    entries.sort();

    let file = std::io::BufWriter::new(std::fs::File::create(zip_path)?);
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for entry in &entries {
        let name = entry
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| entry.display().to_string());
        writer.start_file(name, options)?;
        let mut input = std::fs::File::open(entry)?;
        std::io::copy(&mut input, &mut writer)?;
        writer.flush()?;
    }

    writer.finish()?;
    Ok(OutputFile::archive(zip_path)?)
}

/// 取文件名主干（不含扩展名），无法识别时回退到完整文件名。
pub fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "output".to_string())
        })
}

/// 品牌后缀：未自定义名称的默认产出，在扩展名前插入 `_pdfai.cn`，
/// 如 `doc_merged.pdf` → `doc_merged_pdfai.cn.pdf`，便于识别文件来源。
const BRAND_SUFFIX: &str = "_pdfai.cn";

/// 在扩展名前插入品牌后缀；没有扩展名时直接追加在末尾。
fn with_brand_suffix(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !ext.is_empty() => format!("{stem}{BRAND_SUFFIX}.{ext}"),
        _ => format!("{name}{BRAND_SUFFIX}"),
    }
}

/// 在输出目录下解析最终写入路径：名称为空时使用 `fallback`（并加品牌后缀），
/// 已存在同名文件时自动追加 `-1`、`-2` 后缀，避免静默覆盖用户文件。
pub fn resolve_output_path(out_dir: &Path, name: Option<&str>, fallback: &str) -> PathBuf {
    let desired = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(sanitize_file_name)
        .unwrap_or_else(|| sanitize_file_name(&with_brand_suffix(fallback)));
    let candidate = out_dir.join(&desired);
    ensure_unique_path(candidate)
}

/// 批量输出（拆分 / 导出图片）中单个文件的路径。
///
/// 与 [`resolve_output_path`] 一样不覆盖已存在的文件；批量模式没有自定义名称，
/// 名称由调用方按页序生成（如 `doc_p001.png`），因此统一加品牌后缀。
pub fn resolve_batch_path(out_dir: &Path, name: &str) -> PathBuf {
    ensure_unique_path(out_dir.join(sanitize_file_name(&with_brand_suffix(name))))
}

/// 去除路径分隔符等危险字符，防止用户输入造成越界写入。
fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "output".to_string()
    } else {
        cleaned
    }
}

/// 若路径已存在，则在主干后追加递增序号。
fn ensure_unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }

    let parent = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".to_string());
    let extension = path
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    for index in 1..1000 {
        let name = if extension.is_empty() {
            format!("{stem}-{index}")
        } else {
            format!("{stem}-{index}.{extension}")
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }

    // 极端情况下退回唯一临时名，保证不覆盖已有文件
    unique_temp_path(&parent, &stem, &extension)
}
