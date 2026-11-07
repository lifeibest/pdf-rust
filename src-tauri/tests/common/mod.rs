//! 集成测试公共夹具。
//!
//! 仓库里不提交任何二进制 fixture：先用 `image` crate 生成纯色 PNG，
//! 再经 `images_to_pdf` 造出可控页数的 PDF，所有测试都基于这些临时文件。

// 每个集成测试都是独立的 crate，只会用到这里的一部分助手，
// 因此允许未使用项，避免每个测试文件都报 dead_code。
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Once;

use image::{ImageBuffer, Rgba};
use pdf_zhizhuan_lib::pdf::engine;
use pdf_zhizhuan_lib::pdf::images_to_pdf::{self, ImagesToPdfOptions};
use pdf_zhizhuan_lib::pdf::noop_progress;
use pdf_zhizhuan_lib::pdf::types::{
    CancelToken, ImageFitMode, OrientationOpt, PageSizeOpt, TaskCtx,
};

static ENGINE_INIT: Once = Once::new();

/// 初始化 PDFium 引擎。`engine::init` 自身是幂等的，这里再加一层 `Once`
/// 只是为了避免每个测试都走一遍候选路径探测。
pub fn init_engine() {
    ENGINE_INIT.call_once(|| {
        // 不传额外目录：engine 会退回到 `CARGO_MANIFEST_DIR/binaries/`，
        // 也就是 `scripts/download-pdfium.mjs` 的落点。
        engine::init(&[]).expect("应当能从 src-tauri/binaries 加载 libpdfium");
    });
}

/// 构造一个不会被取消、没有密码的任务上下文。
///
/// 顺便保证引擎已初始化：所有测试都从这里拿到 `TaskCtx`，
/// 因此不需要在每个用例里手动调 `init_engine`。
pub fn test_ctx(cache_dir: &Path) -> TaskCtx {
    init_engine();
    std::fs::create_dir_all(cache_dir).expect("缓存目录应当可创建");
    TaskCtx::new(cache_dir.to_path_buf(), None, CancelToken::new())
}

/// 生成一张纯色 PNG，返回其绝对路径。
pub fn write_png(dir: &Path, name: &str, width: u32, height: u32, color: [u8; 4]) -> PathBuf {
    std::fs::create_dir_all(dir).expect("输入目录应当可创建");
    let path = dir.join(name);
    let buffer = ImageBuffer::from_pixel(width, height, Rgba(color));
    buffer.save(&path).expect("PNG 应当写入成功");
    path
}

/// 生成一张纯色 JPEG，返回其绝对路径。
pub fn write_jpeg(dir: &Path, name: &str, width: u32, height: u32, color: [u8; 3]) -> PathBuf {
    std::fs::create_dir_all(dir).expect("输入目录应当可创建");
    let path = dir.join(name);
    let buffer = ImageBuffer::from_pixel(width, height, image::Rgb(color));
    buffer.save(&path).expect("JPEG 应当写入成功");
    path
}

/// 造一个 `pages` 页的 PDF：每页由一张不同颜色的 PNG 铺满生成，便于肉眼区分。
///
/// 返回生成的 PDF 绝对路径。
pub fn make_pdf(dir: &Path, name: &str, pages: usize, page_size: PageSizeOpt) -> PathBuf {
    init_engine();

    let inputs = dir.join(format!("inputs-{name}"));
    let sources = (0..pages.max(1))
        .map(|index| {
            let shade = (40 + index * 25) as u8;
            write_png(
                &inputs,
                &format!("p{index:03}.png"),
                320,
                440,
                [shade, 90, 160, 255],
            )
        })
        .collect::<Vec<_>>();

    let options = ImagesToPdfOptions {
        sources,
        page_size,
        orientation: OrientationOpt::Portrait,
        image_fit: ImageFitMode::Fit,
        margin_mm: 0.0,
        image_dpi: 96,
        output_name: Some(format!("{name}.pdf")),
    };

    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    let outputs = images_to_pdf::run(&ctx, dir, &options, &mut progress)
        .unwrap_or_else(|err| panic!("生成测试 PDF 失败: {err:?}"));

    let path = outputs.first().expect("应当产出一个文件").path.clone();
    assert!(path.exists(), "生成的 PDF 应当存在于 {}", path.display());
    path
}

/// 读取 PDF 页数。
pub fn page_count(path: &Path) -> u32 {
    init_engine();
    pdf_zhizhuan_lib::pdf::info::probe_with_pages(path, None, false)
        .unwrap_or_else(|err| panic!("探测 {} 失败: {err:?}", path.display()))
        .page_count
}

/// 读取 PDF 每页的 (宽, 高, 旋转角)。
pub fn page_geometry(path: &Path) -> Vec<(f32, f32, u16)> {
    init_engine();
    let info = pdf_zhizhuan_lib::pdf::info::probe(path, None)
        .unwrap_or_else(|err| panic!("探测 {} 失败: {err:?}", path.display()));
    info.pages
        .into_iter()
        .map(|page| (page.width_pt, page.height_pt, page.rotation_deg))
        .collect()
}

/// 读取图片像素尺寸。
pub fn image_size(path: &Path) -> (u32, u32) {
    let reader = image::ImageReader::open(path)
        .unwrap_or_else(|err| panic!("打开图片失败 {}: {err}", path.display()));
    reader
        .into_dimensions()
        .unwrap_or_else(|err| panic!("读取图片尺寸失败 {}: {err}", path.display()))
}

/// 列出目录下匹配扩展名的文件名（已排序）。
pub fn list_files(dir: &Path, extension: &str) -> Vec<String> {
    let mut names = std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("读取目录 {} 失败: {err}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().map(|e| e == extension).unwrap_or(false))
        .map(|path| {
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}
