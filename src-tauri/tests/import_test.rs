//! 图片转 PDF 的集成测试。

mod common;

use std::path::Path;

use pdf_zhizhuan_lib::pdf::images_to_pdf::{self, ImagesToPdfOptions};
use pdf_zhizhuan_lib::pdf::noop_progress;
use pdf_zhizhuan_lib::pdf::types::{ImageFitMode, OrientationOpt, PageSizeOpt};

use common::{image_size, make_pdf, page_count, page_geometry, test_ctx, write_jpeg, write_png};

/// A4 尺寸（点）：595.276 x 841.89
const A4_WIDTH: f32 = 595.276;
const A4_HEIGHT: f32 = 841.89;

fn run(dir: &Path, options: ImagesToPdfOptions) -> std::path::PathBuf {
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    let outputs = images_to_pdf::run(&ctx, dir, &options, &mut progress)
        .unwrap_or_else(|err| panic!("图片转 PDF 失败: {err:?}"));
    assert_eq!(outputs.len(), 1, "应当只产出一个 PDF");
    outputs[0].path.clone()
}

#[test]
fn one_page_per_image_with_fixed_paper_size() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let sources = vec![
        write_png(dir, "a.png", 800, 600, [200, 40, 40, 255]),
        write_png(dir, "b.png", 300, 900, [40, 200, 40, 255]),
        write_jpeg(dir, "c.jpg", 500, 500, [40, 40, 200]),
    ];

    let output = run(
        dir,
        ImagesToPdfOptions {
            sources,
            page_size: PageSizeOpt::A4,
            orientation: OrientationOpt::Portrait,
            image_fit: ImageFitMode::Fit,
            margin_mm: 20.0,
            image_dpi: 96,
            output_name: Some("a4-20mm.pdf".to_string()),
        },
    );

    assert_eq!(page_count(&output), 3, "三张图片应当生成三页");
    for (width, height, rotation) in page_geometry(&output) {
        assert!((width - A4_WIDTH).abs() < 1.0, "页宽应为 A4: {width}");
        assert!((height - A4_HEIGHT).abs() < 1.0, "页高应为 A4: {height}");
        assert_eq!(rotation, 0, "新建页面不应带旋转");
    }
}

#[test]
fn auto_page_size_follows_image_pixels_and_dpi() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    // 96 dpi 下 192px = 144pt；再加两侧各 10mm 留白
    let sources = vec![write_png(dir, "auto.png", 192, 96, [10, 20, 30, 255])];
    let output = run(
        dir,
        ImagesToPdfOptions {
            sources,
            page_size: PageSizeOpt::Auto,
            orientation: OrientationOpt::Auto,
            image_fit: ImageFitMode::NativeDpi,
            margin_mm: 10.0,
            image_dpi: 96,
            output_name: Some("auto.pdf".to_string()),
        },
    );

    let margin_pt = 10.0 * 72.0 / 25.4;
    let geometry = page_geometry(&output);
    assert_eq!(geometry.len(), 1);
    let (width, height, _) = geometry[0];
    assert!(
        (width - (144.0 + 2.0 * margin_pt)).abs() < 1.0,
        "宽: {width}"
    );
    assert!(
        (height - (72.0 + 2.0 * margin_pt)).abs() < 1.0,
        "高: {height}"
    );
}

#[test]
fn landscape_orientation_swaps_paper_dimensions() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let sources = vec![write_png(dir, "v.png", 200, 800, [1, 2, 3, 255])];
    let output = run(
        dir,
        ImagesToPdfOptions {
            sources,
            page_size: PageSizeOpt::A4,
            orientation: OrientationOpt::Landscape,
            image_fit: ImageFitMode::Fit,
            margin_mm: 0.0,
            image_dpi: 96,
            output_name: Some("landscape.pdf".to_string()),
        },
    );

    let (width, height, _) = page_geometry(&output)[0];
    assert!(width > height, "横向页面宽应大于高: {width} x {height}");
    assert!((width - A4_HEIGHT).abs() < 1.0);
    assert!((height - A4_WIDTH).abs() < 1.0);
}

#[test]
fn rejects_non_image_sources() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    // 用 make_pdf 造一个真实的 PDF 当作非法输入
    let pdf = make_pdf(dir, "not-an-image", 1, PageSizeOpt::A4);
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    let result = images_to_pdf::run(
        &ctx,
        dir,
        &ImagesToPdfOptions {
            sources: vec![pdf],
            ..Default::default()
        },
        &mut progress,
    );

    assert!(
        matches!(
            result,
            Err(pdf_zhizhuan_lib::error::AppError::Unsupported(_))
        ),
        "非图片输入应报 Unsupported，实际为 {result:?}"
    );
}

#[test]
fn rejects_empty_source_list() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    let result = images_to_pdf::run(&ctx, dir, &ImagesToPdfOptions::default(), &mut progress);
    assert!(matches!(
        result,
        Err(pdf_zhizhuan_lib::error::AppError::InvalidParams(_))
    ));
}

#[test]
fn generated_pdf_can_be_rendered_back_to_image() {
    // 闭环校验：图片 → PDF → 图片，尺寸应当与原始图片的宽高比一致
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let output = run(
        dir,
        ImagesToPdfOptions {
            sources: vec![write_png(dir, "loop.png", 400, 200, [250, 120, 0, 255])],
            page_size: PageSizeOpt::Auto,
            orientation: OrientationOpt::Auto,
            image_fit: ImageFitMode::Fit,
            margin_mm: 0.0,
            image_dpi: 96,
            output_name: Some("loop.pdf".to_string()),
        },
    );

    let exported = pdf_zhizhuan_lib::pdf::export_images::run(
        &test_ctx(&dir.join("cache")),
        &dir.join("out"),
        &pdf_zhizhuan_lib::pdf::export_images::ExportImagesOptions {
            source: output.clone(),
            dpi: 150,
            format: pdf_zhizhuan_lib::pdf::ImageFormatOpt::Png,
            ..Default::default()
        },
        &mut noop_progress(),
    )
    .expect("导出应当成功");

    assert_eq!(exported.len(), 1);
    let (width, height) = image_size(&exported[0].path);
    // 400x200 @96dpi = 300x150pt，@150dpi 渲染 = 625x312px
    assert_eq!(width, 625);
    assert!((height as i32 - 313).abs() <= 2, "高: {height}");
}
