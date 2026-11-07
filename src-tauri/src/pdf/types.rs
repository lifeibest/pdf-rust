//! 各功能模块共享的数据类型：纸张、方向、图片排版模式、页范围、输出物与任务上下文。
//!
//! 这些类型的 serde 表示即前后端契约，前端 `src/features/*/meta.ts` 中的取值必须与之一致。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use pdfium_render::prelude::{PdfPagePaperSize, PdfPagePaperStandardSize, PdfPoints};
use serde::{Deserialize, Serialize};

/// 输出纸张尺寸。`Auto` 表示跟随第一个输入源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PageSizeOpt {
    #[default]
    A4,
    A3,
    A5,
    Letter,
    Legal,
    Auto,
}

impl PageSizeOpt {
    /// 把选项解析为具体的 PDFium 纸张尺寸；`Auto` 时使用 `fallback`。
    ///
    /// Letter / Legal 按英寸精确换算（612×792、612×1008 点），而不是走 pdfium-render
    /// 内置的毫米取整值（216×279 mm），否则输出页面会比标准尺寸短约 1 点。
    pub fn resolve(self, fallback: PdfPagePaperSize) -> PdfPagePaperSize {
        match self {
            PageSizeOpt::A4 => PdfPagePaperSize::a4(),
            PageSizeOpt::A3 => PdfPagePaperSize::a3(),
            PageSizeOpt::A5 => PdfPagePaperSize::new_portrait(PdfPagePaperStandardSize::A5),
            PageSizeOpt::Letter => us_paper_size(8.5, 11.0),
            PageSizeOpt::Legal => us_paper_size(8.5, 14.0),
            PageSizeOpt::Auto => fallback,
        }
    }

    /// 返回该选项对应纸张的 (宽, 高) 点数，`Auto` 返回 A4，供前端预览使用。
    pub fn points(self) -> (f32, f32) {
        let size = self.resolve(PdfPagePaperSize::a4());
        (size.width().value, size.height().value)
    }
}

/// 以英寸定义的纵向纸张尺寸。
fn us_paper_size(width_in: f32, height_in: f32) -> PdfPagePaperSize {
    PdfPagePaperSize::new_custom(
        PdfPoints::from_inches(width_in),
        PdfPoints::from_inches(height_in),
    )
}

/// 页面方向。`Auto` 依据图片宽高比自动选择横/纵向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrientationOpt {
    #[default]
    Portrait,
    Landscape,
    Auto,
}

impl OrientationOpt {
    /// 结合图片宽高比决定最终纸张尺寸。
    pub fn apply(self, base: PdfPagePaperSize, image_is_landscape: bool) -> PdfPagePaperSize {
        match self {
            OrientationOpt::Portrait => base.portrait(),
            OrientationOpt::Landscape => base.landscape(),
            OrientationOpt::Auto => {
                if image_is_landscape {
                    base.landscape()
                } else {
                    base.portrait()
                }
            }
        }
    }
}

/// 图片在页面上的排版方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImageFitMode {
    /// 等比缩放至完整放入可用区域，居中留白（默认，不裁切内容）
    #[default]
    Fit,
    /// 等比缩放至铺满可用区域，超出部分被裁切
    Fill,
    /// 非等比拉伸铺满可用区域
    Stretch,
    /// 按 `image_dpi` 把像素换算为点，图片原始物理尺寸呈现（可能超出页面）
    NativeDpi,
}

/// 作用页范围。`Spec` 接受 `ranges::parse_ranges` 支持的表达式，如 `1,3-5`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PageScope {
    #[default]
    All,
    Odd,
    Even,
    Spec(String),
}

impl PageScope {
    /// 转换为 `ranges` 模块可解析的表达式字符串。
    pub fn as_spec(&self) -> String {
        match self {
            PageScope::All => "all".to_string(),
            PageScope::Odd => "odd".to_string(),
            PageScope::Even => "even".to_string(),
            PageScope::Spec(spec) => spec.clone(),
        }
    }
}

/// 栅格化输出格式。WebP 走 `image` crate 的无损编码器，`quality` 对其无效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormatOpt {
    Png,
    #[default]
    Jpeg,
    Webp,
}

impl ImageFormatOpt {
    pub fn extension(self) -> &'static str {
        match self {
            ImageFormatOpt::Png => "png",
            ImageFormatOpt::Jpeg => "jpg",
            ImageFormatOpt::Webp => "webp",
        }
    }

    pub fn image_format(self) -> image::ImageFormat {
        match self {
            ImageFormatOpt::Png => image::ImageFormat::Png,
            ImageFormatOpt::Jpeg => image::ImageFormat::Jpeg,
            ImageFormatOpt::Webp => image::ImageFormat::WebP,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    Pdf,
    Image,
    Archive,
}

/// 任务产出的单个文件。`path` 为绝对路径，前端可直接用于 `reveal` 或 `convertFileSrc`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputFile {
    pub path: PathBuf,
    pub name: String,
    pub kind: OutputKind,
    pub size_bytes: u64,
    pub pages: Option<u32>,
}

