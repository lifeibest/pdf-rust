//! PDF 提取图片的集成测试。
//!
//! 两类夹具：
//! - `make_pdf`（common）：PNG 经 SetBitmap 嵌入，滤镜为 FlateDecode，走解码导出分支；
//! - `make_jpeg_pdf`（本文件）：JPEG 经 LoadJpegFile 嵌入，滤镜为 DCTDecode，
//!   用于验证无损直写与跨页去重。

mod common;

use std::path::{Path, PathBuf};

use image::{ImageBuffer, Rgb};
use pdfium_render::prelude::{PdfPageImageObject, PdfPageObjectsCommon, PdfPagePaperSize};

use pdfai_cn_lib::error::AppError;
use pdfai_cn_lib::pdf::engine;
use pdfai_cn_lib::pdf::extract_images::{self, ExtractFormatOpt, ExtractImagesOptions};
use pdfai_cn_lib::pdf::noop_progress;
use pdfai_cn_lib::pdf::types::{OutputFile, PageScope, PageSizeOpt};

use common::{image_size, list_files, make_pdf, test_ctx, write_jpeg};

fn run(dir: &Path, options: ExtractImagesOptions) -> Result<Vec<OutputFile>, AppError> {
    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    extract_images::run(&ctx, &dir.join("out"), &options, &mut progress)
}

/// 造一个每页嵌一张 JPEG 的 PDF（DCTDecode 滤镜），页序与传入路径一致。
fn make_jpeg_pdf(dir: &Path, name: &str, jpegs: &[PathBuf]) -> PathBuf {
    common::init_engine();
    let pdfium = engine::get().expect("引擎应当已初始化");
    let mut document = pdfium.create_new_pdf().expect("应当能创建文档");

    for jpeg in jpegs {
        let mut page = document
            .pages_mut()
            .create_page_at_end(PdfPagePaperSize::a4())
            .expect("应当能建页");
        let mut object =
            PdfPageImageObject::new_from_jpeg_file(&document, jpeg).expect("应当能嵌入 JPEG");
        object.scale(200.0, 200.0).expect("缩放应当成功");
        page.objects_mut()
            .add_image_object(object)
            .expect("应当能加入页面对象");
    }

    let path = dir.join(format!("{name}.pdf"));
    document.save_to_file(&path).expect("应当能保存");
    path
}

/// 生成一张伪随机噪点 JPEG（体积显著大于纯色图），用于最小体积过滤测试。
fn write_noise_jpeg(dir: &Path, name: &str, edge: u32) -> PathBuf {
    std::fs::create_dir_all(dir).expect("输入目录应当可创建");
    let path = dir.join(name);
    let mut seed = 0x1234_5678u32;
    let buffer = ImageBuffer::from_fn(edge, edge, |_, _| {
        // LCG 伪随机，保证噪点不可压缩且结果可复现
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        Rgb([(seed >> 16) as u8, (seed >> 8) as u8, seed as u8])
    });
    buffer.save(&path).expect("JPEG 应当写入成功");
    path
}

#[test]
fn extracts_embedded_images_at_native_resolution() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    // make_pdf 每页嵌一张 320x440 的 PNG
    let source = make_pdf(dir, "doc", 3, PageSizeOpt::A4);

    let outputs = run(
        dir,
        ExtractImagesOptions {
            source,
            ..Default::default()
        },
    )
    .expect("提取应当成功");

    assert_eq!(outputs.len(), 3, "三页各嵌一张图");
    assert_eq!(
        list_files(&dir.join("out"), "png"),
        vec![
            "doc_p001_i01_pdfai.cn.png",
            "doc_p002_i01_pdfai.cn.png",
            "doc_p003_i01_pdfai.cn.png"
        ]
    );
    for output in &outputs {
        // 提取的是原始嵌入图，而不是按页面 DPI 栅格化的结果
        assert_eq!(image_size(&output.path), (320, 440));
    }
}

#[test]
fn jpeg_passthrough_writes_original_bytes() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let jpeg = write_jpeg(dir, "photo.jpg", 64, 48, [200, 60, 30]);
    let source = make_jpeg_pdf(dir, "doc", &[jpeg.clone()]);

    let outputs = run(
        dir,
        ExtractImagesOptions {
            source,
            ..Default::default()
        },
    )
    .expect("提取应当成功");

    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].name.ends_with(".jpg"), "Auto 下 JPEG 应直写为 jpg");

    let original = std::fs::read(&jpeg).expect("源 JPEG 应当可读");
    let extracted = std::fs::read(&outputs[0].path).expect("产出应当可读");
    assert_eq!(extracted, original, "无损直写应与源 JPEG 字节完全一致");
}

