//! PDF 加水印：为指定页面叠加文字水印（支持居中单行 / 全页斜向平铺）。
//!
//! 直接读取操作系统自带中文字体（macOS PingFang/STHeiti，Windows 微软雅黑/黑体），
//! 通过纯 Rust `ab_glyph` 光栅化为带透明通道的 RGBA 图层叠加到每一页，零额外字体体积、
//! 跨平台一致、不依赖外部工具，同时被 `idcard`（身份证拼版防盗水印）复用。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{DynamicImage, Rgba, RgbaImage};
use pdfium_render::prelude::{PdfPageImageObject, PdfPageIndex, PdfPageObjectsCommon, PdfPoints};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::pdf::types::{OutputFile, PageScope, ProgressSink, TaskCtx};
use crate::pdf::{engine, ranges};

/// 水印排布方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkLayout {
    /// 全页斜向交错平铺（防盗用最有效）
    #[default]
    Tile,
    /// 页面正中央单行
    Center,
}

/// 预设水印颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkColor {
    #[default]
    Gray,
    Red,
    Blue,
    Black,
}

impl WatermarkColor {
    pub fn rgb(self) -> [u8; 3] {
        match self {
            WatermarkColor::Gray => [120, 120, 120],
            WatermarkColor::Red => [210, 48, 48],
            WatermarkColor::Blue => [42, 108, 210],
            WatermarkColor::Black => [28, 28, 28],
        }
    }
}

/// 可复用的文字水印样式，供 `watermark` 与 `idcard` 共用。
#[derive(Debug, Clone)]
pub struct WatermarkStyle {
    pub text: String,
    pub layout: WatermarkLayout,
    pub color: WatermarkColor,
    /// 逻辑字号（以 72 DPI 点为基准，渲染时会按目标 DPI 等比缩放）
    pub font_size: f32,
    /// 不透明度（0.05 ~ 1.0）
    pub opacity: f32,
    /// 旋转角度（度，负值表示逆时针上扬，如 -35）
    pub angle: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkOptions {
    pub source: PathBuf,
    pub password: Option<String>,
    pub text: String,
    #[serde(default)]
    pub layout: WatermarkLayout,
    #[serde(default)]
    pub color: WatermarkColor,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default = "default_angle")]
    pub angle: f32,
    #[serde(default)]
    pub scope: PageScope,
    pub output_name: Option<String>,
}

fn default_font_size() -> f32 {
    36.0
}

fn default_opacity() -> f32 {
    0.22
}

fn default_angle() -> f32 {
    -35.0
}

impl Default for WatermarkOptions {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            password: None,
            text: String::new(),
            layout: WatermarkLayout::Tile,
            color: WatermarkColor::Gray,
            font_size: default_font_size(),
            opacity: default_opacity(),
            angle: default_angle(),
            scope: PageScope::All,
            output_name: None,
        }
    }
}

