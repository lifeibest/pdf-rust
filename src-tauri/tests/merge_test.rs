//! PDF 合并的集成测试。

mod common;

use pdfai_cn_lib::error::AppError;
use pdfai_cn_lib::pdf::merge::{self, MergeOptions, MergeSource};
use pdfai_cn_lib::pdf::noop_progress;
use pdfai_cn_lib::pdf::types::{ImageFitMode, OrientationOpt, OutputFile, PageSizeOpt};

use common::{make_pdf, page_count, page_geometry, test_ctx, write_jpeg, write_png};

const A4_WIDTH: f32 = 595.276;
const A4_HEIGHT: f32 = 841.89;

fn run(dir: &std::path::Path, options: MergeOptions) -> Result<Vec<OutputFile>, AppError> {
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    merge::run(&ctx, &dir.join("out"), &options, &mut progress)
}

#[test]
fn merges_pdfs_and_images_in_given_order() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let first = make_pdf(dir, "first", 3, PageSizeOpt::A4);
    let second = make_pdf(dir, "second", 2, PageSizeOpt::Letter);
    let png = write_png(dir, "photo.png", 640, 480, [220, 30, 30, 255]);
    let jpg = write_jpeg(dir, "photo.jpg", 480, 640, [30, 220, 30]);

    let outputs = run(
        dir,
        MergeOptions {
            sources: vec![
                MergeSource {
                    path: first.clone(),
                    password: None,
                },
                MergeSource {
                    path: png,
                    password: None,
                },
                MergeSource {
                    path: second.clone(),
                    password: None,
                },
                MergeSource {
                    path: jpg,
                    password: None,
                },
            ],
            page_size: PageSizeOpt::A4,
            orientation: OrientationOpt::Auto,
            image_fit: ImageFitMode::Fit,
            margin_mm: 10.0,
            image_dpi: 96,
            flatten_annotations: false,
            output_name: Some("combined.pdf".to_string()),
        },
    )
    .expect("合并应当成功");

    assert_eq!(outputs.len(), 1);
    let merged = &outputs[0].path;
    assert_eq!(merged.file_name().unwrap(), "combined.pdf");
    // 3 + 1(图片) + 2 + 1(图片) = 7
    assert_eq!(page_count(merged), 7, "页数应为各来源之和");
    assert_eq!(outputs[0].pages, Some(7));

    let geometry = page_geometry(merged);
    // 前三页来自 A4 PDF
    for (width, height, _) in &geometry[0..3] {
        assert!((width - A4_WIDTH).abs() < 1.0, "A4 页宽: {width}");
        assert!((height - A4_HEIGHT).abs() < 1.0, "A4 页高: {height}");
    }
    // 第 4 页是 640x480 的横向图片，Auto 方向应选横向 A4
    let (width, height, _) = geometry[3];
    assert!(width > height, "横向图片应生成横向页: {width} x {height}");
    // 第 5、6 页来自 Letter PDF，尺寸应保持原样而不被 page_size 覆盖
    let (width, height, _) = geometry[4];
    assert!(
        (width - 612.0).abs() < 1.0,
        "Letter 页宽应保持 612: {width}"
    );
    assert!(
        (height - 792.0).abs() < 1.0,
        "Letter 页高应保持 792: {height}"
    );
    // 第 7 页是 480x640 的竖向图片
    let (width, height, _) = geometry[6];
    assert!(height > width, "竖向图片应生成竖向页: {width} x {height}");
}

#[test]
fn single_source_merge_is_a_valid_copy() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let source = make_pdf(dir, "solo", 4, PageSizeOpt::A4);
    let outputs = run(
        dir,
        MergeOptions {
            sources: vec![MergeSource {
                path: source,
                password: None,
            }],
            ..Default::default()
        },
    )
    .expect("单来源合并应当成功");

    assert_eq!(page_count(&outputs[0].path), 4);
}

#[test]
fn rejects_empty_source_list() {
    let dir = tempfile::tempdir().expect("临时目录");
    let result = run(dir.path(), MergeOptions::default());
    assert!(matches!(result, Err(AppError::InvalidParams(_))));
}

#[test]
fn rejects_unsupported_file_type() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let text = dir.join("notes.txt");
    std::fs::write(&text, "not a pdf").expect("写入测试文件");

    let result = run(
        dir,
        MergeOptions {
            sources: vec![MergeSource {
                path: text,
                password: None,
            }],
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
    // 预先占位同名文件，验证 resolve_output_path 会追加序号而不是静默覆盖
    std::fs::write(out_dir.join("dup.pdf"), b"placeholder").expect("写入占位文件");

    let source = make_pdf(dir, "dup-src", 1, PageSizeOpt::A4);
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    let outputs = merge::run(
        &ctx,
        &out_dir,
        &MergeOptions {
            sources: vec![MergeSource {
                path: source,
                password: None,
            }],
            output_name: Some("dup.pdf".to_string()),
            ..Default::default()
        },
        &mut progress,
    )
    .expect("合并应当成功");

    assert_eq!(
        outputs[0].name, "dup-1.pdf",
        "同名文件应当自动改名，实际为 {}",
        outputs[0].name
    );
    assert_eq!(
        std::fs::read(out_dir.join("dup.pdf")).unwrap(),
        b"placeholder",
        "原有文件不应被覆盖"
    );
}
