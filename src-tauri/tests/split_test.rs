//! PDF 拆分的集成测试。

mod common;

use std::path::Path;

use pdf_zhizhuan_lib::error::AppError;
use pdf_zhizhuan_lib::pdf::noop_progress;
use pdf_zhizhuan_lib::pdf::split::{self, SplitMode, SplitOptions};
use pdf_zhizhuan_lib::pdf::types::{OutputFile, PageSizeOpt};

use common::{list_files, make_pdf, page_count, test_ctx};

fn run(dir: &Path, options: SplitOptions) -> Result<Vec<OutputFile>, AppError> {
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    split::run(&ctx, &dir.join("out"), &options, &mut progress)
}

fn fixture(dir: &Path, pages: usize) -> std::path::PathBuf {
    make_pdf(dir, "doc", pages, PageSizeOpt::A4)
}

#[test]
fn every_n_splits_into_expected_group_sizes() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = fixture(dir, 6);

    let outputs = run(
        dir,
        SplitOptions {
            source,
            mode: SplitMode::EveryN { size: 2 },
            ..Default::default()
        },
    )
    .expect("拆分应当成功");

    assert_eq!(outputs.len(), 3, "6 页每 2 页应产出 3 个文件");
    for output in &outputs {
        assert_eq!(page_count(&output.path), 2);
        assert_eq!(output.pages, Some(2));
    }

    let names = list_files(&dir.join("out"), "pdf");
    assert_eq!(
        names,
        vec![
            "doc_part001_p1-2.pdf",
            "doc_part002_p3-4.pdf",
            "doc_part003_p5-6.pdf"
        ],
        "命名应当带序号与页码区间"
    );
}

#[test]
fn every_n_keeps_indivisible_tail() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = fixture(dir, 5);

    let outputs = run(
        dir,
        SplitOptions {
            source,
            mode: SplitMode::EveryN { size: 2 },
            ..Default::default()
        },
    )
    .expect("拆分应当成功");

    let counts = outputs
        .iter()
        .map(|output| page_count(&output.path))
        .collect::<Vec<_>>();
    assert_eq!(counts, vec![2, 2, 1], "不足一组的尾页也应产出");
}

#[test]
fn each_page_produces_one_file_per_page() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = fixture(dir, 3);

    let outputs = run(
        dir,
        SplitOptions {
            source,
            mode: SplitMode::EachPage,
            ..Default::default()
        },
    )
    .expect("拆分应当成功");

    assert_eq!(outputs.len(), 3);
    assert!(outputs.iter().all(|output| page_count(&output.path) == 1));
}

#[test]
fn by_ranges_creates_one_file_per_token() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = fixture(dir, 8);

    let outputs = run(
        dir,
        SplitOptions {
            source,
            mode: SplitMode::ByRanges {
                spec: "1-3,4-6,7-8".to_string(),
            },
            ..Default::default()
        },
    )
    .expect("拆分应当成功");

    let counts = outputs
        .iter()
        .map(|output| page_count(&output.path))
        .collect::<Vec<_>>();
    assert_eq!(counts, vec![3, 3, 2]);
    assert_eq!(
        list_files(&dir.join("out"), "pdf"),
        vec![
            "doc_part001_p1-3.pdf",
            "doc_part002_p4-6.pdf",
            "doc_part003_p7-8.pdf"
        ]
    );
}

#[test]
fn extract_supports_reordered_and_sparse_pages() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = fixture(dir, 6);

    let outputs = run(
        dir,
        SplitOptions {
            source,
            mode: SplitMode::Extract {
                spec: "6,2,4".to_string(),
            },
            ..Default::default()
        },
    )
    .expect("提取应当成功");

    assert_eq!(outputs.len(), 1, "Extract 只产出一个文件");
    assert_eq!(page_count(&outputs[0].path), 3);
}

#[test]
fn zip_packs_all_parts_into_single_archive() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = fixture(dir, 4);

    let outputs = run(
        dir,
        SplitOptions {
            source,
            mode: SplitMode::EveryN { size: 2 },
            zip: true,
            output_name: Some("pack.zip".to_string()),
            ..Default::default()
        },
    )
    .expect("拆分并打包应当成功");

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].name, "pack.zip");
    assert!(matches!(
        outputs[0].kind,
        pdf_zhizhuan_lib::pdf::OutputKind::Archive
    ));

    let entries = zip_entries(&outputs[0].path);
    assert_eq!(
        entries,
        vec!["doc_part001_p1-2.pdf", "doc_part002_p3-4.pdf"],
        "压缩包内应当按页序包含全部拆分结果"
    );
    // 临时目录不应残留在缓存里
    assert!(list_files(&dir.join("cache"), "pdf").is_empty());
}

#[test]
fn rejects_out_of_range_spec() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = fixture(dir, 3);

    let result = run(
        dir,
        SplitOptions {
            source,
            mode: SplitMode::Extract {
                spec: "1-9".to_string(),
            },
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(AppError::Range(_))));
}

#[test]
fn reports_missing_source_as_io_error() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();

    let result = run(
        dir,
        SplitOptions {
            source: dir.join("does-not-exist.pdf"),
            mode: SplitMode::EachPage,
            ..Default::default()
        },
    );
    assert!(
        matches!(result, Err(AppError::Io(_)) | Err(AppError::Pdf(_))),
        "不存在的文件应报 Io/Pdf 错误，实际为 {result:?}"
    );
}

/// 读取 zip 内的条目名（已排序）。
fn zip_entries(path: &Path) -> Vec<String> {
    let file = std::fs::File::open(path).expect("zip 应当可读");
    let mut archive = zip::ZipArchive::new(file).expect("zip 应当可解析");
    let mut names = (0..archive.len())
        .map(|index| {
            let entry = archive.by_index(index).expect("条目应当可读");
            entry.name().expect("条目名应当合法").to_string()
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}
