use super::*;
use opensuite_protocol::{
    CellPicturePosition, CreateTable, InspectDocx, InspectDocxContent, InspectDocxFocus,
    ParagraphAlignment, PicturePlacement, PictureSizeChange,
};

fn fixture() -> Vec<u8> {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    crate::create_table_to_vec(
        &package,
        &main,
        &source,
        &CreateTable {
            rows: vec![
                vec!["Item".into(), "Picture".into()],
                vec!["Logo".into(), "Keep this text".into()],
            ],
            placement: ParagraphPlacement::End,
            base_revision: None,
        },
    )
    .unwrap()
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
fn operation(
    bytes: &[u8],
    row: usize,
    column: usize,
    position: CellPicturePosition,
) -> InsertPicture {
    let inspection = crate::inspect_docx(
        bytes.to_vec(),
        &InspectDocx {
            focus: InspectDocxFocus::Tables {
                offset: 0,
                limit: 10,
            },
        },
    );
    let Some(InspectDocxContent::Tables(tables)) = inspection.content else {
        panic!("{inspection:?}")
    };
    let mut image = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    image.extend(2000u32.to_be_bytes());
    image.extend(1000u32.to_be_bytes());
    image.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
    InsertPicture {
        auto_fit: true,
        alignment: Some(ParagraphAlignment::Center),
        size: None,
        layout: None,
        image_bytes: image,
        placement: PicturePlacement::TableCell {
            handle: tables.items[0].rows[row].picture_cell_handles[column].clone(),
            position,
        },
        alt_text: Some("Cell picture".into()),
        base_revision: None,
    }
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
fn refuse(bytes: Vec<u8>, operation: &InsertPicture, code: &str) {
    let result = crate::execute_docx_insert_picture(bytes, operation);
    assert!(result.output_artifact.is_none());
    assert_eq!(
        result.operation.diagnostics[0].code, code,
        "{:?}",
        result.operation
    );
}
fn extent(bytes: &[u8]) -> (i64, i64) {
    let layout = crate::inspect_docx_layout(bytes.to_vec(), &Default::default());
    (
        layout.images[0].display_width_emu.unwrap(),
        layout.images[0].display_height_emu.unwrap(),
    )
}

#[test]
fn preserves_populated_cell_properties_multiple_paragraphs_and_unrelated_parts() {
    let bytes = change(&fixture(), |xml| {
        xml.replace("<w:tc>", "<w:tc><w:tcPr><w:shd w:fill=\"EEF4FA\"/><w:tcBorders><w:bottom w:val=\"single\"/></w:tcBorders></w:tcPr>")
        .replace("Keep this text</w:t></w:r></w:p>", "Keep this text</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"7\"/></w:numPr></w:pPr><w:r><w:t>Second paragraph</w:t></w:r></w:p>")
    });
    for position in [CellPicturePosition::Start, CellPicturePosition::End] {
        let operation = operation(&bytes, 1, 1, position);
        let output = insert(bytes.clone(), &operation);
        assert_eq!(
            extent(&output),
            ((4680 - 280) * 635, (4680 - 280) * 635 / 2)
        );
        let before = xml(&bytes);
        let after = xml(&output);
        let start = after.find("<w:p><w:pPr><w:jc").unwrap();
        let end = start + after[start..].find("</w:p>").unwrap() + 6;
        assert_eq!(format!("{}{}", &after[..start], &after[end..]), before);
        let text = after.find("Keep this text").unwrap();
        assert_eq!(start < text, position == CellPicturePosition::Start);
        let input_package = Package::from_bytes(bytes.clone()).unwrap();
        let output_package = Package::from_bytes(output).unwrap();
        let round_trip = output_package
            .write_package_with_named_changes_to_vec(&[], &[])
            .unwrap();
        let reopened = Package::from_bytes(round_trip.clone()).unwrap();
        reopened.verify().unwrap();
        assert_eq!(
            extent(&round_trip),
            ((4680 - 280) * 635, (4680 - 280) * 635 / 2)
        );
        for name in output_package.part_names_with_prefix("/") {
            assert_eq!(
                output_package.read_part_by_name(name).unwrap(),
                reopened.read_part_by_name(name).unwrap()
            );
        }
        for name in input_package.part_names_with_prefix("/") {
            if ![
                "/word/document.xml",
                "/word/_rels/document.xml.rels",
                "/[Content_Types].xml",
            ]
            .contains(&name.as_str())
            {
                assert_eq!(
                    input_package.read_part_by_name(name).unwrap(),
                    output_package.read_part_by_name(name).unwrap(),
                    "{name:?}"
                );
            }
        }
    }
}

#[test]
fn margins_widths_defaults_and_paragraph_bounds_use_existing_proportional_fitter() {
    for (edit, expected_width) in [
        (
            "<w:tcPr><w:tcW w:type=\"dxa\" w:w=\"4680\"/><w:tcMar><w:left w:w=\"400\" w:type=\"dxa\"/></w:tcMar></w:tcPr>",
            4140,
        ),
        (
            "<w:tcPr><w:tcW w:type=\"dxa\" w:w=\"4680\"/></w:tcPr>",
            4400,
        ),
    ] {
        let bytes = change(&fixture(), |xml| {
            xml.replace("<w:tc>", &format!("<w:tc>{edit}"))
        });
        let output = insert(
            bytes.clone(),
            &operation(&bytes, 1, 1, CellPicturePosition::End),
        );
        assert_eq!(extent(&output).0, expected_width * 635);
    }
    let bytes = change(&fixture(), |xml| {
        let start = xml.find("<w:tblCellMar>").unwrap();
        let end = xml.find("</w:tblCellMar>").unwrap() + 15;
        format!("{}{}", &xml[..start], &xml[end..])
    });
    let output = insert(
        bytes.clone(),
        &operation(&bytes, 1, 1, CellPicturePosition::End),
    );
    assert_eq!(extent(&output).0, (4680 - 216) * 635);
    let bytes = fixture();
    let mut op = operation(&bytes, 1, 1, CellPicturePosition::End);
    // A small intrinsic image stays small; explicit sizes retain their existing semantics.
    op.image_bytes[16..20].copy_from_slice(&32u32.to_be_bytes());
    op.image_bytes[20..24].copy_from_slice(&16u32.to_be_bytes());
    assert_eq!(extent(&insert(bytes.clone(), &op)), (304800, 152400));
    op.auto_fit = false;
    op.size = Some(PictureSizeChange::WidthEmu(914400));
    assert_eq!(extent(&insert(bytes.clone(), &op)), (914400, 457200));
    op.size = Some(PictureSizeChange::ExactEmu {
        width: 100,
        height: 300,
    });
    assert_eq!(extent(&insert(bytes, &op)), (100, 300));
}

#[test]
fn two_cells_and_repeated_insertions_have_distinct_media_and_fresh_handles() {
    let bytes = fixture();
    let first = operation(&bytes, 1, 0, CellPicturePosition::End);
    let output = insert(bytes, &first);
    refuse(output.clone(), &first, "PRECONDITION_FAILED");
    let output = insert(
        output.clone(),
        &operation(&output, 1, 1, CellPicturePosition::Start),
    );
    let output = insert(
        output.clone(),
        &operation(&output, 1, 0, CellPicturePosition::Start),
    );
    let package = Package::from_bytes(output.clone()).unwrap();
    package.verify().unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let pictures = crate::DocxDocument::new(&source)
        .unwrap()
        .pictures()
        .collect::<Vec<_>>();
    assert_eq!(pictures.len(), 3);
    let mut relationships = std::collections::HashSet::new();
    for picture in pictures {
        relationships.insert(picture.relationship_id().unwrap().unwrap().to_owned());
        assert!(matches!(
            picture.image_reference(&package, &main).unwrap(),
            crate::ImageReference::Embedded(_)
        ));
    }
    assert_eq!(relationships.len(), 3);
    // Cell pictures use the existing layout handles for resize, replacement, and deletion.
    let layout = crate::inspect_docx_layout(output.clone(), &Default::default());
    let target = opensuite_protocol::PictureTarget {
        handle: layout.images[0].handle.clone(),
        name: None,
        description: None,
        occurrence: None,
    };
    let resized = crate::execute_docx_set_picture_size(
        output,
        &SetPictureSize {
            target,
            size: PictureSizeChange::HeightEmu(635000),
            base_revision: None,
        },
    );
    assert!(resized.output_artifact.is_some(), "{:?}", resized.operation);
    let output = resized.output_artifact.unwrap();
    assert_eq!(extent(&output), (1270000, 635000));
    let layout = crate::inspect_docx_layout(output.clone(), &Default::default());
    let deleted = crate::execute_docx_delete_picture(
        output,
        &DeletePicture {
            target: opensuite_protocol::PictureTarget {
                handle: layout.images[0].handle.clone(),
                name: None,
                description: None,
                occurrence: None,
            },
            base_revision: None,
        },
    );
    assert!(deleted.output_artifact.is_some(), "{:?}", deleted.operation);
    assert_eq!(
        crate::inspect_docx_layout(deleted.output_artifact.unwrap(), &Default::default())
            .image_count,
        2
    );
}

#[test]
fn invalid_stale_and_protected_cell_targets_fail_without_output() {
    let bytes = fixture();
    let mut op = operation(&bytes, 1, 1, CellPicturePosition::End);
    for handle in [
        "t0:r1:c1".to_owned(),
        "bad".into(),
        "t0:r1:c1:0000000000000000".into(),
    ] {
        op.placement = PicturePlacement::TableCell {
            handle,
            position: CellPicturePosition::End,
        };
        refuse(bytes.clone(), &op, "PRECONDITION_FAILED");
    }
    let op = operation(&bytes, 1, 1, CellPicturePosition::End);
    let changed = change(&bytes, |xml| {
        xml.replace("<w:tbl>", "<w:p><w:r><w:t>Changed</w:t></w:r></w:p><w:tbl>")
    });
    refuse(changed, &op, "PRECONDITION_FAILED");
    let bytes = change(&bytes, |xml| {
        xml.replace(
            "<w:tbl>",
            "<w:bookmarkStart w:id=\"1\" w:name=\"Protected\"/><w:tbl>",
        )
        .replace("</w:tbl>", "</w:tbl><w:bookmarkEnd w:id=\"1\"/>")
    });
    refuse(
        bytes.clone(),
        &operation(&bytes, 1, 1, CellPicturePosition::End),
        "UNSUPPORTED_OPERATION",
    );
}

#[test]
fn merged_nested_wrapped_and_ambiguous_geometry_are_refused() {
    let bytes = fixture();
    for property in ["<w:gridSpan w:val=\"2\"/>", "<w:vMerge/>", "<w:hMerge/>"] {
        let bytes = change(&bytes, |xml| {
            xml.replace("<w:tc>", &format!("<w:tc><w:tcPr>{property}</w:tcPr>"))
        });
        refuse(
            bytes.clone(),
            &operation(&bytes, 1, 1, CellPicturePosition::End),
            "UNSUPPORTED_OPERATION",
        );
    }
    let nested = change(&bytes, |xml| {
        xml.replacen(
            "</w:tc>",
            "<w:tbl><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl></w:tc>",
            1,
        )
    });
    refuse(
        nested.clone(),
        &operation(&nested, 1, 1, CellPicturePosition::End),
        "UNSUPPORTED_OPERATION",
    );
    for (old, new) in [
        ("w:w=\"4680\"", "w:w=\"0\""),
        ("<w:tblGrid>", "<w:tblGrid><w:gridCol w:w=\"400\"/>"),
        (
            "<w:tc>",
            "<w:tc><w:tcPr><w:tcW w:w=\"50\" w:type=\"pct\"/></w:tcPr>",
        ),
        ("<w:tblPr>", "<w:tblPr><w:tblStyle w:val=\"Unknown\"/>"),
        (
            "<w:tc>",
            "<w:tc><w:tcPr><w:tcMar><w:left w:type=\"dxa\" w:w=\"5000\"/></w:tcMar></w:tcPr>",
        ),
        (
            "<w:tc>",
            "<w:tc><w:tcPr><w:textDirection w:val=\"tbRl\"/></w:tcPr>",
        ),
        (
            "<w:tblPr>",
            "<w:tblPr><w:tblCellSpacing w:w=\"40\" w:type=\"dxa\"/>",
        ),
        (
            "<w:tc>",
            "<w:tc><w:tcPr><w:tcW w:type=\"dxa\" w:w=\"4680\"/><w:tcW w:type=\"dxa\" w:w=\"4680\"/></w:tcPr>",
        ),
    ] {
        let bytes = change(&bytes, |xml| xml.replace(old, new));
        refuse(
            bytes.clone(),
            &operation(&bytes, 1, 1, CellPicturePosition::End),
            "INVALID_CELL_GEOMETRY",
        );
    }
}

#[test]
fn cell_fitting_uses_vertical_margins_and_default_paragraph_indents() {
    let bytes = fixture();
    let mut op = operation(&bytes, 1, 1, CellPicturePosition::End);
    op.image_bytes[16..20].copy_from_slice(&1000u32.to_be_bytes());
    op.image_bytes[20..24].copy_from_slice(&4000u32.to_be_bytes());
    assert_eq!(extent(&insert(bytes.clone(), &op)), (1_974_850, 7_899_400));
    let package = Package::from_bytes(bytes).unwrap();
    let name = PartName::parse("/word/styles.xml").unwrap();
    let styles = String::from_utf8(package.read_part_by_name(&name).unwrap()).unwrap();
    let styled = styles.replace("<w:name w:val=\"Normal\"/><w:pPr>", "<w:name w:val=\"Normal\"/><w:pPr><w:ind w:left=\"720\" w:right=\"360\" w:firstLine=\"180\"/>");
    assert_ne!(styled, styles);
    let bytes = package
        .write_replaced_part_to_vec(&package.part(&name).unwrap(), styled.as_bytes())
        .unwrap();
    assert_eq!(
        extent(&insert(
            bytes.clone(),
            &operation(&bytes, 1, 1, CellPicturePosition::End)
        ))
        .0,
        (4400 - 720 - 360 - 180) * 635
    );
    let styled = styles.replace("</w:styles>", "<w:style w:type=\"table\" w:default=\"1\" w:styleId=\"DefaultTable\"><w:name w:val=\"Default table\"/></w:style></w:styles>");
    let bytes = package
        .write_replaced_part_to_vec(&package.part(&name).unwrap(), styled.as_bytes())
        .unwrap();
    refuse(
        bytes.clone(),
        &operation(&bytes, 1, 1, CellPicturePosition::End),
        "INVALID_CELL_GEOMETRY",
    );
}

#[test]
fn picture_cell_handles_select_the_exact_direct_table_after_a_wrapped_table() {
    let bytes = change(&fixture(), |xml| {
        let table = &xml[xml.find("<w:tbl>").unwrap()..xml.find("</w:tbl>").unwrap() + 8];
        xml.replace(
            "<w:body>",
            &format!("<w:body><w:sdt><w:sdtContent>{table}</w:sdtContent></w:sdt>"),
        )
    });
    let inspection = crate::inspect_docx(
        bytes.clone(),
        &InspectDocx {
            focus: InspectDocxFocus::Tables {
                offset: 0,
                limit: 10,
            },
        },
    );
    let Some(InspectDocxContent::Tables(tables)) = inspection.content else {
        panic!()
    };
    assert!(tables.items[0].rows[1].picture_cell_handles.is_empty());
    let mut op = operation(&fixture(), 1, 1, CellPicturePosition::End);
    op.placement = PicturePlacement::TableCell {
        handle: tables.items[1].rows[1].picture_cell_handles[1].clone(),
        position: CellPicturePosition::End,
    };
    let before = xml(&bytes);
    let output = insert(bytes, &op);
    let after = xml(&output);
    let wrapped_end = before.find("</w:sdt>").unwrap() + 8;
    assert_eq!(&before[..wrapped_end], &after[..wrapped_end]);
    assert!(after.find("<w:drawing>").unwrap() > wrapped_end);
    // Even changing a different table invalidates the old source stamp.
    let changed = change(&output, |xml| xml.replacen("Item", "Changed", 1));
    refuse(changed, &op, "PRECONDITION_FAILED");
}

#[test]
fn fixed_height_conflicting_width_and_property_history_fail_safely() {
    for (old, new, code) in [
        (
            "<w:tr>",
            "<w:tr><w:trPr><w:trHeight w:val=\"500\" w:hRule=\"exact\"/></w:trPr>",
            "INVALID_CELL_GEOMETRY",
        ),
        (
            "<w:tc>",
            "<w:tc><w:tcPr><w:tcW w:w=\"4500\" w:type=\"dxa\"/></w:tcPr>",
            "INVALID_CELL_GEOMETRY",
        ),
        (
            "<w:tc>",
            "<w:tc><w:tcPr><w:tcPrChange w:id=\"1\"><w:tcPr/></w:tcPrChange></w:tcPr>",
            "UNSUPPORTED_OPERATION",
        ),
        (
            "<w:tr>",
            "<w:tr><w:trPr><w:gridBefore w:val=\"1\"/></w:trPr>",
            "UNSUPPORTED_OPERATION",
        ),
        (
            "<w:tc>",
            "<w:tc><w:tcPr><w:tcMar><w:start w:w=\"100\" w:type=\"dxa\"/></w:tcMar></w:tcPr>",
            "INVALID_CELL_GEOMETRY",
        ),
    ] {
        let bytes = change(&fixture(), |xml| xml.replace(old, new));
        refuse(
            bytes.clone(),
            &operation(&bytes, 1, 1, CellPicturePosition::End),
            code,
        );
    }
}

#[test]
fn cell_inspection_and_first_picture_work_without_a_document_relationship_part() {
    use std::io::{Cursor, Write};
    let package = Package::from_bytes(fixture()).unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for name in package.part_names_with_prefix("/") {
        if name.as_str() == "/word/_rels/document.xml.rels" {
            continue;
        }
        zip.start_file(
            name.as_str().trim_start_matches('/'),
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&package.read_part_by_name(name).unwrap())
            .unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    let op = operation(&bytes, 1, 1, CellPicturePosition::End);
    let output = insert(bytes, &op);
    assert_eq!(extent(&output), (2794000, 1397000));
    let package = Package::from_bytes(output).unwrap();
    package.verify().unwrap();
    assert_eq!(
        package
            .part_relationships(&package.main_office_document().unwrap())
            .unwrap()
            .len(),
        1
    );
}
