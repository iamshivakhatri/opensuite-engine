use super::super::*;
use super::support::*;
use opensuite_protocol::{
    TableCellColumn, TableCellFormattingUpdate, TableCellRow, TableCellTextFormatting,
};

fn input() -> std::path::PathBuf {
    table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>Status</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:rPr><w:u/></w:rPr><w:t>Owner</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Actual</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:hyperlink><w:r><w:t>Status</w:t></w:r></w:hyperlink></w:p></w:tc><w:tc><w:p><w:r><w:t>Alice</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>10</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Other</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Table</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    ))
}

fn update(handle: &str, fill: Option<&str>, bold: bool) -> TableCellFormattingUpdate {
    TableCellFormattingUpdate {
        alignment: None,
        vertical_alignment: None,
        target: TableCellTarget {
            row: None,
            column: None,
            row_label: String::new(),
            column_header: String::new(),
            occurrence: None,
            handle: Some(handle.to_owned()),
        },
        fill: fill.map(str::to_owned),
        text_formatting: bold.then_some(TableCellTextFormatting {
            bold: Some(true),
            italic: Some(true),
            font_family: Some("Aptos".to_owned()),
            font_size_half_points: Some(24),
            color: Some("FFFFFF".to_owned()),
        }),
    }
}

fn semantic_update(
    row: TableCellRow,
    column: TableCellColumn,
    fill: Option<&str>,
    bold: bool,
) -> TableCellFormattingUpdate {
    let mut update = update("", fill, bold);
    update.target = TableCellTarget {
        row: Some(row),
        column: Some(column),
        handle: None,
        ..update.target
    };
    update
}

fn operation(updates: Vec<TableCellFormattingUpdate>) -> SetTableCellsFormatting {
    SetTableCellsFormatting {
        table: TableTarget {
            header_cells: Vec::new(),
            occurrence: None,
            handle: Some("t0".to_owned()),
        },
        updates,
        base_revision: None,
    }
}

fn cell_xml(bytes: &[u8], table_index: usize, row_index: usize, column_index: usize) -> String {
    let package = Package::from_bytes(bytes.to_vec()).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    let (_, _, rows, _) = resolve_table(
        &source,
        &TableTarget {
            header_cells: Vec::new(),
            occurrence: None,
            handle: Some(format!("t{table_index}")),
        },
    )
    .unwrap();
    let cell = rows[row_index].cells().nth(column_index).unwrap();
    String::from_utf8(source_bytes(&source, cell.source_id()).unwrap()).unwrap()
}