pub fn run(
    ctx: &TaskCtx,
    out_dir: &Path,
    opts: &WatermarkOptions,
    progress: ProgressSink<'_>,
) -> Result<Vec<OutputFile>, AppError> {
    if opts.source.as_os_str().is_empty() {
        return Err(AppError::invalid("请先选择一个 PDF 文件"));
    }
    let text = opts.text.trim();
    if text.is_empty() {
        return Err(AppError::invalid("请输入水印文字"));
    }

    let pdfium = engine::get()?;
    let password = opts.password.clone().or_else(|| ctx.password.clone());
    let mut document = pdfium.load_pdf_from_file(&opts.source, password.as_deref())?;

    let total = document.pages().len() as usize;
    if total == 0 {
        return Err(AppError::Pdf("文档中没有任何页面".to_string()));
    }

    let selected = ranges::parse_ranges(&opts.scope.as_spec(), total)?;
    let targets: HashSet<usize> = selected.into_iter().collect();

    let style = WatermarkStyle {
        text: text.to_string(),
        layout: opts.layout,
        color: opts.color,
        font_size: opts.font_size.clamp(10.0, 140.0),
        opacity: opts.opacity.clamp(0.05, 1.0),
        angle: opts.angle.clamp(-180.0, 180.0),
    };

    // 同一文档大多数页面尺寸相同，缓存上一张水印图层避免重复光栅化
    let mut cached_layer: Option<(u32, u32, DynamicImage)> = None;
    let mut stamped = 0usize;

    for index in 0..total {
        ctx.check_cancel()?;
        if !targets.contains(&(index + 1)) {
            continue;
        }

        let (page_w, page_h) = {
            let page = document.pages().get(index as PdfPageIndex)?;
            (page.width().value, page.height().value)
        };

        let (px_w, px_h, dpi_scale) = watermark_canvas_size(page_w, page_h);
        let needs_render = cached_layer
            .as_ref()
            .is_none_or(|(cw, ch, _)| *cw != px_w || *ch != px_h);
        if needs_render {
            let scaled_style = WatermarkStyle {
                font_size: style.font_size * dpi_scale,
                ..style.clone()
            };
            let img = render_watermark_layer(px_w, px_h, &scaled_style)?;
            cached_layer = Some((px_w, px_h, img));
        }

        let layer = &cached_layer.as_ref().expect("水印图层已生成").2;
        let object = PdfPageImageObject::new_with_size(
            &document,
            layer,
            PdfPoints::new(page_w),
            PdfPoints::new(page_h),
        )?;
        let mut page = document.pages_mut().get(index as PdfPageIndex)?;
        page.objects_mut().add_image_object(object)?;

        stamped += 1;
        progress(stamped, targets.len(), &format!("已添加第 {} 页水印", index + 1));
    }

    ctx.check_cancel()?;
    progress(targets.len(), targets.len(), "正在写出 PDF");

    let stem = crate::pdf::file_stem(&opts.source);
    let fallback = format!("{stem}_watermark.pdf");
    let out_path = crate::pdf::resolve_output_path(out_dir, opts.output_name.as_deref(), &fallback);
    crate::pdf::save_document(&document, &out_path, Some(&opts.source))?;

    let summary = format!("已为 {stamped} 页添加水印");
    progress(targets.len(), targets.len(), &summary);
    ctx.set_summary(summary);

    Ok(vec![OutputFile::pdf(out_path, Some(total as u32))?])
}

/// 根据 PDF 页面尺寸（pt）计算水印画布像素尺寸与缩放比例（约 150 DPI，长边上限 2000px）。
pub fn watermark_canvas_size(page_w_pt: f32, page_h_pt: f32) -> (u32, u32, f32) {
    let base_scale = 150.0 / 72.0;
    let raw_w = (page_w_pt.max(72.0) * base_scale).round();
    let raw_h = (page_h_pt.max(72.0) * base_scale).round();
    let max_edge = 2000.0_f32;
    let clamp_factor = if raw_w.max(raw_h) > max_edge {
        max_edge / raw_w.max(raw_h)
    } else {
        1.0
    };
    let w = (raw_w * clamp_factor).round().max(64.0) as u32;
    let h = (raw_h * clamp_factor).round().max(64.0) as u32;
    (w, h, base_scale * clamp_factor)
}

/// 将文字水印渲染为 `width_px × height_px` 的透明 RGBA 图层。
pub fn render_watermark_layer(
    width_px: u32,
    height_px: u32,
    style: &WatermarkStyle,
) -> Result<DynamicImage, AppError> {
    let font_bytes = load_system_font_bytes().ok_or_else(|| {
        AppError::Internal("未找到可用的系统中文字体，无法渲染文字水印".to_string())
    })?;
    let font = FontRef::try_from_slice_and_index(font_bytes, 0)
        .map_err(|err| AppError::Internal(format!("解析系统字体失败: {err}")))?;

    let stamp = rasterize_text_stamp(&font, &style.text, style.font_size, style.color.rgb(), style.opacity);
    let rotated = rotate_rgba(&stamp, style.angle);
    let (sw, sh) = rotated.dimensions();

    let mut canvas = RgbaImage::from_pixel(width_px, height_px, Rgba([0, 0, 0, 0]));
    if sw == 0 || sh == 0 {
        return Ok(DynamicImage::ImageRgba8(canvas));
    }

    match style.layout {
        WatermarkLayout::Center => {
            let ox = (width_px as i32 - sw as i32) / 2;
            let oy = (height_px as i32 - sh as i32) / 2;
            blit_rgba(&mut canvas, &rotated, ox, oy);
        }
        WatermarkLayout::Tile => {
            let gap_x = (sw as i32 / 3).max(48);
            let gap_y = (sh as i32 / 2).max(64);
            let step_x = (sw as i32 + gap_x).max(1);
            let step_y = (sh as i32 + gap_y).max(1);

            let mut row = 0i32;
            let mut y = -(sh as i32) / 2;
            while y < height_px as i32 + sh as i32 {
                // 奇数行水平错开半格，视觉更均匀
                let stagger = if row % 2 == 1 { step_x / 2 } else { 0 };
                let mut x = -(sw as i32) / 2 - stagger;
                while x < width_px as i32 + sw as i32 {
                    blit_rgba(&mut canvas, &rotated, x, y);
                    x += step_x;
                }
                y += step_y;
                row += 1;
            }
        }
    }

    Ok(DynamicImage::ImageRgba8(canvas))
}

