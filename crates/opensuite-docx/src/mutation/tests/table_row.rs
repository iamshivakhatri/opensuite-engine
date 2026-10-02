use super::super::*;
use super::support::*;

#[test]
fn inserts_a_safe_row_after_a_semantic_anchor_and_preserves_tables() {
    let xml = format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>Before</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:tcPr><w:shd w:fill=\"DDDDDD\"/></w:tcPr><w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Role</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:shd w:fill=\"EEEEEE\"/></w:tcPr><w:p><w:pPr><w:jc w:val=\"left\"/></w:pPr><w:r><w:rPr><w:i/></w:rPr><w:t>Alice</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CEO</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:shd w:fill=\"EEEEEE\"/></w:tcPr><w:p><w:pPr><w:jc w:val=\"left\"/></w:pPr><w:r><w:rPr><w:i/></w:rPr><w:t>Bob</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CTO</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Other</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Table</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Keep</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Same</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    );
    let input = table_fixture(&xml);
    let output = row_execute(
        &input,
        &row_operation(&["Name", "Role"], "Bob", &[" Charlie ", "CFO & < >"]),
    )
    .unwrap();
    let package = Package::from_bytes(output.clone()).unwrap();
    package.verify().unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        all_table_rows(&source).unwrap(),
        vec![
            vec![
                vec!["Name".to_owned(), "Role".to_owned()],
                vec!["Alice".to_owned(), "CEO".to_owned()],
                vec!["Bob".to_owned(), "CTO".to_owned()],
                vec![" Charlie ".to_owned(), "CFO & < >".to_owned()],
            ],
            vec![
                vec!["Other".to_owned(), "Table".to_owned()],
                vec!["Keep".to_owned(), "Same".to_owned()],
            ],
        ]
    );
    let output_xml = String::from_utf8(
        package
            .read_part(&package.main_office_document().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(output_xml.contains("xml:space=\"preserve\"> Charlie </w:t>"));
    assert!(output_xml.contains("CFO &amp; &lt; &gt;"));
    assert!(output_xml.contains("<w:shd w:fill=\"EEEEEE\"/>"));
    let inserted = &table_row_xml(&output)[3];
    assert!(inserted.contains("<w:shd w:fill=\"EEEEEE\"/>"));
    assert!(inserted.contains("<w:i/>"));
    assert!(!inserted.contains("<w:b/>"));
    assert!(!inserted.contains("<w:shd w:fill=\"DDDDDD\"/>"));
    assert_eq!(entry(&input, "word/media/image.bin"), vec![1, 2, 3]);
    fs::remove_file(input).unwrap();
}

#[test]
fn inserts_multiple_rows_contiguously_and_validates_all_rows_first() {
    let xml = format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Role</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Alice</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CEO</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Bob</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CTO</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    );
    let input = table_fixture(&xml);
    let operation = InsertTableRowsAfter {
        table: TableTarget {
            header_cells: vec!["Name".to_owned(), "Role".to_owned()],
            occurrence: None,
            handle: None,
        },
        after: TableRowTarget {
            first_cell_text: "Alice".to_owned(),
            occurrence: None,
            handle: None,
        },
        rows: vec![
            vec!["Charlie".to_owned(), "CFO".to_owned()],
            vec!["David".to_owned(), "COO".to_owned()],
            vec!["Emma".to_owned(), String::new()],
        ],
        base_revision: None,
    };
    let output = rows_execute(&input, &operation).unwrap();
    let package = Package::from_bytes(output.clone()).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        all_table_rows(&source).unwrap()[0][2..],
        [
            vec!["Charlie", "CFO"],
            vec!["David", "COO"],
            vec!["Emma", ""],
            vec!["Bob", "CTO"]
        ]
    );
    assert_eq!(entry(&input, "word/media/image.bin"), vec![1, 2, 3]);

    let after_final = InsertTableRowsAfter {
        table: TableTarget {
            header_cells: Vec::new(),
            occurrence: None,
            handle: Some("t0".to_owned()),
        },
        after: TableRowTarget {
            first_cell_text: String::new(),
            occurrence: None,
            handle: Some("t0:r2".to_owned()),
        },
        rows: vec![
            vec!["Final one".to_owned(), "CIO".to_owned()],
            vec!["Final two".to_owned(), "CPO".to_owned()],
        ],
        ..operation.clone()
    };
    let output = rows_execute(&input, &after_final).unwrap();
    let package = Package::from_bytes(output).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        all_table_rows(&source).unwrap()[0][3..],
        [vec!["Final one", "CIO"], vec!["Final two", "CPO"]]
    );

    let mut invalid = operation;
    invalid.rows[1].pop();
    assert_eq!(
        rows_execute(&input, &invalid).unwrap_err().diagnostics[0].code,
        "PRECONDITION_FAILED"
    );
    fs::remove_file(input).unwrap();
}

