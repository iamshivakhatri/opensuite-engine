use super::*;
use opensuite_protocol::{ParagraphAlignment, PictureSizeChange, PictureTarget};

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}
fn operation(width: u32, height: u32) -> InsertPicture {
    InsertPicture {
        auto_fit: true,
        alignment: None,
        size: None,
        layout: None,
        image_bytes: png(width, height),
        placement: ParagraphPlacement::End.into(),
        alt_text: None,
        base_revision: None,
    }
}
fn xml(bytes: &[u8]) -> String {
    let package = Package::from_bytes(bytes.to_vec()).unwrap();
    String::from_utf8(
        package
            .read_part(&package.main_office_document().unwrap())
            .unwrap(),
    )
    .unwrap()
}
fn change(bytes: &[u8], edit: impl FnOnce(String) -> String) -> Vec<u8> {
    let package = Package::from_bytes(bytes.to_vec()).unwrap();
    package
        .write_replaced_part_to_vec(
            &package.main_office_document().unwrap(),
            edit(xml(bytes)).as_bytes(),
        )
        .unwrap()
}
fn insert(bytes: Vec<u8>, operation: &InsertPicture) -> Vec<u8> {
    let result = crate::execute_docx_insert_picture(bytes, operation);
    assert_eq!(
        result.operation.status,
        opensuite_protocol::OperationStatus::Applied,
        "{:?}",
        result.operation
    );
    result.output_artifact.unwrap()
}
fn extent(bytes: &[u8]) -> (i64, i64) {
    let package = Package::from_bytes(bytes.to_vec()).unwrap();
    package.verify().unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    let extent = crate::DocxDocument::new(&source)
        .unwrap()
        .pictures()
        .last()
        .unwrap()
        .extent()
        .unwrap()
        .unwrap();
    (extent.width_emu, extent.height_emu)
}
fn assert_parts_preserved(before: &[u8], after: &[u8], skip: &[&str]) {
    let before = Package::from_bytes(before.to_vec()).unwrap();
    let after = Package::from_bytes(after.to_vec()).unwrap();
    for name in before.part_names_with_prefix("/") {
        if !skip.contains(&name.as_str()) {
            assert_eq!(
                before.read_part_by_name(name).unwrap(),
                after.read_part_by_name(name).unwrap(),
                "{name:?}"
            );
        }
    }
}

#[test]
fn fits_wide_tall_and_small_images_and_keeps_legacy_sizing() {
    for (pixels, expected) in [
        ((2000, 1000), (5_943_600, 2_971_800)),
        ((1000, 4000), (2_006_600, 8_026_400)),
        ((32, 16), (304_800, 152_400)),
        ((1200, 800), (5_943_600, 3_962_400)),
    ] {
        let bytes = insert(crate::create_blank_docx(), &operation(pixels.0, pixels.1));
        assert_eq!(extent(&bytes), expected);
        let xml = xml(&bytes);
        assert!(xml.contains(&format!(
            "<wp:extent cx=\"{}\" cy=\"{}\"",
            expected.0, expected.1
        )));
        assert!(xml.contains(&format!(
            "<a:ext cx=\"{}\" cy=\"{}\"",
            expected.0, expected.1
        )));
        assert!(
            (i128::from(expected.0) * i128::from(pixels.1)
                - i128::from(expected.1) * i128::from(pixels.0))
            .abs()
                <= i128::from(pixels.0.max(pixels.1))
        );
    }
    for (size, expected) in [
        (None, (2_381_250, 9_525_000)),
        (
            Some(PictureSizeChange::WidthEmu(9_144_000)),
            (9_144_000, 36_576_000),
        ),
        (
            Some(PictureSizeChange::HeightEmu(914_400)),
            (228_600, 914_400),
        ),
        (
            Some(PictureSizeChange::ExactEmu {
                width: 123,
                height: 456,
            }),
            (123, 456),
        ),
    ] {
        let operation = InsertPicture {
            auto_fit: false,
            size,
            ..operation(250, 1000)
        };
        assert_eq!(
            extent(&insert(crate::create_blank_docx(), &operation)),
            expected
        );
    }
}

