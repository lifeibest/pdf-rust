//! 图片读写、格式识别与排版计算。
//!
//! `export_images`（PDF 转图片）与 `images_to_pdf` / `merge`（图片建页）共用这里的逻辑，
//! 保证「导出后再导回」的尺寸表现一致。

use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::webp::WebPEncoder;
use image::{DynamicImage, ExtendedColorType, ImageEncoder};

use crate::error::AppError;
use crate::pdf::types::ImageFitMode;

/// 支持作为输入的图片扩展名（小写，不含点）。
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff"];

/// 每英寸点数，PDF 规范固定为 72。
pub const POINTS_PER_INCH: f32 = 72.0;

/// 毫米转点的系数。
pub const POINTS_PER_MM: f32 = POINTS_PER_INCH / 25.4;

pub fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

pub fn is_pdf(path: &Path) -> bool {
    extension_of(path) == "pdf"
}

pub fn is_image(path: &Path) -> bool {
    IMAGE_EXTENSIONS.contains(&extension_of(path).as_str())
}

/// 读取图片。GIF 取第一帧，其余格式由 `image` crate 自动识别。
pub fn load_image(path: &Path) -> Result<DynamicImage, AppError> {
    if !path.exists() {
        return Err(AppError::Io(format!("图片不存在：{}", path.display())));
    }

    image::open(path)
        .map_err(|err| AppError::Image(format!("无法解码图片 {}: {err}", path.display())))
}

/// 按指定格式与质量写出图片。
///
/// 注意：WebP 使用 `image` crate 内置的无损编码器，`quality` 参数对其无效，
/// 前端在选择 WebP 时应禁用质量滑杆。
pub fn encode_image(
    image: &DynamicImage,
    path: &Path,
    format: crate::pdf::types::ImageFormatOpt,
    quality: u8,
) -> Result<(), AppError> {
    use crate::pdf::types::ImageFormatOpt;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match format {
        ImageFormatOpt::Png => {
            image.save_with_format(path, image::ImageFormat::Png)?;
        }
        ImageFormatOpt::Jpeg => {
            // JPEG 不支持 alpha 通道，必须先转 RGB
            let rgb = image.to_rgb8();
            let file = std::io::BufWriter::new(std::fs::File::create(path)?);
            JpegEncoder::new_with_quality(file, quality.clamp(1, 100)).write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ExtendedColorType::Rgb8,
            )?;
        }
        ImageFormatOpt::Webp => {
            let rgba = image.to_rgba8();
            let file = std::io::BufWriter::new(std::fs::File::create(path)?);
            WebPEncoder::new_lossless(file).write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                ExtendedColorType::Rgba8,
            )?;
        }
    }

    Ok(())
}

/// 图片在页面上的落位结果，单位为点，坐标原点为页面左下角（PDF 用户空间）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 计算图片在页面上的落位。纯函数，便于单元测试。
///
/// - `page_width_pt` / `page_height_pt`：页面尺寸（点）
/// - `margin_mm`：四边留白（毫米）
/// - `image_width_px` / `image_height_px`：图片像素尺寸
/// - `mode`：排版方式
/// - `dpi`：`NativeDpi` 模式下像素到点的换算基准
pub fn place_image(
    page_width_pt: f32,
    page_height_pt: f32,
    margin_mm: f32,
    image_width_px: u32,
    image_height_px: u32,
    mode: ImageFitMode,
    dpi: u32,
) -> Placement {
    let margin_pt = (margin_mm.max(0.0)) * POINTS_PER_MM;
    // 边距过大时保留至少 1pt 的可用区域，避免除零与负尺寸
    let avail_w = (page_width_pt - 2.0 * margin_pt).max(1.0);
    let avail_h = (page_height_pt - 2.0 * margin_pt).max(1.0);

    let img_w = image_width_px.max(1) as f32;
    let img_h = image_height_px.max(1) as f32;

    match mode {
        ImageFitMode::Fit => {
            let scale = (avail_w / img_w).min(avail_h / img_h);
            let width = img_w * scale;
            let height = img_h * scale;
            Placement {
                x: margin_pt + (avail_w - width) / 2.0,
                y: margin_pt + (avail_h - height) / 2.0,
                width,
                height,
            }
        }
        ImageFitMode::Fill => {
            let scale = (avail_w / img_w).max(avail_h / img_h);
            let width = img_w * scale;
            let height = img_h * scale;
            // 超出部分由页面边界自然裁切，整体在页面上居中
            Placement {
                x: (page_width_pt - width) / 2.0,
                y: (page_height_pt - height) / 2.0,
                width,
                height,
            }
        }
        ImageFitMode::Stretch => Placement {
            x: margin_pt,
            y: margin_pt,
            width: avail_w,
            height: avail_h,
        },
        ImageFitMode::NativeDpi => {
            let dpi = if dpi == 0 { 96 } else { dpi } as f32;
            let width = img_w * POINTS_PER_INCH / dpi;
            let height = img_h * POINTS_PER_INCH / dpi;
            Placement {
                x: (page_width_pt - width) / 2.0,
                y: (page_height_pt - height) / 2.0,
                width,
                height,
            }
        }
    }
}

/// 依据图片宽高比判断是否为横向图。
pub fn is_landscape(image_width_px: u32, image_height_px: u32) -> bool {
    image_width_px > image_height_px
}