impl OutputFile {
    pub fn pdf(path: impl AsRef<Path>, pages: Option<u32>) -> Result<Self, std::io::Error> {
        Self::new(path, OutputKind::Pdf, pages)
    }

    pub fn image(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        Self::new(path, OutputKind::Image, None)
    }

    pub fn archive(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        Self::new(path, OutputKind::Archive, None)
    }

    fn new(
        path: impl AsRef<Path>,
        kind: OutputKind,
        pages: Option<u32>,
    ) -> Result<Self, std::io::Error> {
        let path = path.as_ref().to_path_buf();
        let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());

        Ok(Self {
            path,
            name,
            kind,
            size_bytes,
            pages,
        })
    }
}

/// 协作式取消令牌。长循环中每处理一页调用一次 `is_cancelled()`。
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// 任务上下文，贯穿一次 `run_task` 调用。
pub struct TaskCtx {
    /// 缩略图等中间产物目录（app cache dir 下）
    pub cache_dir: PathBuf,
    /// 加密文档的打开密码
    pub password: Option<String>,
    pub cancel: CancelToken,
}

impl TaskCtx {
    pub fn new(cache_dir: PathBuf, password: Option<String>, cancel: CancelToken) -> Self {
        Self {
            cache_dir,
            password,
            cancel,
        }
    }

    /// 密码字符串引用，供 `Pdfium::load_pdf_from_file` 使用。
    pub fn password_str(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// 命中取消时返回 `AppError::Cancelled`，用于 `?` 短路。
    pub fn check_cancel(&self) -> Result<(), crate::error::AppError> {
        if self.cancel.is_cancelled() {
            Err(crate::error::AppError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// 进度回调：`(已完成数, 总数, 描述)`。
pub type ProgressSink<'a> = &'a mut (dyn FnMut(usize, usize, &str) + 'a);

/// 供不需要进度的场景（如单元测试）使用的空回调。
pub fn noop_progress() -> impl FnMut(usize, usize, &str) {
    |_: usize, _: usize, _: &str| {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A4 的点数（ISO 216：210 × 297 mm）
    const A4: (f32, f32) = (595.276, 841.89);

    #[test]
    fn us_paper_sizes_match_the_pdf_spec_exactly() {
        assert_eq!(PageSizeOpt::Letter.points(), (612.0, 792.0));
        assert_eq!(PageSizeOpt::Legal.points(), (612.0, 1008.0));
    }

    #[test]
    fn iso_paper_sizes_are_resolved_from_standard_table() {
        let (a4_w, a4_h) = PageSizeOpt::A4.points();
        assert!((a4_w - A4.0).abs() < 0.01 && (a4_h - A4.1).abs() < 0.01);

        let (a3_w, a3_h) = PageSizeOpt::A3.points();
        assert!(a3_w > a4_w && a3_h > a4_h, "A3 应当大于 A4");

        let (a5_w, a5_h) = PageSizeOpt::A5.points();
        assert!(a5_w < a4_w && a5_h < a4_h, "A5 应当小于 A4");
    }

    #[test]
    fn auto_falls_back_to_the_given_size() {
        let letter = PageSizeOpt::Letter.resolve(PdfPagePaperSize::a4());
        let resolved = PageSizeOpt::Auto.resolve(letter);
        assert_eq!(
            (resolved.width().value, resolved.height().value),
            (612.0, 792.0),
            "Auto 应当沿用传入的回退尺寸"
        );
    }

    #[test]
    fn orientation_swaps_width_and_height() {
        let letter = PageSizeOpt::Letter.resolve(PdfPagePaperSize::a4());

        let portrait = OrientationOpt::Portrait.apply(letter, true);
        assert_eq!(
            (portrait.width().value, portrait.height().value),
            (612.0, 792.0)
        );

        let landscape = OrientationOpt::Landscape.apply(letter, false);
        assert_eq!(
            (landscape.width().value, landscape.height().value),
            (792.0, 612.0),
            "横向应当交换宽高，与图片实际比例无关"
        );

        let auto_wide = OrientationOpt::Auto.apply(letter, true);
        assert_eq!(
            (auto_wide.width().value, auto_wide.height().value),
            (792.0, 612.0)
        );

        let auto_tall = OrientationOpt::Auto.apply(letter, false);
        assert_eq!(
            (auto_tall.width().value, auto_tall.height().value),
            (612.0, 792.0)
        );
    }

    #[test]
    fn orientation_is_idempotent() {
        let letter = PageSizeOpt::Letter.resolve(PdfPagePaperSize::a4());
        let once = OrientationOpt::Landscape.apply(letter, false);
        let twice = OrientationOpt::Landscape.apply(once, false);
        assert_eq!(
            (twice.width().value, twice.height().value),
            (once.width().value, once.height().value),
            "已经是横向时再转一次不应翻回纵向"
        );
    }
}
