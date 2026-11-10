//! 发票合并（N-up 拼版）的集成测试。

mod common;

use pdfai_cn_lib::error::AppError;
use pdfai_cn_lib::pdf::invoice::{self, InvoiceMergeOptions};
use pdfai_cn_lib::pdf::merge::MergeSource;
use pdfai_cn_lib::pdf::noop_progress;
use pdfai_cn_lib::pdf::types::{OrientationOpt, OutputFile, PageSizeOpt};

use common::{make_pdf, page_count, page_geometry, test_ctx, write_png};

const A4_SHORT: f32 = 595.276;
const A4_LONG: f32 = 841.89;

fn run(dir: &std::path::Path, options: InvoiceMergeOptions) -> Result<Vec<OutputFile>, AppError> {
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    invoice::run(&ctx, &dir.join("out"), &options, &mut progress)
}

fn source(path: std::path::PathBuf) -> MergeSource {
    MergeSource {
        path,
        password: None,
    }
}

/// 断言页面是 A4（纵向或横向皆可）。
fn assert_a4(width: f32, height: f32) {
    let portrait = (width - A4_SHORT).abs() < 1.0 && (height - A4_LONG).abs() < 1.0;
    let landscape = (width - A4_LONG).abs() < 1.0 && (height - A4_SHORT).abs() < 1.0;
    assert!(
        portrait || landscape,
        "输出页应为 A4，实际为 {width} x {height}"
    );
}

#[test]
fn two_invoices_tile_onto_one_a4() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let a = make_pdf(dir, "inv-a", 1, PageSizeOpt::A5);
    let b = make_pdf(dir, "inv-b", 1, PageSizeOpt::A5);

    let outputs = run(
        dir,
        InvoiceMergeOptions {
            sources: vec![source(a), source(b)],
            per_sheet: 2,
            orientation: OrientationOpt::Portrait,
            output_name: Some("invoices.pdf".to_string()),
            ..Default::default()
        },
    )
    .expect("发票合并应当成功");

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].name, "invoices.pdf");
    assert_eq!(page_count(&outputs[0].path), 1, "两张发票应拼到同一页 A4");
    assert_eq!(outputs[0].pages, Some(1));

    let (width, height, _) = page_geometry(&outputs[0].path)[0];
    assert!((width - A4_SHORT).abs() < 1.0 && (height - A4_LONG).abs() < 1.0, "纵向应为竖版 A4: {width} x {height}");
}

#[test]
fn multiple_invoices_flow_to_extra_sheets() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    // 3 张发票，每页 2 张 → ceil(3/2) = 2 页 A4
    let a = make_pdf(dir, "inv-a", 1, PageSizeOpt::A5);
    let b = make_pdf(dir, "inv-b", 1, PageSizeOpt::A5);
    let c = make_pdf(dir, "inv-c", 1, PageSizeOpt::A5);

    let outputs = run(
        dir,
        InvoiceMergeOptions {
            sources: vec![source(a), source(b), source(c)],
            per_sheet: 2,
            orientation: OrientationOpt::Portrait,
            ..Default::default()
        },
    )
    .expect("发票合并应当成功");

    assert_eq!(page_count(&outputs[0].path), 2, "3 张发票每页 2 张应产出 2 页");
    assert_eq!(outputs[0].pages, Some(2));
    for (width, height, _) in page_geometry(&outputs[0].path) {
        assert_a4(width, height);
    }
}

#[test]
fn multipage_pdf_is_flattened_per_page() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    // 一个 3 页 PDF 视作 3 张发票，每页 4 张 → 1 页 A4
    let multi = make_pdf(dir, "multi", 3, PageSizeOpt::A5);

    let outputs = run(
        dir,
        InvoiceMergeOptions {
            sources: vec![source(multi)],
            per_sheet: 4,
            orientation: OrientationOpt::Landscape,
            ..Default::default()
        },
    )
    .expect("发票合并应当成功");

    assert_eq!(page_count(&outputs[0].path), 1, "3 页发票按每页 4 张应拼成 1 页");
    let (width, height, _) = page_geometry(&outputs[0].path)[0];
    assert!((width - A4_LONG).abs() < 1.0 && (height - A4_SHORT).abs() < 1.0, "横向应为横版 A4: {width} x {height}");
}

#[test]
fn mixes_pdf_and_image_invoices() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let pdf = make_pdf(dir, "inv-pdf", 1, PageSizeOpt::A5);
    // 一张横向图片发票（如中国增值税发票的比例）
    let img = write_png(dir, "inv-img.png", 960, 560, [240, 240, 240, 255]);

    let outputs = run(
        dir,
        InvoiceMergeOptions {
            sources: vec![source(pdf), source(img)],
            per_sheet: 2,
            // Auto：首张发票为竖向 → 应选横向 A4 左右并排
            orientation: OrientationOpt::Auto,
            ..Default::default()
        },
    )
    .expect("PDF 与图片混合合并应当成功");

    assert_eq!(page_count(&outputs[0].path), 1);
    let (width, height, _) = page_geometry(&outputs[0].path)[0];
    assert_a4(width, height);
}

#[test]
fn rejects_empty_source_list() {
    let dir = tempfile::tempdir().expect("临时目录");
    let result = run(dir.path(), InvoiceMergeOptions::default());
    assert!(matches!(result, Err(AppError::InvalidParams(_))));
}

#[test]
fn rejects_unsupported_file_type() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let text = dir.join("notes.txt");
    std::fs::write(&text, "not an invoice").expect("写入测试文件");

    let result = run(
        dir,
        InvoiceMergeOptions {
            sources: vec![source(text)],
            ..Default::default()
        },
    );
    assert!(
        matches!(result, Err(AppError::Unsupported(_))),
        "不支持的类型应报 Unsupported，实际为 {result:?}"
    );
}

#[test]
fn does_not_overwrite_existing_output() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let out_dir = dir.join("out");
    std::fs::create_dir_all(&out_dir).expect("输出目录");
    std::fs::write(out_dir.join("dup.pdf"), b"placeholder").expect("写入占位文件");

    let a = make_pdf(dir, "dup-src", 1, PageSizeOpt::A5);
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    let outputs = invoice::run(
        &ctx,
        &out_dir,
        &InvoiceMergeOptions {
            sources: vec![source(a)],
            per_sheet: 1,
            output_name: Some("dup.pdf".to_string()),
            ..Default::default()
        },
        &mut progress,
    )
    .expect("发票合并应当成功");

    assert_eq!(outputs[0].name, "dup-1.pdf", "同名文件应自动改名");
    assert_eq!(
        std::fs::read(out_dir.join("dup.pdf")).unwrap(),
        b"placeholder",
        "原有文件不应被覆盖"
    );
}