/// 依据 DPI 计算渲染像素宽度：`页面宽度(点) * dpi / 72`，并夹在 `[16, 20000]` 之间
/// 以避免异常输入导致的巨幅位图。
pub fn pixels_for_dpi(width_pt: f32, dpi: u32) -> i32 {
    let dpi = dpi.clamp(24, 1200) as f32;
    let pixels = (width_pt * dpi / POINTS_PER_INCH).round() as i32;
    pixels.clamp(16, 20_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A4: 595 x 842 pt
    const A4_W: f32 = 595.0;
    const A4_H: f32 = 842.0;

    #[test]
    fn detects_file_kinds_by_extension() {
        assert!(is_pdf(Path::new("/tmp/a.PDF")));
        assert!(!is_pdf(Path::new("/tmp/a.png")));
        assert!(is_image(Path::new("/tmp/a.JPG")));
        assert!(is_image(Path::new("/tmp/a.tiff")));
        assert!(!is_image(Path::new("/tmp/a.pdf")));
    }

    #[test]
    fn fit_keeps_aspect_ratio_and_stays_inside_margins() {
        let placement = place_image(A4_W, A4_H, 10.0, 2000, 1000, ImageFitMode::Fit, 96);
        let ratio = placement.width / placement.height;
        assert!((ratio - 2.0).abs() < 1e-4, "等比缩放应保持宽高比: {ratio}");

        let margin_pt = 10.0 * POINTS_PER_MM;
        assert!(placement.x >= margin_pt - 1e-3);
        assert!(placement.y >= margin_pt - 1e-3);
        assert!(placement.x + placement.width <= A4_W - margin_pt + 1e-3);
        assert!(placement.y + placement.height <= A4_H - margin_pt + 1e-3);
    }

    #[test]
    fn fit_centers_image_on_page() {
        let placement = place_image(A4_W, A4_H, 0.0, 100, 100, ImageFitMode::Fit, 96);
        // 正方形图片在竖向 A4 上受宽度限制，垂直方向应居中
        assert!((placement.width - A4_W).abs() < 1e-3);
        assert!((placement.height - A4_W).abs() < 1e-3);
        assert!((placement.y - (A4_H - placement.height) / 2.0).abs() < 1e-3);
        assert!(placement.x.abs() < 1e-3);
    }

    #[test]
    fn fill_covers_available_area() {
        let placement = place_image(A4_W, A4_H, 10.0, 100, 100, ImageFitMode::Fill, 96);
        let margin_pt = 10.0 * POINTS_PER_MM;
        assert!(placement.width >= A4_W - 2.0 * margin_pt - 1e-3);
        assert!(placement.height >= A4_H - 2.0 * margin_pt - 1e-3);
    }

    #[test]
    fn stretch_fills_available_area_exactly() {
        let placement = place_image(A4_W, A4_H, 5.0, 300, 100, ImageFitMode::Stretch, 96);
        let margin_pt = 5.0 * POINTS_PER_MM;
        assert!((placement.width - (A4_W - 2.0 * margin_pt)).abs() < 1e-3);
        assert!((placement.height - (A4_H - 2.0 * margin_pt)).abs() < 1e-3);
    }

    #[test]
    fn native_dpi_converts_pixels_to_points() {
        // 96 dpi 下 96px = 1 inch = 72pt
        let placement = place_image(A4_W, A4_H, 0.0, 96, 192, ImageFitMode::NativeDpi, 96);
        assert!((placement.width - 72.0).abs() < 1e-3);
        assert!((placement.height - 144.0).abs() < 1e-3);

        // 72 dpi 下 1px = 1pt
        let placement = place_image(A4_W, A4_H, 0.0, 100, 100, ImageFitMode::NativeDpi, 72);
        assert!((placement.width - 100.0).abs() < 1e-3);
    }

    #[test]
    fn degenerate_inputs_do_not_produce_nan_or_negative_sizes() {
        for (pw, ph, iw, ih, margin) in [
            (0.0f32, 0.0f32, 0u32, 0u32, 0.0f32),
            (10.0, 10.0, 1, 1, 1000.0),
            (A4_W, A4_H, 0, 5, 20.0),
        ] {
            for mode in [
                ImageFitMode::Fit,
                ImageFitMode::Fill,
                ImageFitMode::Stretch,
                ImageFitMode::NativeDpi,
            ] {
                let placement = place_image(pw, ph, margin, iw, ih, mode, 0);
                assert!(
                    placement.width.is_finite() && placement.width > 0.0,
                    "{mode:?}"
                );
                assert!(
                    placement.height.is_finite() && placement.height > 0.0,
                    "{mode:?}"
                );
                assert!(placement.x.is_finite() && placement.y.is_finite());
            }
        }
    }

    #[test]
    fn computes_pixels_for_dpi() {
        assert_eq!(pixels_for_dpi(595.0, 72), 595);
        assert_eq!(pixels_for_dpi(595.0, 150), 1240);
        // dpi 超出合理区间时先夹紧到 [24, 1200]，再换算像素
        assert_eq!(pixels_for_dpi(595.0, 0), 198);
        assert_eq!(pixels_for_dpi(595.0, 100_000), 9917);
    }

    #[test]
    fn detects_orientation() {
        assert!(is_landscape(200, 100));
        assert!(!is_landscape(100, 200));
        assert!(!is_landscape(100, 100));
    }
}