/// 加载并缓存系统 CJK 字体文件字节。
fn load_system_font_bytes() -> Option<&'static [u8]> {
    static FONT_DATA: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    FONT_DATA
        .get_or_init(|| {
            let candidates: &[&str] = if cfg!(target_os = "macos") {
                &[
                    "/System/Library/Fonts/PingFang.ttc",
                    "/System/Library/Fonts/STHeiti Medium.ttc",
                    "/System/Library/Fonts/STHeiti Light.ttc",
                    "/System/Library/Fonts/Hiragino Sans GB.ttc",
                    "/System/Library/Fonts/Supplemental/Songti.ttc",
                    "/System/Library/Fonts/Helvetica.ttc",
                ]
            } else if cfg!(windows) {
                &[
                    r"C:\Windows\Fonts\msyh.ttc",
                    r"C:\Windows\Fonts\msyh.ttf",
                    r"C:\Windows\Fonts\simhei.ttf",
                    r"C:\Windows\Fonts\simsun.ttc",
                    r"C:\Windows\Fonts\arial.ttf",
                ]
            } else {
                &[
                    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                ]
            };

            for path in candidates {
                if let Ok(bytes) = std::fs::read(path)
                    && FontRef::try_from_slice_and_index(&bytes, 0).is_ok()
                {
                    return Some(bytes);
                }
            }
            None
        })
        .as_deref()
}

/// 将单行文字光栅化为紧凑的透明 RGBA 图章。
fn rasterize_text_stamp(
    font: &FontRef<'_>,
    text: &str,
    font_size: f32,
    rgb: [u8; 3],
    opacity: f32,
) -> RgbaImage {
    let scale = PxScale::from(font_size.max(8.0));
    let scaled = font.as_scaled(scale);
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let line_height = (ascent - descent).ceil().max(1.0) as u32;

    let mut glyphs = Vec::new();
    let mut caret_x = 0.0_f32;
    let mut prev = None;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        if let Some(prev_id) = prev {
            caret_x += scaled.kern(prev_id, id);
        }
        let glyph = id.with_scale_and_position(scale, ab_glyph::point(caret_x, ascent));
        caret_x += scaled.h_advance(id);
        prev = Some(id);
        glyphs.push(glyph);
    }

    let pad = 4u32;
    let width = (caret_x.ceil().max(1.0) as u32) + pad * 2;
    let height = line_height + pad * 2;
    let mut img = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));

    for glyph in glyphs {
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, cov| {
                let x = bounds.min.x as i32 + gx as i32 + pad as i32;
                let y = bounds.min.y as i32 + gy as i32 + pad as i32;
                if x >= 0 && y >= 0 && (x as u32) < width && (y as u32) < height {
                    let alpha = (cov.clamp(0.0, 1.0) * opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
                    if alpha > 0 {
                        let dst = img.get_pixel_mut(x as u32, y as u32);
                        let merged = dst[3].max(alpha);
                        *dst = Rgba([rgb[0], rgb[1], rgb[2], merged]);
                    }
                }
            });
        }
    }

    img
}

