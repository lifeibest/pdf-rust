//! PDF 压缩的集成测试。
//!
//! 夹具分两类：
//! - 噪点图 PDF（Flate 几乎压不动，模拟扫描件）：压缩后必然变小；
//! - 纯色图 PDF（Flate 极致压缩，模拟文字 / 矢量文档）：栅格化反而变大，触发兜底。

mod common;

use std::path::{Path, PathBuf};

use image::{ImageBuffer, Rgb};

use pdfai_cn_lib::error::AppError;
use pdfai_cn_lib::pdf::compress::{self, CompressOptions};
use pdfai_cn_lib::pdf::images_to_pdf::{self, ImagesToPdfOptions};
use pdfai_cn_lib::pdf::noop_progress;
use pdfai_cn_lib::pdf::types::{ImageFitMode, OrientationOpt, OutputFile, PageSizeOpt};

use common::{make_pdf, page_count, page_geometry, test_ctx};

fn run(dir: &Path, options: CompressOptions) -> Result<Vec<OutputFile>, AppError> {
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    compress::run(&ctx, &dir.join("out"), &options, &mut progress)
}

/// 伪随机噪点 PNG：Flate 无法有效压缩，嵌入 PDF 后体积接近原始像素量。
fn write_noise_png(dir: &Path, name: &str, width: u32, height: u32) -> PathBuf {
    std::fs::create_dir_all(dir).expect("输入目录应当可创建");
    let path = dir.join(name);
    let mut seed = 0x1234_5678u32;
    let buffer = ImageBuffer::from_fn(width, height, |_, _| {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let r = (seed >> 16) as u8;
        let g = (seed >> 8) as u8;
        let b = seed as u8;
        Rgb([r, g, b])
    });
    buffer.save(&path).expect("PNG 应当写入成功");
    path
}

/// 用噪点图造一个「扫描件」PDF。
fn make_noise_pdf(dir: &Path, name: &str, pages: usize) -> PathBuf {
    let inputs = dir.join(format!("inputs-{name}"));
    let sources = (0..pages.max(1))
        .map(|index| write_noise_png(&inputs, &format!("p{index:03}.png"), 600, 800))
        .collect::<Vec<_>>();

    let options = ImagesToPdfOptions {
        sources,
        page_size: PageSizeOpt::A4,
        orientation: OrientationOpt::Portrait,
        image_fit: ImageFitMode::Fit,
        margin_mm: 0.0,
        image_dpi: 96,
        output_name: Some(format!("{name}.pdf")),
    };
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    images_to_pdf::run(&ctx, dir, &options, &mut progress)
        .expect("生成测试 PDF 失败")[0]
        .path
        .clone()
}

#[test]
fn compresses_noisy_scan_and_preserves_geometry() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_noise_pdf(dir, "scan", 2);
    let original_size = std::fs::metadata(&source).expect("源文件应当存在").len();

    let outputs = run(
        dir,
        CompressOptions {
            source: source.clone(),
            dpi: 100,
            quality: 50,
            ..Default::default()
        },
    )
    .expect("压缩应当成功");

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].name, "scan_compressed_pdfai.cn.pdf");
    assert_eq!(outputs[0].pages, Some(2));
    assert!(
        outputs[0].size_bytes < original_size,
        "噪点扫描件压缩后应当变小：{} vs {original_size}",
        outputs[0].size_bytes
    );
    assert_eq!(page_count(&outputs[0].path), 2);

    // 页面尺寸保持 A4，旋转归零（渲染已把旋转 baked 进位图）
    let geometry = page_geometry(&outputs[0].path);
    for (width, height, rotation) in geometry {
        assert!((width - 595.0).abs() < 1.0, "页宽应当保持 A4：{width}");
        assert!((height - 842.0).abs() < 1.0, "页高应当保持 A4：{height}");
        assert_eq!(rotation, 0);
    }
    // 源文件不受影响
    assert_eq!(std::fs::metadata(&source).unwrap().len(), original_size);
}

#[test]
fn falls_back_to_original_when_result_is_larger() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    // 纯色页 Flate 体积极小，高分辨率栅格化必然变大
    let source = make_pdf(dir, "vector", 2, PageSizeOpt::A4);
    let original = std::fs::read(&source).expect("源文件应当可读");

    let outputs = run(
        dir,
        CompressOptions {
            source: source.clone(),
            dpi: 300,
            quality: 100,
            ..Default::default()
        },
    )
    .expect("兜底路径也应当成功");

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].size_bytes, original.len() as u64);
    assert_eq!(
        std::fs::read(&outputs[0].path).expect("产出应当可读"),
        original,
        "兜底时产出应当与源文件字节一致"
    );
}

#[test]
fn custom_output_name_is_respected() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_noise_pdf(dir, "scan", 1);

    let outputs = run(
        dir,
        CompressOptions {
            source,
            output_name: Some("small.pdf".to_string()),
            ..Default::default()
        },
    )
    .expect("压缩应当成功");

    assert_eq!(outputs[0].name, "small.pdf");
}

#[test]
fn clamps_extreme_options() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_noise_pdf(dir, "scan", 1);

    let outputs = run(
        dir,
        CompressOptions {
            source,
            dpi: 0,
            quality: 0,
            ..Default::default()
        },
    )
    .expect("极端参数应当被夹紧后正常处理");

    assert_eq!(outputs[0].pages, Some(1));
    assert!(outputs[0].path.exists());
}

#[test]
fn rejects_empty_source() {
    let dir = tempfile::tempdir().expect("临时目录");
    let result = run(dir.path(), CompressOptions::default());
    assert!(matches!(result, Err(AppError::InvalidParams(_))));
}

#[test]
fn reports_completion_summary_for_result_panel() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let mut progress = noop_progress();

    // 压缩有效分支：摘要带前后体积与减小比例
    let scan = make_noise_pdf(dir, "scan", 1);
    let ctx = test_ctx(&dir.join("cache"));
    compress::run(
        &ctx,
        &dir.join("out"),
        &CompressOptions {
            source: scan,
            dpi: 100,
            quality: 50,
            ..Default::default()
        },
        &mut progress,
    )
    .expect("压缩应当成功");
    let summary = ctx.summary().expect("压缩应当写入完成摘要");
    assert!(summary.contains("→") && summary.contains("减小"), "{summary}");

    // 兜底分支：摘要说明保留了原始文件
    let plain = make_pdf(dir, "vector", 1, PageSizeOpt::A4);
    let ctx = test_ctx(&dir.join("cache2"));
    compress::run(
        &ctx,
        &dir.join("out2"),
        &CompressOptions {
            source: plain,
            dpi: 300,
            quality: 100,
            ..Default::default()
        },
        &mut progress,
    )
    .expect("兜底应当成功");
    let summary = ctx.summary().expect("兜底也应当写入完成摘要");
    assert!(summary.contains("保留原始文件"), "{summary}");
}