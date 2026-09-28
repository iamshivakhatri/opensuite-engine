use super::super::*;
use super::support::*;
use opensuite_protocol::{TableCellFormattingUpdate, TableCellTextFormatting};

fn input() -> std::path::PathBuf {
    table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>Status</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:rPr><w:u/></w:rPr><w:t>Owner</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Actual</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:hyperlink><w:r><w:t>Status</w:t></w:r></w:hyperlink></w:p></w:tc><w:tc><w:p><w:r><w:t>Alice</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>10</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Other</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Table</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    ))
}

fn update(handle: &str, fill: Option<&str>, bold: bool) -> TableCellFormattingUpdate {
    TableCellFormattingUpdate {
        target: TableCellTarget {
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
            .map(|column| update(&format!("t0:r0:c{column}"), Some("17365d"), true))
            .collect(),
    );
    let result = crate::execute_docx_set_table_cells_formatting(original.clone(), &operation);
    assert_eq!(
        result.operation.status,
        opensuite_protocol::OperationStatus::Applied
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
