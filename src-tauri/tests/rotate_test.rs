//! PDF 旋转的集成测试。

mod common;

use std::path::Path;

use pdfai_cn_lib::error::AppError;
use pdfai_cn_lib::pdf::noop_progress;
use pdfai_cn_lib::pdf::rotate::{self, RotateOptions};
use pdfai_cn_lib::pdf::types::{OutputFile, PageScope, PageSizeOpt};

use common::{make_pdf, page_count, page_geometry, test_ctx};

/// A4 竖向页面的宽高（点）
const A4_WIDTH: f32 = 595.276;
const A4_HEIGHT: f32 = 841.89;

fn run(dir: &Path, options: RotateOptions) -> Result<Vec<OutputFile>, AppError> {
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    rotate::run(&ctx, &dir.join("out"), &options, &mut progress)
}

fn rotations(path: &Path) -> Vec<u16> {
    page_geometry(path)
        .into_iter()
        .map(|(_, _, rotation)| rotation)
        .collect()
}

#[test]
fn rotates_all_pages_by_90_degrees() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 4, PageSizeOpt::A4);

    let outputs = run(
        dir,
        RotateOptions {
            source,
            degrees: 90,
            scope: PageScope::All,
            ..Default::default()
        },
    )
    .expect("旋转应当成功");

    assert_eq!(outputs.len(), 1);
    assert_eq!(page_count(&outputs[0].path), 4, "旋转不应改变页数");
    assert_eq!(rotations(&outputs[0].path), vec![90, 90, 90, 90]);
}

#[test]
fn rotation_stacks_and_normalizes() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 2, PageSizeOpt::A4);

    // 第一次旋转 90
    let first = run(
        dir,
        RotateOptions {
            source: source.clone(),
            degrees: 90,
            scope: PageScope::All,
            output_name: Some("step1.pdf".to_string()),
            ..Default::default()
        },
    )
    .expect("第一次旋转应当成功");
    assert_eq!(rotations(&first[0].path), vec![90, 90]);

    // 在已旋转 90 度的结果上再转 270，应回到 0 度而不是 360
    let second = run(
        dir,
        RotateOptions {
            source: first[0].path.clone(),
            degrees: 270,
            scope: PageScope::All,
            output_name: Some("step2.pdf".to_string()),
            ..Default::default()
        },
    )
    .expect("第二次旋转应当成功");
    assert_eq!(rotations(&second[0].path), vec![0, 0]);
}

#[test]
fn negative_degrees_rotate_counter_clockwise() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 1, PageSizeOpt::A4);

    let outputs = run(
        dir,
        RotateOptions {
            source,
            degrees: -90,
            scope: PageScope::All,
            ..Default::default()
        },
    )
    .expect("旋转应当成功");

    assert_eq!(rotations(&outputs[0].path), vec![270]);
}

#[test]
fn scope_limits_affected_pages() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 6, PageSizeOpt::A4);

    let odd = run(
        dir,
        RotateOptions {
            source: source.clone(),
            degrees: 180,
            scope: PageScope::Odd,
            output_name: Some("odd.pdf".to_string()),
            ..Default::default()
        },
    )
    .expect("旋转应当成功");
    assert_eq!(rotations(&odd[0].path), vec![180, 0, 180, 0, 180, 0]);

    let spec = run(
        dir,
        RotateOptions {
            source,
            degrees: 90,
            scope: PageScope::Spec("2,5-6".to_string()),
            output_name: Some("spec.pdf".to_string()),
            ..Default::default()
        },
    )
    .expect("旋转应当成功");
    assert_eq!(rotations(&spec[0].path), vec![0, 90, 0, 0, 90, 90]);
}

#[test]
fn in_place_overwrites_source_without_corrupting_it() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 3, PageSizeOpt::A4);
    let original_size = std::fs::metadata(&source).expect("源文件应当存在").len();

    let outputs = run(
        dir,
        RotateOptions {
            source: source.clone(),
            degrees: 90,
            scope: PageScope::All,
            in_place: true,
            ..Default::default()
        },
    )
    .expect("原地旋转应当成功");

    assert_eq!(outputs[0].path, source, "原地模式应返回源路径");
    assert_eq!(rotations(&source), vec![90, 90, 90]);
    assert!(
        std::fs::metadata(&source).expect("源文件应当仍存在").len() > 0,
        "原地保存后文件不应为空（原 {original_size} 字节）"
    );
    // 不应留下隐藏临时文件
    let leftovers = std::fs::read_dir(dir)
        .expect("目录应当可读")
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(".pdfzt-"))
        .count();
    assert_eq!(leftovers, 0, "原地保存不应残留临时文件");
}

#[test]
fn page_geometry_reflects_rotation() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 1, PageSizeOpt::A4);

    let before = page_geometry(&source);
    let outputs = run(
        dir,
        RotateOptions {
            source,
            degrees: 90,
            scope: PageScope::All,
            ..Default::default()
        },
    )
    .expect("旋转应当成功");
    let after = page_geometry(&outputs[0].path);

    let (bw, bh, br) = before[0];
    let (aw, ah, ar) = after[0];
    assert_eq!(br, 0);
    assert_eq!(ar, 90);
    // 记录的宽高应当与旋转角自洽：90/270 度时宽高互换
    assert!((bw - A4_WIDTH).abs() < 1.0 && (bh - A4_HEIGHT).abs() < 1.0);
    if ar == 90 || ar == 270 {
        assert!(
            (aw - bh).abs() < 1.0 && (ah - bw).abs() < 1.0,
            "旋转 90 度后宽高应互换: {aw}x{ah} vs {bw}x{bh}"
        );
    }
}

#[test]
fn rejects_angles_that_are_not_quarter_turns() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 1, PageSizeOpt::A4);

    let result = run(
        dir,
        RotateOptions {
            source,
            degrees: 45,
            scope: PageScope::All,
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(AppError::InvalidParams(_))));
}

#[test]
fn rejects_out_of_range_scope() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 2, PageSizeOpt::A4);

    let result = run(
        dir,
        RotateOptions {
            source,
            degrees: 90,
            scope: PageScope::Spec("1-99".to_string()),
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(AppError::Range(_))));
}
