use super::*;
use opensuite_protocol::{ImagePayload, ImageTextDistance, ParagraphPlacement, PictureTarget};

fn target(handle: String) -> PictureTarget {
    PictureTarget {
        handle: Some(handle),
        name: None,
        description: None,
        occurrence: None,
    }
}
fn layout() -> PictureLayoutPatch {
    PictureLayoutPatch {
        horizontal: Some(ImageAxisPosition {
            reference: ImagePositionReference::Margin,
            position: ImagePosition::Align(ImageAlignment::End),
        }),
        vertical: Some(ImageAxisPosition {
            reference: ImagePositionReference::Paragraph,
            position: ImagePosition::OffsetEmu(0),
        }),
        wrap: Some(ImageWrap::Square),
        distance: ImageTextDistance {
            left_emu: Some(114_300),
            ..Default::default()
        },
    }
}
fn seed(floating: bool) -> Vec<u8> {
    // Metadata-only PNG is enough for structural Rust tests; Node dogfood uses a decoded PNG.
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend_from_slice(&2u32.to_be_bytes());
    png.extend_from_slice(&1u32.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    let result = crate::execute_docx_insert_picture(
        crate::create_blank_docx(),
        &InsertPicture {
            image_bytes: png,
            placement: ParagraphPlacement::End,
            size: Some(PictureSizeChange::WidthEmu(914_400)),
            layout: floating.then(layout),
            alt_text: Some("Preserve me".into()),
            base_revision: None,
        },
    );
    assert!(
        result.operation.status == opensuite_protocol::OperationStatus::Applied,
        "{:?}",
        result.operation
    );
    result.output_artifact.unwrap()
}
fn handle(bytes: &[u8]) -> String {
    crate::inspect_docx_layout(bytes.to_vec(), &Default::default()).images[0]
        .handle
        .clone()
        .unwrap()
}
fn xml(bytes: &[u8]) -> Vec<u8> {
    let package = Package::from_bytes(bytes.to_vec()).unwrap();
    package
        .read_part(&package.main_office_document().unwrap())
        .unwrap()
}
fn imported(bytes: &[u8], change: impl FnOnce(String) -> String) -> Vec<u8> {
    let package = Package::from_bytes(bytes.to_vec()).unwrap();
    let main = package.main_office_document().unwrap();
    package
        .write_replaced_part_to_vec(
            &main,
            change(String::from_utf8(xml(bytes)).unwrap()).as_bytes(),
        )
        .unwrap()
}
#[test]
fn inline_dimensions_share_one_ratio_rule_and_reject_invalid_input() {
    let bytes = seed(false);
    let snapshot = crate::inspect_docx_layout(bytes.clone(), &Default::default());
    assert_eq!(snapshot.images[0].display_width_emu, Some(914_400));
    assert_eq!(snapshot.images[0].display_height_emu, Some(457_200));
    assert_eq!(snapshot.images[0].aspect_ratio, Some(2.0));
    assert_eq!(
        sized_extent((2, 1), Some(PictureSizeChange::HeightEmu(100))).unwrap(),
        (200, 100)
    );
    assert_eq!(
        sized_extent(
            (2, 1),
            Some(PictureSizeChange::ExactEmu {
                width: 3,
                height: 9
            })
        )
        .unwrap(),
        (3, 9)
    );
    assert!(sized_extent((2, 1), Some(PictureSizeChange::WidthEmu(0))).is_err());
    assert!(sized_extent((2, 1), Some(PictureSizeChange::HeightEmu(i64::MAX))).is_err());
}
#[test]
fn floating_wrap_position_and_distance_round_trip_and_stale_handles_fail() {
    let mut bytes = seed(true);
    let stale = handle(&bytes);
    for wrap in [
        ImageWrap::TopAndBottom,
        ImageWrap::BehindText,
        ImageWrap::InFrontOfText,
        ImageWrap::Square,
    ] {
        let result = crate::execute_docx_set_picture_layout(
            bytes.clone(),
            &SetPictureLayout {
                target: target(handle(&bytes)),
                layout: PictureLayoutPatch {
                    wrap: Some(wrap),
                    distance: ImageTextDistance {
                        right_emu: Some(100),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                base_revision: None,
            },
        );
        assert!(
            result.operation.status == opensuite_protocol::OperationStatus::Applied,
            "{:?}",
            result.operation
        );
        bytes = result.output_artifact.unwrap();
        let facts = crate::inspect_docx_layout(bytes.clone(), &Default::default());
        assert!(facts.images[0].anchor.as_ref().unwrap().editable);
        assert_eq!(
            facts.images[0].anchor.as_ref().unwrap().distance_emu["left"],
            114_300
        );
    }
    let result = crate::execute_docx_set_picture_layout(
        bytes.clone(),
        &SetPictureLayout {
            target: target(stale),
            layout: PictureLayoutPatch {
                horizontal: Some(ImageAxisPosition {
                    reference: ImagePositionReference::Page,
                    position: ImagePosition::OffsetEmu(20_000_000),
                }),
                ..Default::default()
            },
            base_revision: None,
        },
    );
    assert!(result.output_artifact.is_none()); // stale after XML changes, even if wrap returns to square
    let result = crate::execute_docx_set_picture_layout(
        bytes.clone(),
        &SetPictureLayout {
            target: target(handle(&bytes)),
            layout: PictureLayoutPatch {
                horizontal: Some(ImageAxisPosition {
                    reference: ImagePositionReference::Page,
                    position: ImagePosition::OffsetEmu(20_000_000),
                }),
                ..Default::default()
            },
            base_revision: None,
        },
    );
    assert!(
        result.operation.status == opensuite_protocol::OperationStatus::Applied,
        "{:?}",
        result.operation
    );
    assert!(
        crate::inspect_docx_layout(result.output_artifact.unwrap(), &Default::default())
            .diagnostics
            .iter()
            .any(|d| d.code == "IMAGE_POSITION_OUTSIDE_PAGE")
    );
}
#[test]
fn imported_resize_position_and_replacement_preserve_source_and_other_parts() {
    let bytes = imported(&seed(true), |s| {
        s.replace("locked=\"0\"", "locked='1' custom='keep'")
            .replace(
                "<pic:spPr>",
                "<pic:spPr><a:extLst><a:ext uri='preserve'/></a:extLst>",
            )
            .replace("<a:stretch>", "<a:srcRect l='100'/><a:stretch>")
            .replace("relativeFrom=\"margin\"", "relativeFrom='margin'")
    });
    let before = String::from_utf8(xml(&bytes)).unwrap();
    let resized = crate::execute_docx_set_picture_size(
        bytes.clone(),
        &SetPictureSize {
            target: target(handle(&bytes)),
            size: PictureSizeChange::WidthEmu(1_828_800),
            base_revision: None,
        },
    );
    assert!(
        resized.operation.status == opensuite_protocol::OperationStatus::Applied,
        "{:?}",
        resized.operation
    );
    let resized = resized.output_artifact.unwrap();
    assert_eq!(
        String::from_utf8(xml(&resized)).unwrap(),
        before
            .replace("cx=\"914400\"", "cx=\"1828800\"")
            .replace("cy=\"457200\"", "cy=\"914400\"")
    );
    let moved = crate::execute_docx_set_picture_layout(
        bytes.clone(),
        &SetPictureLayout {
            target: target(handle(&bytes)),
            layout: PictureLayoutPatch {
                horizontal: Some(ImageAxisPosition {
                    reference: ImagePositionReference::Page,
                    position: ImagePosition::Align(ImageAlignment::Center),
                }),
                ..Default::default()
            },
            base_revision: None,
        },
    );
    assert!(
        moved.operation.status == opensuite_protocol::OperationStatus::Applied,
        "{:?}",
        moved.operation
    );
    let moved = moved.output_artifact.unwrap();
    assert_eq!(
        String::from_utf8(xml(&moved)).unwrap(),
        before
            .replace("relativeFrom='margin'", "relativeFrom='page'")
            .replace("<wp:align>right</wp:align>", "<wp:align>center</wp:align>")
    );
    let package = Package::from_bytes(moved.clone()).unwrap();
    let replacement = crate::execute_docx_replace_picture(
        moved.clone(),
        &ReplacePicture {
            target: target(handle(&moved)),
            replacement: ImagePayload {
                content_type: "image/png".into(),
                bytes: b"\x89PNG\r\n\x1a\nreplacement".to_vec(),
            },
            base_revision: None,
        },
    );
    assert!(
        replacement.operation.status == opensuite_protocol::OperationStatus::Applied,
        "{:?}",
        replacement.operation
    );
    let replacement = replacement.output_artifact.unwrap();
    assert_eq!(xml(&replacement), xml(&moved));
    let reopened = Package::from_bytes(replacement).unwrap();
    for name in package.part_names_with_prefix("/") {
        if name.as_str() != "/word/media/image1.png" {
            assert_eq!(
                package.read_part_by_name(name).unwrap(),
                reopened.read_part_by_name(name).unwrap(),
                "{name:?}"
            );
        }
    }
}
#[test]
fn unsupported_imports_and_conversions_fail_without_output() {
    for bytes in [
        imported(&seed(true), |s| {
            s.replace("<wp:wrapSquare", "<wp:wrapTight")
        }),
        imported(&seed(true), |s| {
            s.replace("<pic:nvPicPr>", "<pic:pic/><pic:nvPicPr>")
        }),
        seed(false),
    ] {
        let original = bytes.clone();
        let result = crate::execute_docx_set_picture_layout(
            bytes.clone(),
            &SetPictureLayout {
                target: target(handle(&bytes)),
                layout: PictureLayoutPatch {
                    wrap: Some(ImageWrap::Square),
                    ..Default::default()
                },
                base_revision: None,
            },
        );
        assert!(result.output_artifact.is_none());
        assert_eq!(bytes, original);
    }
    let bytes = seed(true);
    let snapshot = crate::inspect_docx_layout(
        imported(&bytes, |s| s.replace("<wp:wrapSquare", "<wp:wrapThrough")),
        &Default::default(),
    );
    assert_eq!(
        snapshot.images[0].anchor.as_ref().unwrap().wrap.as_deref(),
        Some("through")
    );
    let result = crate::execute_docx_set_picture_size(
        bytes,
        &SetPictureSize {
            target: target("d99:invalid".into()),
            size: PictureSizeChange::WidthEmu(1),
            base_revision: None,
        },
    );
    assert!(result.output_artifact.is_none());
}

#[test]
fn libreoffice_import_resize_changes_only_dimension_values() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/libreoffice-floating-image.docx"
    ));
    let before = String::from_utf8(xml(bytes)).unwrap();
    let result = crate::execute_docx_set_picture_size(
        bytes.to_vec(),
        &SetPictureSize {
            target: target(handle(bytes)),
            size: PictureSizeChange::WidthEmu(2_743_200),
            base_revision: None,
        },
    );
    assert_eq!(
        result.operation.status,
        opensuite_protocol::OperationStatus::Applied,
        "{:?}",
        result.operation
    );
    let output = result.output_artifact.unwrap();
    assert_eq!(
        String::from_utf8(xml(&output)).unwrap(),
        before
            .replace("cx=\"1828800\"", "cx=\"2743200\"")
            .replace("cy=\"914400\"", "cy=\"1371600\"")
    );
    let before_package = Package::from_bytes(bytes.to_vec()).unwrap();
    let after_package = Package::from_bytes(output).unwrap();
    for name in before_package.part_names_with_prefix("/") {
        if name.as_str() != "/word/document.xml" {
            assert_eq!(
                before_package.read_part_by_name(name).unwrap(),
                after_package.read_part_by_name(name).unwrap(),
                "{name:?}"
            );
        }
    }
}

