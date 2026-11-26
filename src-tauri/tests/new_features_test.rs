//! 新功能集成测试：PDF 加水印、证件 A4 拼版、可视化页面整理、PDF 转长图、文档漂白。

mod common;

use pdfai_cn_lib::pdf::export_images::{self, ExportImagesOptions};
use pdfai_cn_lib::pdf::idcard::{self, IdCardOptions};
use pdfai_cn_lib::pdf::images_to_pdf::{self, ImagesToPdfOptions};
use pdfai_cn_lib::pdf::organize::{self, OrganizeOptions, OrganizePageSpec};
use pdfai_cn_lib::pdf::types::{ImageEnhanceOpt, ImageFormatOpt, PageScope, PageSizeOpt};
use pdfai_cn_lib::pdf::watermark::{self, WatermarkColor, WatermarkLayout, WatermarkOptions};
use pdfai_cn_lib::pdf::noop_progress;

use common::{image_size, make_pdf, page_count, page_geometry, test_ctx, write_png};

#[test]
fn watermark_stamps_all_selected_pages() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let src = make_pdf(dir, "contract", 3, PageSizeOpt::A4);

    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    let outputs = watermark::run(
        &ctx,
        dir,
        &WatermarkOptions {
            source: src,
            text: "仅供测试使用".to_string(),
            layout: WatermarkLayout::Tile,
            color: WatermarkColor::Gray,
            scope: PageScope::All,
            ..Default::default()
        },
        &mut progress,
    )
    .expect("加水印应当成功");

    assert_eq!(outputs.len(), 1);
    assert_eq!(page_count(&outputs[0].path), 3);
    assert!(ctx.summary().is_some());
}

#[test]
fn idcard_tiles_two_photos_onto_single_a4_with_watermark() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let front = write_png(dir, "front.png", 800, 500, [205, 205, 205, 255]);
    let back = write_png(dir, "back.png", 500, 800, [210, 210, 210, 255]); // 竖拍应自动摆正

    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    let outputs = idcard::run(
        &ctx,
        dir,
        &IdCardOptions {
            sources: vec![front, back],
            enhance: ImageEnhanceOpt::Bleach,
            watermark_text: Some("仅供办理业务使用".to_string()),
            watermark_opacity: 0.25,
            output_name: Some("idcard_out.pdf".to_string()),
        },
        &mut progress,
    )
    .expect("证件拼版应当成功");

    assert_eq!(outputs.len(), 1);
    assert_eq!(page_count(&outputs[0].path), 1);
    let (w, h, _) = page_geometry(&outputs[0].path)[0];
    assert!((w - 595.276).abs() < 1.5);
    assert!((h - 841.89).abs() < 1.5);
}

#[test]
fn organize_reorders_deletes_and_rotates_individual_pages() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let src = make_pdf(dir, "slides", 3, PageSizeOpt::A4);

    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    let outputs = organize::run(
        &ctx,
        dir,
        &OrganizeOptions {
            source: src,
            password: None,
            // 删除第 2 页（index=1），把第 3 页放前面，第 1 页旋转 90°
            pages: vec![
                OrganizePageSpec {
                    page_index: 2,
                    rotate_deg: 0,
                },
                OrganizePageSpec {
                    page_index: 0,
                    rotate_deg: 90,
                },
            ],
            output_name: Some("organized.pdf".to_string()),
        },
        &mut progress,
    )
    .expect("页面整理应当成功");

    assert_eq!(outputs.len(), 1);
    let geom = page_geometry(&outputs[0].path);
    assert_eq!(geom.len(), 2);
    assert_eq!(geom[0].2, 0);
    assert_eq!(geom[1].2, 90);
}

#[test]
fn export_images_stitches_pages_into_single_long_image() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let src = make_pdf(dir, "longdoc", 3, PageSizeOpt::A4);

    let ctx = test_ctx(&dir.join("cache"));
    let mut progress = noop_progress();
    let outputs = export_images::run(
        &ctx,
        dir,
        &ExportImagesOptions {
            source: src,
            dpi: 72,
            format: ImageFormatOpt::Png,
            stitch: true,
            ..Default::default()
        },
        &mut progress,
    )
    .expect("长图拼接应当成功");

    assert_eq!(outputs.len(), 1);
    let (w, h) = image_size(&outputs[0].path);
    assert!(w > 400);
    // 3 页 A4 @72dpi 高度 > 3 * 800 = 2400px
    assert!(h > 2400, "长图高度应包含 3 页总高，实际为 {h}");
}

#[test]
fn images_to_pdf_supports_document_bleach_and_bw_enhancement() {
    let dir = tempfile::tempdir().expect("临时目录");
    let dir = dir.path();
    let photo = write_png(dir, "scan.png", 200, 300, [200, 200, 200, 255]);

    for mode in [ImageEnhanceOpt::Bleach, ImageEnhanceOpt::Bw] {
        let ctx = test_ctx(&dir.join("cache"));
        let mut progress = noop_progress();
        let outputs = images_to_pdf::run(
            &ctx,
            dir,
            &ImagesToPdfOptions {
                sources: vec![photo.clone()],
                enhance: mode,
                ..Default::default()
            },
            &mut progress,
        )
        .expect("增强模式图片转 PDF 应当成功");
        assert_eq!(page_count(&outputs[0].path), 1);
    }
}