/// 围绕中心逆时针/顺时针旋转 RGBA 图像（双线性插值，边缘平滑）。
fn rotate_rgba(src: &RgbaImage, angle_deg: f32) -> RgbaImage {
    if angle_deg.abs() < 0.1 {
        return src.clone();
    }
    let (sw, sh) = src.dimensions();
    let rad = angle_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();

    let nw = ((sw as f32 * cos.abs()) + (sh as f32 * sin.abs())).ceil().max(1.0) as u32;
    let nh = ((sw as f32 * sin.abs()) + (sh as f32 * cos.abs())).ceil().max(1.0) as u32;
    let mut dst = RgbaImage::from_pixel(nw, nh, Rgba([0, 0, 0, 0]));

    let cx = sw as f32 / 2.0;
    let cy = sh as f32 / 2.0;
    let ncx = nw as f32 / 2.0;
    let ncy = nh as f32 / 2.0;

    for dy in 0..nh {
        for dx in 0..nw {
            let rx = dx as f32 - ncx;
            let ry = dy as f32 - ncy;
            let sx = rx * cos + ry * sin + cx;
            let sy = -rx * sin + ry * cos + cy;
            if let Some(pixel) = sample_bilinear(src, sx, sy)
                && pixel[3] > 0
            {
                dst.put_pixel(dx, dy, pixel);
            }
        }
    }

    dst
}

fn sample_bilinear(img: &RgbaImage, x: f32, y: f32) -> Option<Rgba<u8>> {
    let (w, h) = img.dimensions();
    if x < 0.0 || y < 0.0 || x >= (w as f32 - 1.0) || y >= (h as f32 - 1.0) {
        return None;
    }
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let p00 = img.get_pixel(x0, y0).0;
    let p10 = img.get_pixel(x0 + 1, y0).0;
    let p01 = img.get_pixel(x0, y0 + 1).0;
    let p11 = img.get_pixel(x0 + 1, y0 + 1).0;

    let mut out = [0u8; 4];
    for c in 0..4 {
        let top = p00[c] as f32 * (1.0 - fx) + p10[c] as f32 * fx;
        let bot = p01[c] as f32 * (1.0 - fx) + p11[c] as f32 * fx;
        out[c] = (top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8;
    }
    Some(Rgba(out))
}

/// 将 `src` 按 alpha 混合贴到 `dst` 的 `(ox, oy)` 处。
fn blit_rgba(dst: &mut RgbaImage, src: &RgbaImage, ox: i32, oy: i32) {
    let (dw, dh) = dst.dimensions();
    let (sw, sh) = src.dimensions();
    for sy in 0..sh {
        let dy = oy + sy as i32;
        if dy < 0 || dy >= dh as i32 {
            continue;
        }
        for sx in 0..sw {
            let dx = ox + sx as i32;
            if dx < 0 || dx >= dw as i32 {
                continue;
            }
            let sp = src.get_pixel(sx, sy).0;
            if sp[3] == 0 {
                continue;
            }
            let dp = dst.get_pixel_mut(dx as u32, dy as u32);
            let sa = sp[3] as f32 / 255.0;
            let da = dp[3] as f32 / 255.0;
            let out_a = sa + da * (1.0 - sa);
            if out_a > 0.0 {
                for c in 0..3 {
                    let v = (sp[c] as f32 * sa + dp[c] as f32 * da * (1.0 - sa)) / out_a;
                    dp[c] = v.round().clamp(0.0, 255.0) as u8;
                }
                dp[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watermark_canvas_size_stays_bounded() {
        let (w, h, scale) = watermark_canvas_size(595.0, 842.0);
        assert!(w <= 2000 && h <= 2000);
        assert!(scale > 0.0);

        let (lw, lh, _) = watermark_canvas_size(5000.0, 5000.0);
        assert!(lw <= 2000 && lh <= 2000);
    }

    #[test]
    fn renders_non_empty_watermark_layer() {
        let style = WatermarkStyle {
            text: "仅供测试使用".to_string(),
            layout: WatermarkLayout::Tile,
            color: WatermarkColor::Gray,
            font_size: 40.0,
            opacity: 0.3,
            angle: -35.0,
        };
        let img = render_watermark_layer(600, 800, &style).expect("系统字体应能渲染水印");
        let rgba = img.to_rgba8();
        assert!(rgba.pixels().any(|p| p[3] > 0), "水印图层应包含可见像素");
    }
}