#[test]
fn replacement_rejects_media_shared_with_headers_or_legacy_images() {
    let bytes = seed(true);
    let legacy = imported(&bytes, |s| {
        s.replace("</w:body>", r#"<w:p><w:r><w:pict><v:imagedata xmlns:v="urn:schemas-microsoft-com:vml" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="rId3"/></w:pict></w:r></w:p></w:body>"#)
    });
    let header = crate::execute_docx_set_header_footer_text(
        bytes,
        &opensuite_protocol::SetHeaderFooterText {
            kind: opensuite_protocol::HeaderFooterKind::Header,
            text: Some("Shared header".into()),
            base_revision: None,
        },
    )
    .output_artifact
    .unwrap();
    let package = Package::from_bytes(header).unwrap();
    let header_name = package
        .part_names_with_prefix("/word/")
        .find(|p| p.as_str().contains("header") && p.as_str().ends_with(".xml"))
        .unwrap();
    let file_name = header_name.as_str().rsplit('/').next().unwrap();
    let relationships = PartName::parse(format!("/word/_rels/{file_name}.rels")).unwrap();
    let relation = br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="sharedImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/></Relationships>"#;
    let shared = package
        .write_package_with_named_changes_to_vec(&[], &[(relationships, relation)])
        .unwrap();
    for bytes in [legacy, shared] {
        let result = crate::execute_docx_replace_picture(
            bytes.clone(),
            &ReplacePicture {
                target: target(handle(&bytes)),
                replacement: ImagePayload {
                    content_type: "image/png".into(),
                    bytes: b"\x89PNG\r\n\x1a\nreplacement".to_vec(),
                },
                base_revision: None,
            },
        );
        assert!(result.output_artifact.is_none());
        assert!(result.operation.diagnostics[0].message.contains("shared"));
    }
}

#[test]
fn imported_wrap_distance_overrides_are_inspected_and_patched_without_normalizing() {
    let bytes = imported(&seed(true), |s| {
        s.replace("wrapText=\"bothSides\"", "wrapText='bothSides' distL='200'")
    });
    let snapshot = crate::inspect_docx_layout(bytes.clone(), &Default::default());
    assert_eq!(
        snapshot.images[0].anchor.as_ref().unwrap().distance_emu["left"],
        200
    );
    let result = crate::execute_docx_set_picture_layout(
        bytes.clone(),
        &SetPictureLayout {
            target: target(handle(&bytes)),
            layout: PictureLayoutPatch {
                distance: ImageTextDistance {
                    left_emu: Some(300),
                    ..Default::default()
                },
                ..Default::default()
            },
            base_revision: None,
        },
    );
    assert_eq!(
        result.operation.status,
        opensuite_protocol::OperationStatus::Applied,
        "{:?}",
        result.operation
    );
    let output = result.output_artifact.unwrap();
    assert_eq!(
        String::from_utf8(xml(&output)).unwrap(),
        String::from_utf8(xml(&bytes))
            .unwrap()
            .replace("distL=\"114300\"", "distL=\"300\"")
            .replace("distL='200'", "distL='300'")
    );
    let result = crate::execute_docx_set_picture_layout(
        bytes.clone(),
        &SetPictureLayout {
            target: target(handle(&bytes)),
            layout: PictureLayoutPatch {
                wrap: Some(ImageWrap::TopAndBottom),
                ..Default::default()
            },
            base_revision: None,
        },
    );
    assert!(result.output_artifact.is_none()); // preserve imported wrap-level distance rather than drop it
}

#[test]
fn resize_does_not_target_extension_transforms() {
    let bytes = imported(&seed(true), |s| {
        s.replace("<a:xfrm>", "<extra:xfrm xmlns:extra='urn:unknown'>")
            .replace("</a:xfrm>", "</extra:xfrm>")
    });
    let result = crate::execute_docx_set_picture_size(
        bytes.clone(),
        &SetPictureSize {
            target: target(handle(&bytes)),
            size: PictureSizeChange::WidthEmu(1828800),
            base_revision: None,
        },
    );
    assert!(result.output_artifact.is_none());
}