#[test]
fn dedupes_same_image_referenced_by_multiple_pages() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let jpeg = write_jpeg(dir, "logo.jpg", 32, 32, [10, 120, 200]);
    // 同一张图出现在三页上
    let source = make_jpeg_pdf(dir, "doc", &[jpeg.clone(), jpeg.clone(), jpeg]);

    let outputs = run(
        dir,
        ExtractImagesOptions {
            source,
            ..Default::default()
        },
    )
    .expect("提取应当成功");

    assert_eq!(outputs.len(), 1, "重复引用只应导出一次");
    assert!(outputs[0].name.contains("_p001_"), "应记首次出现的页码");
}

#[test]
fn min_size_filters_out_small_images() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let small = write_jpeg(dir, "icon.jpg", 8, 8, [90, 90, 90]);
    let big = write_noise_jpeg(dir, "photo.jpg", 600);
    let source = make_jpeg_pdf(dir, "doc", &[small, big]);

    let outputs = run(
        dir,
        ExtractImagesOptions {
            source,
            min_size_kb: 1,
            ..Default::default()
        },
    )
    .expect("提取应当成功");

    assert_eq!(outputs.len(), 1, "小于 1KB 的小图应被过滤");
    assert_eq!(image_size(&outputs[0].path), (600, 600));
}

#[test]
fn scope_limits_extraction_to_selected_pages() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let jpegs = (0..3)
        .map(|index| {
            write_jpeg(
                dir,
                &format!("p{index}.jpg"),
                40,
                40,
                [40 + index as u8 * 60, 80, 140],
            )
        })
        .collect::<Vec<_>>();
    let source = make_jpeg_pdf(dir, "doc", &jpegs);

    let outputs = run(
        dir,
        ExtractImagesOptions {
            source,
            scope: PageScope::Spec("2".to_string()),
            ..Default::default()
        },
    )
    .expect("提取应当成功");

    assert_eq!(outputs.len(), 1);
    assert!(outputs[0].name.contains("_p002_"));
}

#[test]
fn explicit_format_reencodes_embedded_images() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    // PNG 嵌入（FlateDecode），显式要求 JPEG 输出
    let source = make_pdf(dir, "doc", 2, PageSizeOpt::A4);

    let outputs = run(
        dir,
        ExtractImagesOptions {
            source,
            format: ExtractFormatOpt::Jpeg,
            quality: 80,
            ..Default::default()
        },
    )
    .expect("提取应当成功");

    assert_eq!(outputs.len(), 2);
    assert_eq!(list_files(&dir.join("out"), "jpg").len(), 2);
    assert_eq!(image_size(&outputs[0].path), (320, 440));
}

#[test]
fn zip_packs_all_images_into_single_archive() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 3, PageSizeOpt::A4);

    let outputs = run(
        dir,
        ExtractImagesOptions {
            source,
            zip: true,
            output_name: Some("images.zip".to_string()),
            ..Default::default()
        },
    )
    .expect("提取并打包应当成功");

    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].name, "images.zip");
    assert!(list_files(&dir.join("out"), "png").is_empty());

    let file = std::fs::File::open(&outputs[0].path).expect("zip 应当可读");
    let archive = zip::ZipArchive::new(file).expect("zip 应当可解析");
    assert_eq!(archive.len(), 3, "压缩包内应有三张图片");
}

#[test]
fn rejects_document_without_images() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    common::init_engine();
    let pdfium = engine::get().expect("引擎应当已初始化");
    let mut document = pdfium.create_new_pdf().expect("应当能创建文档");
    document
        .pages_mut()
        .create_page_at_end(PdfPagePaperSize::a4())
        .expect("应当能建页");
    let source = dir.join("blank.pdf");
    document.save_to_file(&source).expect("应当能保存");

    let result = run(
        dir,
        ExtractImagesOptions {
            source,
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(AppError::Pdf(_))));
}

#[test]
fn rejects_out_of_range_scope() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let source = make_pdf(dir, "doc", 2, PageSizeOpt::A4);

    let result = run(
        dir,
        ExtractImagesOptions {
            source,
            scope: PageScope::Spec("9".to_string()),
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(AppError::Range(_))));
}