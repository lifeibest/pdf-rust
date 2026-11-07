//! PDF 转图片的集成测试。

mod common;

use std::path::Path;

use pdf_zhizhuan_lib::error::AppError;
use pdf_zhizhuan_lib::pdf::export_images::{self, ExportImagesOptions};
use pdf_zhizhuan_lib::pdf::noop_progress;
use pdf_zhizhuan_lib::pdf::types::{ImageFormatOpt, OutputFile, PageScope, PageSizeOpt};

use common::{image_size, list_files, make_pdf, test_ctx};

/// A4 竖向页宽（点）
const A4_WIDTH_PT: f32 = 595.276;

fn run(dir: &Path, options: ExportImagesOptions) -> Result<Vec<OutputFile>, AppError> {
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    export_images::run(&ctx, &dir.join("out"), &options, &mut progress)
}

#[test]
fn exports_selected_pages_at_requested_dpi() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 6, PageSizeOpt::A4);

    let outputs = run(
        dir,
        ExportImagesOptions {
            source,
            scope: PageScope::Spec("1,3-5".to_string()),
            dpi: 150,
            format: ImageFormatOpt::Png,
            ..Default::default()
        },
    )
    .expect("导出应当成功");

    assert_eq!(outputs.len(), 4, "1,3-5 共四页");
    assert_eq!(
        list_files(&dir.join("out"), "png"),
        vec![
            "doc_p001.png",
            "doc_p003.png",
            "doc_p004.png",
            "doc_p005.png"
        ],
        "文件名应当带三位页码，便于按名排序"
    );

    let expected_width = (A4_WIDTH_PT * 150.0 / 72.0).round() as u32;
    for output in &outputs {
        let (width, height) = image_size(&output.path);
        assert_eq!(width, expected_width, "宽度应等于 页宽(pt) * dpi / 72");
        assert!(height > width, "A4 竖向渲染结果应当是高大于宽");
    }
}

#[test]
fn dpi_controls_output_resolution() {
    // 两次导出必须落在不同目录，否则同名文件会互相覆盖，测不到真实分辨率
    let low_root = tempfile::tempdir().expect("临时目录");
    let high_root = tempfile::tempdir().expect("临时目录");
    let low_dir = low_root.path();
    let high_dir = high_root.path();

    let low_source = make_pdf(low_dir, "doc", 1, PageSizeOpt::A4);
    let high_source = make_pdf(high_dir, "doc", 1, PageSizeOpt::A4);

    let low = run(
        low_dir,
        ExportImagesOptions {
            source: low_source,
            dpi: 72,
            format: ImageFormatOpt::Png,
            ..Default::default()
        },
    )
    .expect("导出应当成功");

    let high = run(
        high_dir,
        ExportImagesOptions {
            source: high_source,
            dpi: 300,
            format: ImageFormatOpt::Png,
            ..Default::default()
        },
    )
    .expect("导出应当成功");

    let (low_width, _) = image_size(&low[0].path);
    let (high_width, _) = image_size(&high[0].path);
    assert_eq!(low_width, A4_WIDTH_PT.round() as u32, "72 dpi 时 1pt = 1px");
    assert!(
        high_width > low_width * 3,
        "300 dpi 应当显著大于 72 dpi: {high_width} vs {low_width}"
    );
}

#[test]
fn max_edge_clamps_oversized_renders() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 1, PageSizeOpt::A4);

    let outputs = run(
        dir,
        ExportImagesOptions {
            source,
            dpi: 600,
            max_edge_px: Some(400),
            format: ImageFormatOpt::Png,
            ..Default::default()
        },
    )
    .expect("导出应当成功");

    let (width, height) = image_size(&outputs[0].path);
    assert!(width <= 400, "宽应受上限约束: {width}");
    assert!(height <= 400, "高应受上限约束: {height}");
}

#[test]
fn writes_jpeg_and_webp_with_expected_extensions() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 2, PageSizeOpt::A4);

    let jpeg = run(
        dir,
        ExportImagesOptions {
            source: source.clone(),
            dpi: 96,
            format: ImageFormatOpt::Jpeg,
            quality: 60,
            ..Default::default()
        },
    )
    .expect("JPEG 导出应当成功");
    assert_eq!(list_files(&dir.join("out"), "jpg").len(), 2);
    assert!(jpeg[0].name.ends_with(".jpg"));

    let out_dir = dir.join("webp");
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    let webp = export_images::run(
        &ctx,
        &out_dir,
        &ExportImagesOptions {
            source,
            dpi: 96,
            format: ImageFormatOpt::Webp,
            ..Default::default()
        },
        &mut progress,
    )
    .expect("WebP 导出应当成功");

    assert_eq!(list_files(&out_dir, "webp").len(), 2);
    assert!(webp[0].name.ends_with(".webp"));
    // WebP 无损编码同样可以被读回
    let (width, _) = image_size(&webp[0].path);
    assert!(width > 0);
}

#[test]
fn zip_packs_all_images_into_single_archive() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 3, PageSizeOpt::A4);

    let outputs = run(
        dir,
        ExportImagesOptions {
            source,
            dpi: 72,
            format: ImageFormatOpt::Png,
            zip: true,
            output_name: Some("pages.zip".to_string()),
            ..Default::default()
        },
    )
    .expect("导出并打包应当成功");

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].name, "pages.zip");
    assert!(matches!(
        outputs[0].kind,
        pdf_zhizhuan_lib::pdf::OutputKind::Archive
    ));
    // 打包模式下不应在输出目录里散落图片
    assert!(list_files(&dir.join("out"), "png").is_empty());

    let file = std::fs::File::open(&outputs[0].path).expect("zip 应当可读");
    let archive = zip::ZipArchive::new(file).expect("zip 应当可解析");
    assert_eq!(archive.len(), 3, "压缩包内应有三张图片");
}

#[test]
fn keyword_scopes_are_supported() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 5, PageSizeOpt::A4);

    let outputs = run(
        dir,
        ExportImagesOptions {
            source,
            scope: PageScope::Even,
            dpi: 72,
            format: ImageFormatOpt::Png,
            ..Default::default()
        },
    )
    .expect("导出应当成功");

    assert_eq!(
        outputs
            .iter()
            .map(|output| output.name.clone())
            .collect::<Vec<_>>(),
        vec!["doc_p002.png", "doc_p004.png"]
    );
}

#[test]
fn rejects_out_of_range_scope() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 2, PageSizeOpt::A4);

    let result = run(
        dir,
        ExportImagesOptions {
            source,
            scope: PageScope::Spec("9".to_string()),
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(AppError::Range(_))));
}