#[test]
fn rejects_unsafe_or_ambiguous_row_insertions() {
    let table = "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Role</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Bob</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CTO</w:t></w:r></w:p></w:tc></w:tr></w:tbl>";
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body>{table}{table}</w:body></w:document>"
    ));
    let operation = row_operation(&["Name", "Role"], "Bob", &["Charlie", "CFO"]);
    assert_eq!(
        row_execute(&input, &operation).unwrap_err().diagnostics[0].code,
        "TARGET_AMBIGUOUS"
    );
    let wrong = row_operation(&["Name", "Role"], "Bob", &["Charlie"]);
    let single = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body>{table}</w:body></w:document>"
    ));
    assert_eq!(
        row_execute(&single, &wrong).unwrap_err().diagnostics[0].code,
        "PRECONDITION_FAILED"
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(single).unwrap();
}

#[test]
fn inserted_rows_copy_the_nearest_body_formatting() {
    let rows = [
        styled_row("Header", "001F3F", true),
        styled_row("Body A", "FFFFFF", false),
        styled_row("Body B", "DDDDDD", false),
        styled_row("Final", "FFFF00", true),
    ];
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl>{}</w:tbl></w:body></w:document>",
        rows.join("")
    ));
    let original = table_row_xml(&fs::read(&input).unwrap());

    for (anchor_index, template, inserted_index) in [(1, 1, 2), (0, 1, 1), (2, 2, 3)] {
        let output = row_execute(&input, &row_operation_by_index(anchor_index, &["New"])).unwrap();
        let actual = table_row_xml(&output);
        assert_eq!(
            actual[inserted_index],
            original[template].replace(anchor_text(template), "New")
        );
        let mut expected = original.clone();
        expected.insert(inserted_index, actual[inserted_index].clone());
        assert_eq!(actual, expected);
    }

    let operation = InsertTableRowsAfter {
        table: TableTarget {
            header_cells: Vec::new(),
            occurrence: None,
            handle: Some("t0".to_owned()),
        },
        after: TableRowTarget {
            first_cell_text: String::new(),
            occurrence: None,
            handle: Some("t0:r2".to_owned()),
        },
        rows: vec![vec!["New 1".to_owned()], vec!["New 2".to_owned()]],
        base_revision: None,
    };
    let actual = table_row_xml(&rows_execute(&input, &operation).unwrap());
    let mut expected = original.clone();
    expected.insert(3, original[2].replace("Body B", "New 1"));
    expected.insert(4, original[2].replace("Body B", "New 2"));
    assert_eq!(actual, expected);
    fs::remove_file(input).unwrap();
}

