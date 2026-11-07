//! 文档信息探测：页数、页面尺寸、旋转角、元数据、是否加密。

use std::path::{Path, PathBuf};

use pdfium_render::prelude::{PdfDocumentMetadataTagType, PdfPageRenderRotation};
use serde::Serialize;

use crate::error::AppError;
use crate::pdf::engine;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    /// 0-based 页索引
    pub index: u32,
    /// 页面宽度（点，1pt = 1/72 inch），已计入 /Rotate 造成的宽高互换
    pub width_pt: f32,
    pub height_pt: f32,
    /// 页面自带的旋转角（顺时针，度）
    pub rotation_deg: u16,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<String>,
    pub modification_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocInfo {
    pub path: PathBuf,
    pub file_name: String,
    pub file_size: u64,
    pub page_count: u32,
    /// 文档是否被密码保护
    pub encrypted: bool,
    /// 提供了密码但仍无法打开（密码错误）
    pub password_failed: bool,
    /// PDF 文件版本，如 `V1_7`
    pub pdf_version: Option<String>,
    pub metadata: DocMetadata,
    /// `include_pages` 为 true 时填充；大文档可由调用方关闭以省去逐页加载
    pub pages: Vec<PageInfo>,
}

/// 探测 PDF 文档信息。
///
/// 加密文档不会返回错误，而是把 `encrypted` 置真、`page_count` 置 0，
/// 以便前端弹出密码输入框后重试。
pub fn probe(path: impl AsRef<Path>, password: Option<&str>) -> Result<DocInfo, AppError> {
    probe_with_pages(path, password, true)
}

/// 同 [`probe`]，但可关闭逐页明细以加快大文档的探测速度。
pub fn probe_with_pages(
    path: impl AsRef<Path>,
    password: Option<&str>,
    include_pages: bool,
) -> Result<DocInfo, AppError> {
    let path = path.as_ref();
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    let base = DocInfo {
        path: path.to_path_buf(),
        file_name,
        file_size,
        page_count: 0,
        encrypted: false,
        password_failed: false,
        pdf_version: None,
        metadata: DocMetadata::default(),
        pages: Vec::new(),
    };

    let pdfium = engine::get()?;
    let document = match pdfium.load_pdf_from_file(path, password) {
        Ok(document) => document,
        Err(err) => {
            let mapped = AppError::from(err);
            // 未提供密码或密码错误都归为“需要密码”，由前端决定是首次询问还是提示重试
            if matches!(mapped, AppError::PasswordRequired(_)) {
                return Ok(DocInfo {
                    encrypted: true,
                    password_failed: password.is_some(),
                    ..base
                });
            }
            return Err(mapped);
        }
    };

    let page_count = document.pages().len().max(0) as u32;
    let metadata = read_metadata(&document);
    let pdf_version = Some(format!("{:?}", document.version()));
    let encrypted = password.is_some() || trailer_mentions_encryption(path);
    let pages = if include_pages {
        collect_pages(&document)?
    } else {
        Vec::new()
    };

    Ok(DocInfo {
        page_count,
        encrypted,
        pdf_version,
        metadata,
        pages,
        ..base
    })
}

/// 快速判断文档是否带加密字典：`/Encrypt` 位于文件末尾的 trailer 中，
/// 因此只读取最后一段字节即可，避免为大文档做全量扫描。
fn trailer_mentions_encryption(path: &Path) -> bool {
    use std::io::{Read, Seek, SeekFrom};

    const TAIL_BYTES: u64 = 128 * 1024;

    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(len) = file.metadata().map(|m| m.len()) else {
        return false;
    };

    let start = len.saturating_sub(TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return false;
    }

    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return false;
    }

    buf.windows(8).any(|window| window == b"/Encrypt")
}

fn collect_pages(
    document: &pdfium_render::prelude::PdfDocument,
) -> Result<Vec<PageInfo>, AppError> {
    let mut pages = Vec::with_capacity(document.pages().len().max(0) as usize);

    for (index, page) in document.pages().iter().enumerate() {
        let rotation = page.rotation().unwrap_or(PdfPageRenderRotation::None);
        pages.push(PageInfo {
            index: index as u32,
            width_pt: page.width().value,
            height_pt: page.height().value,
            rotation_deg: rotation_degrees(rotation),
        });
    }

    Ok(pages)
}

/// 把 [`PdfPageRenderRotation`] 转换为顺时针角度。
pub fn rotation_degrees(rotation: PdfPageRenderRotation) -> u16 {
    match rotation {
        PdfPageRenderRotation::None => 0,
        PdfPageRenderRotation::Degrees90 => 90,
        PdfPageRenderRotation::Degrees180 => 180,
        PdfPageRenderRotation::Degrees270 => 270,
    }
}

/// 把顺时针角度转换为 [`PdfPageRenderRotation`]，非法角度返回参数错误。
pub fn rotation_from_degrees(degrees: i32) -> Result<PdfPageRenderRotation, AppError> {
    // 归一化到 [0, 360)，支持负角度（逆时针）输入
    let normalized = degrees.rem_euclid(360);
    match normalized {
        0 => Ok(PdfPageRenderRotation::None),
        90 => Ok(PdfPageRenderRotation::Degrees90),
        180 => Ok(PdfPageRenderRotation::Degrees180),
        270 => Ok(PdfPageRenderRotation::Degrees270),
        other => Err(AppError::invalid(format!(
            "旋转角度必须是 90 的整数倍，收到 {other}"
        ))),
    }
}

fn read_metadata(document: &pdfium_render::prelude::PdfDocument) -> DocMetadata {
    use PdfDocumentMetadataTagType as Tag;

    let metadata = document.metadata();
    let get = |tag: Tag| -> Option<String> {
        metadata
            .get(tag)
            .map(|entry| entry.value().trim().to_string())
            .filter(|value| !value.is_empty())
    };

    DocMetadata {
        title: get(Tag::Title),
        author: get(Tag::Author),
        subject: get(Tag::Subject),
        keywords: get(Tag::Keywords),
        creator: get(Tag::Creator),
        producer: get(Tag::Producer),
        creation_date: get(Tag::CreationDate),
        modification_date: get(Tag::ModificationDate),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_rotation_to_degrees() {
        assert_eq!(rotation_degrees(PdfPageRenderRotation::None), 0);
        assert_eq!(rotation_degrees(PdfPageRenderRotation::Degrees90), 90);
        assert_eq!(rotation_degrees(PdfPageRenderRotation::Degrees180), 180);
        assert_eq!(rotation_degrees(PdfPageRenderRotation::Degrees270), 270);
    }

    #[test]
    fn converts_degrees_to_rotation() {
        assert_eq!(
            rotation_from_degrees(90).unwrap(),
            PdfPageRenderRotation::Degrees90
        );
        assert_eq!(
            rotation_from_degrees(450).unwrap(),
            PdfPageRenderRotation::Degrees90
        );
        assert_eq!(
            rotation_from_degrees(-90).unwrap(),
            PdfPageRenderRotation::Degrees270
        );
        assert_eq!(
            rotation_from_degrees(0).unwrap(),
            PdfPageRenderRotation::None
        );
        assert!(matches!(
            rotation_from_degrees(45),
            Err(AppError::InvalidParams(_))
        ));
    }
}