#[test]
fn uses_landscape_margins_gutters_and_equal_or_unequal_columns() {
    for (page, margins, columns, width) in [
        (
            "15840\" w:h=\"12240\" w:orient=\"landscape",
            "1440",
            "",
            12_960,
        ),
        ("12240\" w:h=\"15840", "2880", "", 6_480),
        (
            "12240\" w:h=\"15840",
            "1440",
            "<w:cols w:num=\"2\" w:space=\"720\"/>",
            4_320,
        ),
        (
            "12240\" w:h=\"15840",
            "1440",
            "<w:cols w:equalWidth=\"0\"><w:col w:w=\"3000\" w:space=\"720\"/><w:col w:w=\"5640\"/></w:cols>",
            3_000,
        ),
    ] {
        let input = change(&crate::create_blank_docx(), |xml| {
            xml.replace("12240\" w:h=\"15840", page)
                .replace("w:left=\"1440\"", &format!("w:left=\"{margins}\""))
                .replace("w:right=\"1440\"", &format!("w:right=\"{margins}\""))
                .replace("</w:sectPr>", &format!("{columns}</w:sectPr>"))
        });
        let output = insert(input.clone(), &operation(4000, 1000));
        assert_eq!(extent(&output), (width * 635, width * 635 / 4));
        assert_parts_preserved(
            &input,
            &output,
            &[
                "/word/document.xml",
                "/word/_rels/document.xml.rels",
                "/[Content_Types].xml",
            ],
        );
    }
    let input = change(&crate::create_blank_docx(), |xml| {
        xml.replace("w:gutter=\"0\"", "w:gutter=\"720\"")
    });
    assert_eq!(
        extent(&insert(input, &operation(4000, 1000))).0,
        8_640 * 635
    );
}

#[test]
fn uses_default_style_indents_for_new_picture_paragraph() {
    let input = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let styles_name = PartName::parse("/word/styles.xml").unwrap();
    let styles = String::from_utf8(input.read_part_by_name(&styles_name).unwrap()).unwrap();
    // The default Normal style applies to a new unstyled picture paragraph.
    let styled = styles.replace("<w:name w:val=\"Normal\"/><w:pPr>", "<w:name w:val=\"Normal\"/><w:pPr><w:ind w:left=\"720\" w:right=\"360\" w:firstLine=\"180\"/>");
    assert_ne!(styled, styles);
    let bytes = input
        .write_replaced_part_to_vec(&input.part(&styles_name).unwrap(), styled.as_bytes())
        .unwrap();
    assert_eq!(
        extent(&insert(bytes, &operation(4000, 1000))).0,
        (9360 - 720 - 360 - 180) * 635
    );
}

#[test]
fn selects_the_section_at_each_side_of_a_section_boundary() {
    let input = change(&crate::create_blank_docx(), |xml| {
        let section =
            xml[xml.find("<w:sectPr>").unwrap()..xml.find("</w:sectPr>").unwrap() + 11].to_owned();
        xml.replace(
            "<w:body>",
            &format!(
                "<w:body><w:p><w:pPr>{}</w:pPr></w:p><w:p><w:r><w:t>Second</w:t></w:r></w:p>",
                section.replace("w:left=\"1440\"", "w:left=\"4320\"")
            ),
        )
    });
    for (placement, width) in [
        (ParagraphPlacement::Start, 6480),
        (
            ParagraphPlacement::Before {
                handle: "b0".into(),
            },
            6480,
        ),
        (
            ParagraphPlacement::After {
                handle: "b0".into(),
            },
            9360,
        ),
        (
            ParagraphPlacement::Before {
                handle: "b1".into(),
            },
            9360,
        ),
        (ParagraphPlacement::End, 9360),
    ] {
        let output = insert(
            input.clone(),
            &InsertPicture {
                placement: placement.into(),
                ..operation(4000, 1000)
            },
        );
        assert_eq!(extent(&output).0, width * 635);
        let package = Package::from_bytes(output).unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(
            crate::DocxDocument::new(&source)
                .unwrap()
                .sections()
                .count(),
            2
        );
    }
}