#[test]
fn formats_three_header_cells_without_touching_repeated_text_elsewhere() {
    let path = input();
    let original = fs::read(&path).unwrap();
    let operation = operation(
        (0..3)
            .map(|column| {
                semantic_update(
                    TableCellRow::Header,
                    TableCellColumn::Header {
                        text: ["Status", "Owner", "Actual"][column].into(),
                        occurrence: None,
                    },
                    Some("17365d"),
                    true,
                )
            })
            .collect(),
    );
    let result = crate::execute_docx_set_table_cells_formatting(original.clone(), &operation);
    assert_eq!(
        result.operation.status,
        opensuite_protocol::OperationStatus::Applied
    );
    let mut wrong = operation.clone();
    wrong.table.header_cells = vec!["Wrong".into()];
    let failed = crate::execute_docx_set_table_cells_formatting(original.clone(), &wrong);
    assert!(failed.output_artifact.is_none());
    assert_eq!(
        failed.operation.diagnostics[0].reason_code.as_deref(),
        Some("EXPECTED_TEXT_MISMATCH")
    );
    let output = result.output_artifact.unwrap();
    for column in 0..3 {
        let cell = cell_xml(&output, 0, 0, column);
        for expected in [
            "w:fill=\"17365D\"",
            "<w:b",
            "<w:i",
            "w:ascii=\"Aptos\"",
            "w:val=\"24\"",
            "w:val=\"FFFFFF\"",
        ] {
            assert!(cell.contains(expected), "missing {expected} in {cell}");
        }
    }
    assert!(cell_xml(&output, 0, 0, 1).contains("<w:u/>"));
    assert_eq!(cell_xml(&output, 0, 1, 0), cell_xml(&original, 0, 1, 0));
    assert_eq!(cell_xml(&output, 1, 0, 0), cell_xml(&original, 1, 0, 0));
    let package = Package::from_bytes(output).unwrap();
    let xml = String::from_utf8(
        package
            .read_part(&package.main_office_document().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(xml.contains("<w:body><w:p><w:r><w:t>Status</w:t></w:r></w:p>"));
    fs::remove_file(path).unwrap();
}

#[test]
fn semantic_cells_require_occurrences_for_duplicate_labels_and_headers() {
    let path = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Value</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Value</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Value</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Total</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>10</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>20</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Total</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>30</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>40</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    ));
    let original = fs::read(&path).unwrap();
    let row = TableCellRow::Label {
        text: "Total".into(),
        occurrence: Some(1),
    };
    let column = TableCellColumn::Header {
        text: "Value".into(),
        occurrence: Some(2),
    };
    let updates = vec![
        semantic_update(row.clone(), TableCellColumn::First, Some("ABCDEF"), true),
        semantic_update(row.clone(), column.clone(), None, true),
    ];
    let result = crate::execute_docx_set_table_cells_formatting(
        original.clone(),
        &operation(updates.clone()),
    );
    assert_eq!(
        result.operation.status,
        opensuite_protocol::OperationStatus::Applied
    );
    let output = result.output_artifact.unwrap();
    assert!(cell_xml(&output, 0, 2, 0).contains("w:fill=\"ABCDEF\""));
    assert!(cell_xml(&output, 0, 2, 2).contains("<w:b"));
    assert_eq!(cell_xml(&output, 0, 1, 0), cell_xml(&original, 0, 1, 0));
    assert_eq!(cell_xml(&output, 0, 2, 1), cell_xml(&original, 0, 2, 1));
    for ambiguous in [
        semantic_update(
            TableCellRow::Label {
                text: "Total".into(),
                occurrence: None,
            },
            TableCellColumn::First,
            None,
            true,
        ),
        semantic_update(
            row.clone(),
            TableCellColumn::Header {
                text: "Value".into(),
                occurrence: None,
            },
            None,
            true,
        ),
    ] {
        let failed = crate::execute_docx_set_table_cells_formatting(
            original.clone(),
            &operation(vec![updates[0].clone(), ambiguous]),
        );
        assert!(failed.output_artifact.is_none());
        assert_eq!(failed.operation.diagnostics[0].code, "TARGET_AMBIGUOUS");
        assert_eq!(failed.operation.diagnostics[0].update_index, Some(1));
        assert!(
            failed.operation.diagnostics[0]
                .target_description
                .as_deref()
                .unwrap()
                .contains("Total")
        );
    }
    let wrong_handle = crate::execute_docx_set_table_cells_formatting(
        original.clone(),
        &operation(vec![
            update("t0:r0:c0", Some("ABCDEF"), false),
            update("t0:r9:c1", Some("ABCDEF"), false),
        ]),
    );
    let diagnostic = &wrong_handle.operation.diagnostics[0];
    assert!(wrong_handle.output_artifact.is_none());
    assert_eq!(diagnostic.update_index, Some(1));
    assert_eq!(
        diagnostic
            .target
            .as_ref()
            .map(|target| target.handle.as_str()),
        Some("t0:r9:c1")
    );
    assert_eq!(
        diagnostic.reason_code.as_deref(),
        Some("TABLE_ROW_NOT_FOUND")
    );
    let duplicate = crate::execute_docx_set_table_cells_formatting(
        original,
        &operation(vec![updates[0].clone(), updates[0].clone()]),
    );
    assert!(duplicate.output_artifact.is_none());
    assert_eq!(
        duplicate.operation.diagnostics[0].code,
        "PRECONDITION_FAILED"
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn supports_fill_only_and_text_only_and_rejects_all_invalid_updates_atomically() {
    let path = input();
    let original = fs::read(&path).unwrap();
    for (update, has_fill) in [
        (update("t0:r0:c0", Some("ABCDEF"), false), true),
        (update("t0:r0:c0", None, true), false),
    ] {
        let result = crate::execute_docx_set_table_cells_formatting(
            original.clone(),
            &operation(vec![update]),
        );
        let cell = cell_xml(&result.output_artifact.unwrap(), 0, 0, 0);
        assert_eq!(cell.contains("w:fill=\"ABCDEF\""), has_fill);
        assert_eq!(cell.contains("<w:b"), !has_fill);
    }
    let valid = update("t0:r0:c0", Some("ABCDEF"), true);
    let unsupported = update("t0:r1:c0", None, true);
    for invalid in [
        update("t1:r0:c0", None, true),
        update("t0:r9:c0", None, true),
        valid.clone(),
        unsupported.clone(),
    ] {
        let result = crate::execute_docx_set_table_cells_formatting(
            original.clone(),
            &operation(vec![valid.clone(), invalid]),
        );
        assert!(result.output_artifact.is_none());
        assert_ne!(
            result.operation.status,
            opensuite_protocol::OperationStatus::Applied
        );
    }
    assert!(!cell_xml(&original, 0, 0, 0).contains("<w:b"));
    fs::remove_file(path).unwrap();
}

fn alignment_update() -> TableCellFormattingUpdate {
    let mut u = update("t0:r0:c0", None, false);
    u.alignment = Some(PropertyPatch::Set(TableAlignment::Right));
    u.vertical_alignment = Some(PropertyPatch::Set(
        opensuite_protocol::CellVerticalAlignment::Bottom,
    ));
    u
}

#[test]
fn aligns_all_cell_paragraphs_and_preserves_text_and_existing_cell_properties() {
    let keep = "<w:tcW w:w=\"2400\" w:type=\"dxa\"/><w:tcBorders><w:top w:val=\"single\" w:sz=\"8\"/></w:tcBorders><w:tcMar><w:left w:w=\"90\" w:type=\"dxa\"/></w:tcMar>";
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:tcPr>{keep}<w:hideMark/></w:tcPr><w:p><w:pPr><w:spacing w:after=\"80\"/></w:pPr><w:r><w:rPr><w:i/></w:rPr><w:t>First</w:t></w:r></w:p><w:p/><w:p><w:r><w:t>Second</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    ));
    let original = fs::read(&input).unwrap();
    let mut u = alignment_update();
    u.fill = Some("ABCDEF".into());
    let op = operation(vec![u]);
    let output = crate::execute_docx_set_table_cells_formatting(original.clone(), &op)
        .output_artifact
        .unwrap();
    let xml = cell_xml(&output, 0, 0, 0);
    assert_eq!(xml.matches("<w:jc w:val=\"right\"/>").count(), 3);
    assert_eq!(xml.matches("<w:vAlign w:val=\"bottom\"/>").count(), 1);
    assert!(xml.contains("<w:vAlign w:val=\"bottom\"/><w:hideMark/>"));
    for keep in [
        "<w:tcW w:w=\"2400\" w:type=\"dxa\"/>",
        "<w:tcMar><w:left w:w=\"90\" w:type=\"dxa\"/></w:tcMar>",
        "<w:i/>",
        "<w:spacing w:after=\"80\"/>",
    ] {
        assert!(xml.contains(keep));
    }
    let repeated = crate::execute_docx_set_table_cells_formatting(output.clone(), &op)
        .output_artifact
        .unwrap();
    assert_eq!(cell_xml(&repeated, 0, 0, 0), xml);
    let mut clear = alignment_update();
    clear.alignment = Some(PropertyPatch::Clear);
    clear.vertical_alignment = Some(PropertyPatch::Clear);
    let cleared = crate::execute_docx_set_table_cells_formatting(output, &operation(vec![clear]))
        .output_artifact
        .unwrap();
    let xml = cell_xml(&cleared, 0, 0, 0);
    assert!(!xml.contains("<w:jc"));
    assert!(!xml.contains("<w:vAlign"));
    assert!(xml.contains("w:fill=\"ABCDEF\""));
    fs::remove_file(input).unwrap();
}

#[test]
fn alignment_handles_absent_and_empty_properties_and_clears_without_new_defaults() {
    for properties in ["", "<w:tcPr/>", "<w:tcPr></w:tcPr>"] {
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc>{properties}<w:p/></w:tc></w:tr></w:tbl></w:body></w:document>"
        ));
        let original = fs::read(&input).unwrap();
        let mut clear = alignment_update();
        clear.alignment = Some(PropertyPatch::Clear);
        clear.vertical_alignment = Some(PropertyPatch::Clear);
        let cleared = crate::execute_docx_set_table_cells_formatting(
            original.clone(),
            &operation(vec![clear]),
        )
        .output_artifact
        .unwrap();
        assert_eq!(cell_xml(&cleared, 0, 0, 0), cell_xml(&original, 0, 0, 0));
        for (horizontal, vertical) in [
            (
                TableAlignment::Left,
                opensuite_protocol::CellVerticalAlignment::Top,
            ),
            (
                TableAlignment::Center,
                opensuite_protocol::CellVerticalAlignment::Center,
            ),
            (
                TableAlignment::Right,
                opensuite_protocol::CellVerticalAlignment::Bottom,
            ),
        ] {
            let mut u = alignment_update();
            u.alignment = Some(PropertyPatch::Set(horizontal));
            u.vertical_alignment = Some(PropertyPatch::Set(vertical));
            u.fill = Some("ABCDEF".into());
            let output = crate::execute_docx_set_table_cells_formatting(
                original.clone(),
                &operation(vec![u]),
            )
            .output_artifact
            .unwrap();
            let xml = cell_xml(&output, 0, 0, 0);
            assert_eq!(xml.matches("<w:tcPr").count(), 1);
            assert_eq!(xml.matches("<w:vAlign").count(), 1);
            assert_eq!(xml.matches("<w:jc").count(), 1);
            assert_eq!(xml.matches("<w:shd").count(), 1);
        }
        fs::remove_file(input).unwrap();
    }
}

