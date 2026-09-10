use super::super::*;
use super::support::*;

#[test]
fn replaces_unique_png_picture_and_preserves_xml_parts() {
    let input = valid_picture_fixture();
    let output = path("picture-output");
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let replacement = b"\x89PNG\r\n\x1a\nreplacement".to_vec();
    let result = replace_picture(
        &package,
        &main,
        &source,
        &ReplacePicture {
            target: opensuite_protocol::PictureTarget {
                handle: None,
                name: None,
                description: Some("Company logo".to_owned()),
                occurrence: None,
            },
            replacement: opensuite_protocol::ImagePayload {
                content_type: "image/png".to_owned(),
                bytes: replacement.clone(),
            },
            base_revision: None,
        },
        &output,
    );
    assert_eq!(
        result.status,
        opensuite_protocol::OperationStatus::Applied,
        "{result:?}"
    );
    assert_eq!(entry(&output, "media/logo.png"), replacement);
    let mut input_payloads = payloads(&input);
    let mut output_payloads = payloads(&output);
    input_payloads.remove("media/logo.png");
    output_payloads.remove("media/logo.png");
    assert_eq!(input_payloads, output_payloads);
    let reopened = Package::open(&output).unwrap();
    let (main, source) = crate::open_main_source(&reopened).unwrap();
    let picture = crate::DocxDocument::new(&source)
        .unwrap()
        .pictures()
        .find(|picture| picture.metadata().and_then(|meta| meta.name) == Some("Logo".to_owned()))
        .unwrap();
    assert_eq!(
        picture.metadata().unwrap().description.as_deref(),
        Some("Company logo")
    );
    assert_eq!(
        reopened
            .read_part(&match picture.image_reference(&reopened, &main).unwrap() {
                crate::ImageReference::Embedded(image) => image.part,
                _ => panic!(),
            })
            .unwrap(),
        replacement
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
}

#[test]
fn rejects_picture_type_mismatch_invalid_and_ambiguous_targets() {
    let input = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/images.docx");
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let run = |name: Option<&str>, content_type: &str, bytes: Vec<u8>| {
        replace_picture(
            &package,
            &main,
            &source,
            &ReplacePicture {
                target: opensuite_protocol::PictureTarget {
                    handle: None,
                    name: name.map(str::to_owned),
                    description: None,
                    occurrence: None,
                },
                replacement: opensuite_protocol::ImagePayload {
                    content_type: content_type.to_owned(),
                    bytes,
                },
                base_revision: None,
            },
            path("picture-rejected"),
        )
    };
    assert_eq!(
        run(
            Some("Logo"),
            "image/jpeg",
            vec![0xff, 0xd8, 0xff, 0xff, 0xd9]
        )
        .diagnostics[0]
            .code,
        "UNSUPPORTED_OPERATION"
    );
    assert_eq!(
        run(Some("Logo"), "image/png", b"bad".to_vec()).diagnostics[0].code,
        "UNSUPPORTED_OPERATION"
    );
    assert_eq!(
        run(Some("Missing"), "image/png", b"\x89PNG\r\n\x1a\n".to_vec()).diagnostics[0].code,
        "TARGET_NOT_FOUND"
    );
}

#[test]
fn replaces_unique_jpeg_picture_by_name() {
    let input = valid_picture_fixture();
    let output = path("picture-jpeg-output");
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let replacement = vec![0xff, 0xd8, 0xff, 0, 0xff, 0xd9];
    let result = replace_picture(
        &package,
        &main,
        &source,
        &ReplacePicture {
            target: opensuite_protocol::PictureTarget {
                handle: None,
                name: Some("Photo".to_owned()),
                description: None,
                occurrence: None,
            },
            replacement: opensuite_protocol::ImagePayload {
                content_type: "image/jpeg".to_owned(),
                bytes: replacement.clone(),
            },
            base_revision: None,
        },
        &output,
    );
    assert_eq!(
        result.status,
        opensuite_protocol::OperationStatus::Applied,
        "{result:?}"
    );
    assert_eq!(entry(&output, "media/photo.jpeg"), replacement);
    assert_eq!(
        entry(&input, "media/logo.png"),
        entry(&output, "media/logo.png")
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
}

#[test]
fn requires_occurrence_for_duplicate_pictures_and_rejects_shared_media() {
    let input = picture_fixture(&[
        ("Duplicate", "one", "../media/one.png"),
        ("Duplicate", "two", "../media/two.png"),
    ]);
    let output = path("picture-duplicate-output");
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let operation = |occurrence| ReplacePicture {
        target: opensuite_protocol::PictureTarget {
            handle: None,
            name: Some("Duplicate".to_owned()),
            description: None,
            occurrence,
        },
        replacement: opensuite_protocol::ImagePayload {
            content_type: "image/png".to_owned(),
            bytes: b"\x89PNG\r\n\x1a\nreplacement".to_vec(),
        },
        base_revision: None,
    };
    assert_eq!(
        replace_picture(&package, &main, &source, &operation(None), &output).diagnostics[0].code,
        "TARGET_AMBIGUOUS"
    );
    let result = replace_picture(&package, &main, &source, &operation(Some(1)), &output);
    assert_eq!(
        result.status,
        opensuite_protocol::OperationStatus::Applied,
        "{result:?}"
    );
    assert_eq!(
        entry(&input, "media/one.png"),
        entry(&output, "media/one.png")
    );
    assert_eq!(
        entry(&output, "media/two.png"),
        b"\x89PNG\r\n\x1a\nreplacement"
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();

    let shared = picture_fixture(&[
        ("First", "one", "../media/shared.png"),
        ("Second", "two", "../media/shared.png"),
    ]);
    let package = Package::open(&shared).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let shared_operation = ReplacePicture {
        target: opensuite_protocol::PictureTarget {
            handle: None,
            name: Some("First".to_owned()),
            description: None,
            occurrence: None,
        },
        ..operation(None)
    };
    assert_eq!(
        replace_picture(
            &package,
            &main,
            &source,
            &shared_operation,
            path("shared-output")
        )
        .diagnostics[0]
            .code,
        "UNSUPPORTED_OPERATION"
    );
    fs::remove_file(shared).unwrap();
}

#[test]
fn inserts_inline_picture_with_bytes_dimensions_and_collision_safe_ids() {
    let input = crate::create_blank_docx();
    let package = Package::from_bytes(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let first = insert_picture_to_vec(
        &package,
        &main,
        &source,
        &InsertPicture {
            image_bytes: png(640, 480),
            placement: ParagraphPlacement::Start,
            alt_text: Some("chart".to_owned()),
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(first).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let second = insert_picture_to_vec(
        &package,
        &main,
        &source,
        &InsertPicture {
            image_bytes: png(1600, 800),
            placement: ParagraphPlacement::End,
            alt_text: None,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(second).unwrap();
    package.verify().unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let document = crate::DocxDocument::new(&source).unwrap();
    let pictures = document.pictures().collect::<Vec<_>>();
    assert_eq!(pictures.len(), 2);
    assert_eq!(pictures[0].metadata().unwrap().id.as_deref(), Some("1"));
    assert_eq!(pictures[1].metadata().unwrap().id.as_deref(), Some("2"));
    assert_eq!(
        pictures[1].extent().unwrap().unwrap().width_emu,
        MAX_INLINE_PICTURE_WIDTH_EMU
    );
    let crate::ImageReference::Embedded(image) =
        pictures[1].image_reference(&package, &main).unwrap()
    else {
        panic!("embedded image expected")
    };
    assert_eq!(package.read_part(&image.part).unwrap(), png(1600, 800));
    assert_eq!(
        package.content_type_default("png").unwrap().as_str(),
        PNG_CONTENT_TYPE
    );
}

#[test]
fn rejects_malformed_and_unsupported_picture_bytes_without_output() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    for image_bytes in [b"not an image".to_vec(), b"\x89PNG\r\n\x1a\n".to_vec()] {
        assert!(
            insert_picture_to_vec(
                &package,
                &main,
                &source,
                &InsertPicture {
                    image_bytes,
                    placement: ParagraphPlacement::End,
                    alt_text: None,
                    base_revision: None,
                }
            )
            .is_err()
        );
    }
}

#[test]
fn inserts_pictures_before_and_after_body_handles() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let first = insert_paragraph_to_vec(
        &package,
        &main,
        &source,
        &InsertParagraph {
            text: "First".to_owned(),
            placement: ParagraphPlacement::Start,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(first).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let body = insert_paragraph_to_vec(
        &package,
        &main,
        &source,
        &InsertParagraph {
            text: "Second".to_owned(),
            placement: ParagraphPlacement::End,
            base_revision: None,
        },
    )
    .unwrap();
    for placement in [
        ParagraphPlacement::Before {
            handle: "b1".to_owned(),
        },
        ParagraphPlacement::After {
            handle: "b0".to_owned(),
        },
    ] {
        let package = Package::from_bytes(body.clone()).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        assert!(
            insert_picture_to_vec(
                &package,
                &main,
                &source,
                &InsertPicture {
                    image_bytes: png(1, 1),
                    placement,
                    alt_text: None,
                    base_revision: None,
                },
            )
            .is_ok()
        );
    }
}

#[test]
fn deletes_a_supported_picture_and_preserves_its_media_part() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let inserted = insert_picture_to_vec(
        &package,
        &main,
        &source,
        &InsertPicture {
            image_bytes: png(2, 1),
            placement: ParagraphPlacement::Start,
            alt_text: None,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(inserted).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let output = delete_picture_to_vec(
        &package,
        &main,
        &source,
        &DeletePicture {
            target: opensuite_protocol::PictureTarget {
                handle: Some("p0".to_owned()),
                name: None,
                description: None,
                occurrence: None,
            },
            base_revision: None,
        },
    )
    .unwrap();
    let reopened = Package::from_bytes(output).unwrap();
    let (main, source) = crate::open_main_source(&reopened).unwrap();
    assert_eq!(
        crate::DocxDocument::new(&source)
            .unwrap()
            .pictures()
            .count(),
        0
    );
    assert_eq!(
        reopened
            .read_part(
                &reopened
                    .part(&PartName::parse("/word/media/image1.png").unwrap())
                    .unwrap()
            )
            .unwrap(),
        png(2, 1)
    );
    assert!(
        delete_picture_to_vec(
            &reopened,
            &main,
            &source,
            &DeletePicture {
                target: opensuite_protocol::PictureTarget {
                    handle: Some("p0".to_owned()),
                    name: None,
                    description: None,
                    occurrence: None
                },
                base_revision: None,
            }
        )
        .is_err()
    );
}

#[test]
fn resizes_one_picture_and_preserves_the_other_picture() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let first = insert_picture_to_vec(
        &package,
        &main,
        &source,
        &InsertPicture {
            image_bytes: png(4, 2),
            placement: ParagraphPlacement::Start,
            alt_text: Some("png alt".to_owned()),
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(first).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let input = insert_picture_to_vec(
        &package,
        &main,
        &source,
        &InsertPicture {
            image_bytes: jpeg(4, 2),
            placement: ParagraphPlacement::End,
            alt_text: Some("jpeg alt".to_owned()),
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let output = set_picture_size_to_vec(
        &package,
        &main,
        &source,
        &SetPictureSize {
            target: opensuite_protocol::PictureTarget {
                handle: Some("p1".to_owned()),
                name: None,
                description: None,
                occurrence: None,
            },
            size: PictureSizeChange::HeightEmu(9_525),
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(output).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let output = set_picture_size_to_vec(
        &package,
        &main,
        &source,
        &SetPictureSize {
            target: opensuite_protocol::PictureTarget {
                handle: Some("p0".to_owned()),
                name: None,
                description: None,
                occurrence: None,
            },
            size: PictureSizeChange::WidthEmu(19_050),
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(output).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let pictures = crate::DocxDocument::new(&source)
        .unwrap()
        .pictures()
        .collect::<Vec<_>>();
    assert_eq!(
        pictures[0].extent().unwrap().unwrap(),
        crate::PictureExtent {
            width_emu: 19_050,
            height_emu: 9_525
        }
    );
    assert_eq!(
        pictures[1].extent().unwrap().unwrap(),
        crate::PictureExtent {
            width_emu: 19_050,
            height_emu: 9_525
        }
    );
    assert_eq!(
        pictures[1].metadata().unwrap().description.as_deref(),
        Some("jpeg alt")
    );
    assert_eq!(
        package
            .read_part(
                &package
                    .part(&PartName::parse("/word/media/image2.jpeg").unwrap())
                    .unwrap()
            )
            .unwrap(),
        jpeg(4, 2)
    );
    assert!(
        set_picture_size_to_vec(
            &package,
            &main,
            &source,
            &SetPictureSize {
                target: opensuite_protocol::PictureTarget {
                    handle: Some("p1".to_owned()),
                    name: None,
                    description: None,
                    occurrence: None
                },
                size: PictureSizeChange::WidthEmu(0),
                base_revision: None,
            }
        )
        .is_err()
    );
    assert!(
        set_picture_size_to_vec(
            &package,
            &main,
            &source,
            &SetPictureSize {
                target: opensuite_protocol::PictureTarget {
                    handle: Some("p9".to_owned()),
                    name: None,
                    description: None,
                    occurrence: None
                },
                size: PictureSizeChange::WidthEmu(1),
                base_revision: None,
            }
        )
        .is_err()
    );
    let xml = String::from_utf8(package.read_part(&main).unwrap()).unwrap();
    assert!(xml.contains("<wp:extent cx=\"19050\" cy=\"9525\""));
    assert!(xml.contains("<a:ext cx=\"19050\" cy=\"9525\""));
}