#[test]
fn unsafe_anchor_uses_nearby_safe_row_and_header_only_table_uses_minimal_row() {
    let unsafe_row = "<w:tr><w:tc><w:p><w:r><w:t>Unsafe</w:t></w:r></w:p><w:p><w:r><w:t>second paragraph</w:t></w:r></w:p></w:tc></w:tr>";
    let header = styled_row("Header", "001F3F", true);
    let body_a = styled_row("Body A", "EEEEEE", false);
    let body_c = styled_row("Body C", "DDDDDD", false);
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl>{header}{body_a}{unsafe_row}{body_c}</w:tbl></w:body></w:document>"
    ));
    let original = table_row_xml(&fs::read(&input).unwrap());
    let actual = table_row_xml(&row_execute(&input, &row_operation_by_index(2, &["New"])).unwrap());
    assert_eq!(actual[3], original[3].replace("Body C", "New"));
    assert_eq!(actual[0..3], original[0..3]);
    assert_eq!(actual[4], original[3]);
    fs::remove_file(input).unwrap();

    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl>{header}</w:tbl></w:body></w:document>"
    ));
    let actual = table_row_xml(&row_execute(&input, &row_operation_by_index(0, &["New"])).unwrap());
    assert_eq!(actual[0], header);
    assert_eq!(
        actual[1],
        "<w:tr><w:tc><w:p><w:r><w:t>New</w:t></w:r></w:p></w:tc></w:tr>"
    );
    fs::remove_file(input).unwrap();
}

#[test]
fn deletes_semantic_rows_in_the_selected_table_and_keeps_legacy_targets() {
    let table = format!(
        "<w:tbl>{}</w:tbl>",
        [
            styled_row("Header", "001F3F", true),
            styled_row("Total", "EEEEEE", false),
            styled_row("Total", "DDDDDD", false),
        ]
        .join("")
    );
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body>{table}{table}</w:body></w:document>"
    ));
    let mut operation = DeleteTableRow {
        table: TableTarget {
            header_cells: vec!["Header".into()],
            occurrence: Some(1),
            handle: None,
        },
        row: DeleteTableRowTarget::Semantic(TableCellRow::Label {
            text: "Total".into(),
            occurrence: Some(1),
        }),
        base_revision: None,
    };
    let output = delete_row_execute(&input, &operation).unwrap();
    let package = Package::from_bytes(output).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    let rows = all_table_rows(&source).unwrap();
    assert_eq!(rows[0].len(), 3);
    assert_eq!(rows[1], vec![vec!["Header"], vec!["Total"]]);
    assert_eq!(entry(&input, "word/media/image.bin"), vec![1, 2, 3]);

    operation.row = DeleteTableRowTarget::Semantic(TableCellRow::Label {
        text: "Total".into(),
        occurrence: None,
    });
    let failed = delete_row_execute(&input, &operation).unwrap_err();
    assert_eq!(failed.diagnostics[0].code, "TARGET_AMBIGUOUS");
    assert_eq!(failed.diagnostics[0].candidate_count, Some(2));
    assert_eq!(
        failed.diagnostics[0].required_selector_kind.as_deref(),
        Some("rowOccurrence")
    );
    operation.table.occurrence = None;
    assert_eq!(
        delete_row_execute(&input, &operation)
            .unwrap_err()
            .diagnostics[0]
            .code,
        "TARGET_AMBIGUOUS"
    );
    operation.table.occurrence = Some(1);

    operation.row = DeleteTableRowTarget::Semantic(TableCellRow::Index {
        index: 2,
        expected_first_cell_text: "Wrong".into(),
    });
    let error = delete_row_execute(&input, &operation).unwrap_err();
    assert_eq!(
        error.diagnostics[0].reason_code.as_deref(),
        Some("EXPECTED_TEXT_MISMATCH")
    );
    operation.row = DeleteTableRowTarget::Semantic(TableCellRow::Index {
        index: 2,
        expected_first_cell_text: "Total".into(),
    });
    assert!(delete_row_execute(&input, &operation).is_ok());

    operation.row = DeleteTableRowTarget::Legacy(TableRowTarget {
        first_cell_text: "Total".into(),
        occurrence: Some(1),
        handle: None,
    });
    assert!(delete_row_execute(&input, &operation).is_ok());
    operation.row = DeleteTableRowTarget::Legacy(TableRowTarget {
        first_cell_text: String::new(),
        occurrence: None,
        handle: Some("t1:r2".into()),
    });
    assert!(delete_row_execute(&input, &operation).is_ok());
    operation.row = DeleteTableRowTarget::Semantic(TableCellRow::Header);
    assert!(delete_row_execute(&input, &operation).is_ok());
    fs::remove_file(input).unwrap();
}

