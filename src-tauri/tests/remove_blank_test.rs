//! PDF 删除空白页的集成测试。
//!
//! 夹具：用 `images_to_pdf` 把纯白 PNG（空白页）与纯色 PNG（内容页）混排成 PDF；
//! 阈值灵敏度用例另造「白纸中央一个 24px 黑点」的页面（约占 0.4% 面积）。

mod common;

use std::path::{Path, PathBuf};

use image::{ImageBuffer, Rgba};

use pdfai_cn_lib::error::AppError;
use pdfai_cn_lib::pdf::images_to_pdf::{self, ImagesToPdfOptions};
use pdfai_cn_lib::pdf::noop_progress;
use pdfai_cn_lib::pdf::remove_blank::{self, RemoveBlankOptions};
use pdfai_cn_lib::pdf::types::{ImageFitMode, OrientationOpt, OutputFile, PageSizeOpt};

use common::{page_count, test_ctx, write_png};

fn run(dir: &Path, options: RemoveBlankOptions) -> Result<Vec<OutputFile>, AppError> {
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    remove_blank::run(&ctx, &dir.join("out"), &options, &mut progress)
}

/// 按 `blanks` 标记混排空白页（纯白）与内容页（纯色），返回 PDF 路径。
fn make_mixed_pdf(dir: &Path, name: &str, blanks: &[bool], page_size: PageSizeOpt) -> PathBuf {
    common::init_engine();
    let inputs = dir.join(format!("inputs-{name}"));
    let sources = blanks
        .iter()
        .enumerate()
        .map(|(index, &blank)| {
            let color = if blank {
                [255, 255, 255, 255]
            } else {
                [40, 90, 160, 255]
            };
            write_png(&inputs, &format!("p{index:03}.png"), 320, 440, color)
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
    outputs.first().expect("应当产出一个文件").path.clone()
}

/// 造一个「白纸 + 中央 24px 黑点」的页面源（黑点约占 0.4% 面积）。
fn write_speck_png(dir: &Path, name: &str) -> PathBuf {
    std::fs::create_dir_all(dir).expect("输入目录应当可创建");
    let path = dir.join(name);
    let buffer = ImageBuffer::from_fn(320, 440, |x, y| {
        if (148..172).contains(&x) && (208..232).contains(&y) {
            Rgba([0u8, 0, 0, 255])
        } else {
            Rgba([255u8, 255, 255, 255])
        }
    });
    buffer.save(&path).expect("PNG 应当写入成功");
    path
}

#[test]
fn removes_blank_pages_and_keeps_content() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    // 第 2、4 页空白
    let source = make_mixed_pdf(dir, "doc", &[false, true, false, true], PageSizeOpt::A4);

    let outputs = run(
        dir,
        RemoveBlankOptions {
            source: source.clone(),
            ..Default::default()
        },
    )
    .expect("删除空白页应当成功");

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].name, "doc_noblank_pdfai.cn.pdf");
    assert_eq!(outputs[0].pages, Some(2));
    assert_eq!(page_count(&outputs[0].path), 2);
    // 另存模式不应改动源文件
    assert_eq!(page_count(&source), 4);
}

#[test]
fn in_place_overwrites_source_atomically() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_mixed_pdf(dir, "doc", &[true, false, true], PageSizeOpt::A4);

    let outputs = run(
        dir,
        RemoveBlankOptions {
            source: source.clone(),
            in_place: true,
            ..Default::default()
        },
    )
    .expect("原地删除应当成功");

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].path, source, "原地模式产出即源文件");
    assert_eq!(page_count(&source), 1);
    // 原地模式不应产生输出目录
    assert!(!dir.join("out").exists(), "原地模式不应创建输出目录");
}

#[test]
fn no_blank_pages_is_reported() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_mixed_pdf(dir, "doc", &[false, false], PageSizeOpt::A4);

    let result = run(
        dir,
        RemoveBlankOptions {
            source,
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(AppError::Pdf(_))));
}

#[test]
fn all_blank_is_rejected() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_mixed_pdf(dir, "doc", &[true, true], PageSizeOpt::A4);

    let result = run(
        dir,
        RemoveBlankOptions {
            source,
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(AppError::Pdf(_))));
}

#[test]
fn threshold_controls_sensitivity() {
    // 三页：黑点页（约 0.4% 墨量）+ 纯白页 + 内容页。
    // 默认阈值 0.5%：黑点页与白页都删，剩 1 页；
    // 阈值 0.2%：黑点页保留，只删白页，剩 2 页。
    let build = |root: &Path| {
        let inputs = root.join("inputs");
        let speck = write_speck_png(&inputs, "speck.png");
        let white = write_png(&inputs, "white.png", 320, 440, [255, 255, 255, 255]);
        let color = write_png(&inputs, "color.png", 320, 440, [40, 90, 160, 255]);
        let options = ImagesToPdfOptions {
            sources: vec![speck, white, color],
            // Auto 让页面与图片同尺寸，墨量占比不被页边白稀释
            page_size: PageSizeOpt::Auto,
            orientation: OrientationOpt::Portrait,
            image_fit: ImageFitMode::Fit,
            margin_mm: 0.0,
            image_dpi: 96,
            output_name: Some("doc.pdf".to_string()),
        };
        let ctx = test_ctx(&root.join("cache"));
        let mut progress = noop_progress();
        images_to_pdf::run(&ctx, root, &options, &mut progress)
            .expect("生成测试 PDF 失败")[0]
            .path
            .clone()
    };

    let loose_root = tempfile::tempdir().expect("临时目录");
    let loose = build(loose_root.path());
    let loose_out = run(
        loose_root.path(),
        RemoveBlankOptions {
            source: loose,
            ..Default::default()
        },
    )
    .expect("默认阈值应当成功");
    assert_eq!(loose_out[0].pages, Some(1), "0.5% 阈值下黑点页应被判为空白");

    let strict_root = tempfile::tempdir().expect("临时目录");
    let strict = build(strict_root.path());
    let strict_out = run(
        strict_root.path(),
        RemoveBlankOptions {
            source: strict,
            threshold: 0.2,
            ..Default::default()
        },
    )
    .expect("严格阈值应当成功");
    assert_eq!(strict_out[0].pages, Some(2), "0.2% 阈值下黑点页应被保留");
}

#[test]
fn custom_output_name_is_respected() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_mixed_pdf(dir, "doc", &[true, false], PageSizeOpt::A4);

    let outputs = run(
        dir,
        RemoveBlankOptions {
            source,
            output_name: Some("clean.pdf".to_string()),
            ..Default::default()
        },
    )
    .expect("删除空白页应当成功");

    assert_eq!(outputs[0].name, "clean.pdf");
}