#[test]
fn aligns_the_actual_textless_picture_paragraph_without_extra_paragraphs() {
    for (alignment, value) in [
        (ParagraphAlignment::Left, "left"),
        (ParagraphAlignment::Center, "center"),
        (ParagraphAlignment::Right, "right"),
    ] {
        let input = crate::create_blank_docx();
        let output = insert(
            input,
            &InsertPicture {
                alignment: Some(alignment),
                ..operation(32, 16)
            },
        );
        let xml = xml(&output);
        assert!(xml.contains(&format!(
            "<w:p><w:pPr><w:jc w:val=\"{value}\"/></w:pPr><w:r><w:drawing>"
        )));
        assert_eq!(xml.matches("<w:p>").count(), 1);
        assert!(!xml.contains("<w:t>"));
    }
}

#[test]
fn deletion_retains_section_and_unknown_paragraph_properties_and_other_images() {
    let bytes = insert(crate::create_blank_docx(), &operation(32, 16));
    let bytes = insert(bytes, &operation(64, 32));
    let bytes = change(&bytes, |xml| {
        let section =
            xml[xml.find("<w:sectPr>").unwrap()..xml.find("</w:sectPr>").unwrap() + 11].to_owned();
        xml.replacen("<w:p>", &format!("<w:p><w:pPr><w:keepNext/><w:pPrChange w:id=\"9\"><w:pPr/></w:pPrChange>{section}<custom xmlns=\"urn:preserve\"/></w:pPr>"), 1)
    });
    let package = Package::from_bytes(bytes.clone()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let handle = crate::inspect_docx_layout(bytes.clone(), &Default::default()).images[0]
        .handle
        .clone()
        .unwrap();
    let output = delete_picture_to_vec(
        &package,
        &main,
        &source,
        &DeletePicture {
            target: PictureTarget {
                handle: Some(handle),
                name: None,
                description: None,
                occurrence: None,
            },
            base_revision: None,
        },
    )
    .unwrap();
    let drawing = source
        .node_ids()
        .find(|id| word(&source, *id, "drawing"))
        .unwrap();
    let span = source.node(drawing).unwrap().span();
    let expected = [
        &source.original_bytes()[..span.start],
        &source.original_bytes()[span.end..],
    ]
    .concat();
    assert_eq!(xml(&output).as_bytes(), expected);
    assert_parts_preserved(&bytes, &output, &["/word/document.xml"]);
    let reopened = Package::from_bytes(output).unwrap();
    let (_, after) = crate::open_main_source(&reopened).unwrap();
    let document = crate::DocxDocument::new(&after).unwrap();
    assert_eq!(document.sections().count(), 2);
    assert_eq!(document.pictures().count(), 1);
    assert_eq!(document.paragraphs().count(), 2);
}

#[test]
fn refuses_unknown_impossible_and_malformed_geometry_or_conflicting_options() {
    for (from, to) in [
        ("12240", "0"),
        ("12240", "wide"),
        ("w:left=\"1440\"", "w:left=\"13000\""),
        ("w:top=\"1440\"", "w:top=\"-1\""),
        ("<w:pgSz w:w=\"12240\" w:h=\"15840\"/>", ""),
        (
            "</w:sectPr>",
            "<w:cols w:num=\"2\" w:space=\"10000\"/></w:sectPr>",
        ),
        ("</w:sectPr>", "<w:cols w:num=\"0\"/></w:sectPr>"),
        ("</w:sectPr>", "<w:cols w:num=\"2\"/></w:sectPr>"),
        (
            "</w:sectPr>",
            "<w:cols w:equalWidth=\"0\"><w:col w:w=\"0\"/></w:cols></w:sectPr>",
        ),
    ] {
        let input = change(&crate::create_blank_docx(), |xml| xml.replace(from, to));
        let result = crate::execute_docx_insert_picture(input, &operation(2000, 1000));
        assert!(
            result.output_artifact.is_none(),
            "{from}: {:?}",
            result.operation
        );
        assert_eq!(
            result.operation.diagnostics[0].code,
            "INVALID_PAGE_GEOMETRY"
        );
    }
    for operation in [
        InsertPicture {
            size: Some(PictureSizeChange::WidthEmu(100)),
            ..operation(2, 1)
        },
        InsertPicture {
            alignment: Some(ParagraphAlignment::Both),
            ..operation(2, 1)
        },
        InsertPicture {
            image_bytes: b"\x89PNG\r\n\x1a\n".to_vec(),
            ..operation(2, 1)
        },
        operation(u32::MAX, 1),
    ] {
        let result = crate::execute_docx_insert_picture(crate::create_blank_docx(), &operation);
        assert!(result.output_artifact.is_none(), "{:?}", result.operation);
    }
}

#[test]
fn deletion_refuses_enclosing_ranges_and_mixed_picture_content() {
    let bytes = insert(crate::create_blank_docx(), &operation(32, 16));
    for (before, after) in [
        (
            "<w:bookmarkStart w:id=\"7\" w:name=\"keep\"/>",
            "<w:bookmarkEnd w:id=\"7\"/>",
        ),
        (
            "<w:p><w:r><w:fldChar w:fldCharType=\"begin\"/><w:instrText>DATE</w:instrText></w:r></w:p>",
            "<w:p><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>",
        ),
        ("<w:ins w:id=\"1\" w:author=\"Editor\">", "</w:ins>"),
        ("", "<w:r><w:t>Keep this text</w:t></w:r>"),
    ] {
        let input = change(&bytes, |xml| {
            if before.is_empty() {
                xml.replacen("</w:p>", &format!("{after}</w:p>"), 1)
            } else {
                xml.replacen("<w:p>", &format!("{before}<w:p>"), 1)
                    .replacen("</w:p><w:sectPr>", &format!("</w:p>{after}<w:sectPr>"), 1)
            }
        });
        let package = Package::from_bytes(input.clone()).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let handle = crate::inspect_docx_layout(input, &Default::default()).images[0]
            .handle
            .clone()
            .unwrap();
        assert!(
            delete_picture_to_vec(
                &package,
                &main,
                &source,
                &DeletePicture {
                    target: PictureTarget {
                        handle: Some(handle),
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
}

#[test]
fn refuses_default_paragraph_geometry_that_would_clip_an_image() {
    for spacing in [
        "<w:spacing w:line=\"200\" w:lineRule=\"exact\"/>",
        "<w:spacing w:before=\"16000\" w:after=\"16000\"/>",
    ] {
        let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
        let part = package
            .part(&PartName::parse("/word/styles.xml").unwrap())
            .unwrap();
        let styles = String::from_utf8(package.read_part(&part).unwrap()).unwrap();
        let styles = styles.replace(
            "<w:spacing w:before=\"160\" w:after=\"160\" w:line=\"276\" w:lineRule=\"auto\"/>",
            spacing,
        );
        let input = package
            .write_replaced_part_to_vec(&part, styles.as_bytes())
            .unwrap();
        let result = crate::execute_docx_insert_picture(input, &operation(1000, 4000));
        assert!(result.output_artifact.is_none());
    }
}

#[test]
fn deletion_refuses_protected_structures_hidden_inside_a_drawing() {
    let bytes = insert(crate::create_blank_docx(), &operation(32, 16));
    let bytes = change(&bytes, |xml| {
        xml.replace("</wp:inline>", "<w:sectPr/></wp:inline>")
    });
    let package = Package::from_bytes(bytes.clone()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let handle = crate::inspect_docx_layout(bytes, &Default::default()).images[0]
        .handle
        .clone()
        .unwrap();
    let result = delete_picture_to_vec(
        &package,
        &main,
        &source,
        &DeletePicture {
            target: PictureTarget {
                handle: Some(handle),
                name: None,
                description: None,
                occurrence: None,
            },
            base_revision: None,
        },
    )
    .unwrap_err();
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "UNSUPPORTED_PICTURE_DELETE"
                || diagnostic.message.contains("section properties"))
    );
}