#[test]
fn semantic_row_delete_keeps_last_row_and_unsafe_table_checks() {
    let row = styled_row("Header", "001F3F", true);
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl>{row}</w:tbl></w:body></w:document>"
    ));
    let operation = DeleteTableRow {
        table: TableTarget {
            header_cells: vec!["Header".into()],
            occurrence: None,
            handle: None,
        },
        row: DeleteTableRowTarget::Semantic(TableCellRow::Header),
        base_revision: None,
    };
    assert_eq!(
        delete_row_execute(&input, &operation)
            .unwrap_err()
            .diagnostics[0]
            .reason_code
            .as_deref(),
        Some("LAST_TABLE_ROW")
    );
    fs::remove_file(input).unwrap();

    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Header</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:gridSpan w:val=\"2\"/></w:tcPr><w:p><w:r><w:t>Total</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    ));
    assert_eq!(
        delete_row_execute(&input, &operation)
            .unwrap_err()
            .diagnostics[0]
            .code,
        "UNSUPPORTED_OPERATION"
    );
    let mut handle_operation = operation;
    handle_operation.row = DeleteTableRowTarget::Legacy(TableRowTarget {
        first_cell_text: String::new(),
        occurrence: None,
        handle: Some("t0:r0".into()),
    });
    assert_eq!(
        delete_row_execute(&input, &handle_operation)
            .unwrap_err()
            .diagnostics[0]
            .code,
        "UNSUPPORTED_OPERATION"
    );
    fs::remove_file(input).unwrap();
}

fn styled_row(text: &str, shading: &str, bold: bool) -> String {
    let run_properties = if bold { "<w:rPr><w:b/></w:rPr>" } else { "" };
    format!(
        "<w:tr><w:tc><w:tcPr><w:shd w:fill=\"{shading}\"/></w:tcPr><w:p><w:r>{run_properties}<w:t>{text}</w:t></w:r></w:p></w:tc></w:tr>"
    )
}

fn anchor_text(index: usize) -> &'static str {
    ["Header", "Body A", "Body B", "Final"][index]
}

fn row_operation_by_index(index: usize, cells: &[&str]) -> InsertTableRowAfter {
    let mut operation = row_operation(&["Header"], "", cells);
    operation.table.handle = Some("t0".to_owned());
    operation.after.handle = Some(format!("t0:r{index}"));
    operation
}

fn table_row_xml(bytes: &[u8]) -> Vec<String> {
    let package = Package::from_bytes(bytes.to_vec()).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    let (_, _, rows, _) = resolve_table(
        &source,
        &TableTarget {
            header_cells: Vec::new(),
            occurrence: None,
            handle: Some("t0".to_owned()),
        },
    )
    .unwrap();
    rows.iter()
        .map(|row| String::from_utf8(source_bytes(&source, row.source_id()).unwrap()).unwrap())
        .collect()
}

fn row_operation(headers: &[&str], after: &str, cells: &[&str]) -> InsertTableRowAfter {
    InsertTableRowAfter {
        table: TableTarget {
            header_cells: headers.iter().map(|value| (*value).to_owned()).collect(),
            occurrence: None,
            handle: None,
        },
        after: TableRowTarget {
            first_cell_text: after.to_owned(),
            occurrence: None,
            handle: None,
        },
        cells: cells.iter().map(|value| (*value).to_owned()).collect(),
        base_revision: Some("caller-version-7".to_owned()),
    }
}

fn row_execute(
    input: &std::path::Path,
    operation: &InsertTableRowAfter,
) -> Result<Vec<u8>, OperationResult> {
    let package = Package::open(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    insert_table_row_after_to_vec(&package, &main, &source, operation)
}

fn rows_execute(
    input: &std::path::Path,
    operation: &InsertTableRowsAfter,
) -> Result<Vec<u8>, OperationResult> {
    let package = Package::open(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    insert_table_rows_after_to_vec(&package, &main, &source, operation)
}

fn delete_row_execute(
    input: &std::path::Path,
    operation: &DeleteTableRow,
) -> Result<Vec<u8>, OperationResult> {
    let package = Package::open(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    delete_table_row_to_vec(&package, &main, &source, operation)
}