#[test]
fn alignment_refuses_merged_nested_wrapped_and_protected_tables_and_duplicate_properties() {
    let base = format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Text</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    );
    for (from, to) in [
        ("<w:tc>", "<w:tc><w:tcPr><w:gridSpan w:val=\"2\"/></w:tcPr>"),
        ("<w:tc>", "<w:tc><w:tcPr><w:hMerge/></w:tcPr>"),
        ("<w:tc>", "<w:tc><w:tbl/>"),
        ("<w:tbl>", "<w:tbl><w:tblPr><w:tblpPr/></w:tblPr>"),
        ("<w:tr>", "<w:tr><w:trPr><w:trPrChange/></w:trPr>"),
        ("<w:p>", "<w:p><w:bookmarkStart w:id=\"1\"/>"),
        ("<w:p>", "<w:p><w:pPr><w:jc/><w:jc/></w:pPr>"),
        ("<w:tc>", "<w:tc><w:tcPr><w:vAlign/><w:vAlign/></w:tcPr>"),
        ("<w:tc>", "<w:tc><w:tcPr/><w:tcPr/>"),
        ("<w:p>", "<w:p><w:pPr/><w:pPr/>"),
        (
            "<w:tc>",
            "<w:tc><w:tcPr><w:vAlign w:val=\"top\" extra=\"keep\"/></w:tcPr>",
        ),
        (
            "<w:p>",
            "<w:p><w:pPr><w:jc w:val=\"left\" extra=\"keep\"/></w:pPr>",
        ),
    ] {
        let input = table_fixture(&base.replace(from, to));
        let result = crate::execute_docx_set_table_cells_formatting(
            fs::read(&input).unwrap(),
            &operation(vec![alignment_update()]),
        );
        assert!(result.output_artifact.is_none(), "{to}");
        fs::remove_file(input).unwrap();
    }
}
