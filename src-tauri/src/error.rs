//! 统一的应用错误类型。
//!
//! 所有 Tauri command 都返回 `Result<T, AppError>`。`AppError` 采用 serde 邻接标签
//! （`{ "kind": "...", "message": "..." }`）序列化，前端 `lib/tauri.ts` 依据 `kind`
//! 映射到 i18n 文案，避免出现原始英文技术信息。

use pdfium_render::prelude::{PdfiumError, PdfiumInternalError};
use serde::Serialize;

#[derive(Debug, Clone, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum AppError {
    /// 未能加载 libpdfium 动态库，payload 为已尝试过的搜索路径
    #[error("未找到 PDFium 运行库，已尝试：{0:?}")]
    PdfiumNotFound(Vec<String>),

    /// 文档被密码保护，需要用户提供密码
    #[error("文档已加密，请输入打开密码：{0}")]
    PasswordRequired(String),

    /// PDF 引擎返回的其它错误
    #[error("PDF 处理失败：{0}")]
    Pdf(String),

    #[error("文件读写失败：{0}")]
    Io(String),

    #[error("图片处理失败：{0}")]
    Image(String),

    #[error("压缩打包失败：{0}")]
    Archive(String),

    #[error("参数不合法：{0}")]
    InvalidParams(String),

    #[error("页码范围不合法：{0}")]
    Range(String),

    #[error("不支持的操作：{0}")]
    Unsupported(String),

    /// 用户主动取消
    #[error("任务已取消")]
    Cancelled,

    /// 未激活且今日免费次数已用完
    #[error("今日免费次数已用完，激活后可不限次使用")]
    QuotaExceeded,

    /// 激活码校验失败（签名/过期/机器码不匹配等）
    #[error("激活码无效：{0}")]
    LicenseInvalid(String),

    #[error("内部错误：{0}")]
    Internal(String),
}

impl AppError {
    pub fn invalid<T: std::fmt::Display>(msg: T) -> Self {
        AppError::InvalidParams(msg.to_string())
    }

    /// 前端可直接展示的简短描述，用于任务中心列表
    pub fn brief(&self) -> String {
        match self {
            AppError::Cancelled => "已取消".to_string(),
            other => other.to_string(),
        }
    }
}

impl From<PdfiumError> for AppError {
    fn from(err: PdfiumError) -> Self {
        match &err {
            PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
                AppError::PasswordRequired("密码错误或文档受保护".to_string())
            }
            PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::FileError) => {
                AppError::Io("无法打开文件，请确认文件存在且未被占用".to_string())
            }
            PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::FormatError) => {
                AppError::Pdf("文件格式无法识别，可能不是有效的 PDF".to_string())
            }
            PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PageError) => {
                AppError::Pdf("页面索引超出范围".to_string())
            }
            PdfiumError::IoError(io) => AppError::Io(io.to_string()),
            PdfiumError::ImageError => AppError::Image("图片编解码失败".to_string()),
            PdfiumError::NoPagesInDocument => AppError::Pdf("文档中没有任何页面".to_string()),
            other => AppError::Pdf(other.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}

impl From<image::ImageError> for AppError {
    fn from(err: image::ImageError) -> Self {
        AppError::Image(err.to_string())
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(err: zip::result::ZipError) -> Self {
        AppError::Archive(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}
