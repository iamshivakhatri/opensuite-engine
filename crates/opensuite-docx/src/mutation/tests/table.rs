use super::super::*;
use super::support::*;

#[test]
fn creates_a_readable_table_with_positive_grid_widths() {
    let input = crate::create_blank_docx();
    let package = Package::from_bytes(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let output = create_table_to_vec(
        &package,
        &main,
        &source,
        &CreateTable {
            rows: vec![
                vec!["A".to_owned(), "B".to_owned()],
                vec!["1".to_owned(), "2".to_owned()],
            ],
            placement: ParagraphPlacement::End,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(output).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    let xml = String::from_utf8(source.original_bytes().to_vec()).unwrap();
    assert!(xml.contains("<w:tblBorders>"));
    assert!(xml.contains("<w:tblCellMar>"));
    assert!(xml.contains("w:w=\"9360\""));
    assert!(xml.contains("w:before=\"0\" w:after=\"0\""));
    assert!(!xml.contains("w:w:w") && !xml.contains("w:wbefore"));
    assert!(!xml.contains("w:gridCol w:w=\"0\""));
    assert_eq!(
        all_table_rows(&source).unwrap(),
        vec![vec![
            vec!["A".to_owned(), "B".to_owned()],
            vec!["1".to_owned(), "2".to_owned()]
        ]]
    );
}

#[test]
fn sets_one_simple_table_cell_and_preserves_other_payloads() {
    let xml = format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>100</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Draft</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Expenses</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>50</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Draft</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    );
    let input = table_fixture(&xml);
    let output = path("table-output");
    let result = table_execute(
        &input,
        &output,
        &table_operation("Revenue", "Amount", "100", "125 & < >"),
    );
    assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
    assert_eq!(result.changes[0].kind, "table_cell_text_set");
    assert!(!result.to_json().to_string().contains("NodeId"));
    assert_eq!(
        entry(&input, "word/media/image.bin"),
        entry(&output, "word/media/image.bin")
    );
    let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
    assert!(xml.contains("<w:t>125 &amp; &lt; &gt;</w:t>"));
    assert!(xml.contains("<w:t>Expenses</w:t>"));
    Package::open(&output).unwrap().verify().unwrap();
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
}

#[test]
fn sets_multiple_table_cells_atomically() {
    let xml = format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Role</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Team</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Alice</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CEO</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Product</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Bob</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CTO</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Platform</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    );
    let input = table_fixture(&xml);
    let update = |row: &str, column: &str, expected: &str, replacement: &str| TableCellTextUpdate {
        target: TableCellTarget {
            row_label: row.to_owned(),
            column_header: column.to_owned(),
            occurrence: None,
            handle: None,
        },
        expected_current_text: expected.to_owned(),
        replacement: replacement.to_owned(),
    };
    let operation = SetTableCellsText {
        table: TableTarget {
            header_cells: vec!["Name".to_owned(), "Role".to_owned(), "Team".to_owned()],
            occurrence: None,
            handle: None,
        },
        updates: vec![
            update("Alice", "Role", "CEO", "Founder & CEO"),
            update("Bob", "Role", "CTO", "CTO & VP Engineering"),
            update("Bob", "Team", "Platform", "Engineering"),
        ],
        base_revision: None,
    };
    let output = cells_execute(&input, &operation).unwrap();
    let package = Package::from_bytes(output).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(all_table_rows(&source).unwrap()[0][1][1], "Founder & CEO");
    assert_eq!(
        all_table_rows(&source).unwrap()[0][2][1..],
        ["CTO & VP Engineering", "Engineering"]
    );
    let mut bad = operation.clone();
    bad.updates[2].expected_current_text = "wrong".to_owned();
    assert_eq!(
        cells_execute(&input, &bad).unwrap_err().diagnostics[0].code,
        "PRECONDITION_FAILED"
    );
    let duplicate = SetTableCellsText {
        updates: vec![operation.updates[0].clone(), operation.updates[0].clone()],
        ..operation
    };
    assert_eq!(
        cells_execute(&input, &duplicate).unwrap_err().diagnostics[0].code,
        "PRECONDITION_FAILED"
    );
    fs::remove_file(input).unwrap();
}

#[test]
fn fills_a_blank_trailing_row_by_inspected_cell_handles() {
    let xml = format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Executive Role</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Meeting Access Level</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>CFO</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Full access</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>CHRO</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Limited access</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr></w:tbl></w:body></w:document>"
    );
    let input = table_fixture(&xml);
    let operation = SetTableCellsText {
        table: TableTarget {
            header_cells: Vec::new(),
            occurrence: None,
            handle: Some("t0".to_owned()),
        },
        updates: vec![
            TableCellTextUpdate {
                target: TableCellTarget {
                    row_label: String::new(),
                    column_header: String::new(),
                    occurrence: None,
                    handle: Some("t0:r3:c0".to_owned()),
                },
                expected_current_text: String::new(),
                replacement: "Guest Panelist".to_owned(),
            },
            TableCellTextUpdate {
                target: TableCellTarget {
                    row_label: String::new(),
                    column_header: String::new(),
                    occurrence: None,
                    handle: Some("t0:r3:c1".to_owned()),
                },
                expected_current_text: String::new(),
                replacement: "Invited — selected meetings".to_owned(),
            },
        ],
        base_revision: None,
    };
    let output = cells_execute(&input, &operation).unwrap();
    let package = Package::from_bytes(output).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        all_table_rows(&source).unwrap()[0][3],
        ["Guest Panelist", "Invited — selected meetings"]
    );
    fs::remove_file(input).unwrap();
}

#[test]
fn inserts_a_grid_aware_table_column() {
    let xml = format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tblGrid><w:gridCol w:w=\"2400\"/><w:gridCol w:w=\"3600\"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w=\"2400\" w:type=\"dxa\"/></w:tcPr><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w=\"3600\" w:type=\"dxa\"/></w:tcPr><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Role</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:tcW w:w=\"2400\" w:type=\"dxa\"/></w:tcPr><w:p><w:r><w:t>Alice</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w=\"3600\" w:type=\"dxa\"/></w:tcPr><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>CEO</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:tcW w:w=\"2400\" w:type=\"dxa\"/></w:tcPr><w:p><w:r><w:t>Bob</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w=\"3600\" w:type=\"dxa\"/></w:tcPr><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>CTO</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    );
    let input = table_fixture(&xml);
    let operation = InsertTableColumnAfter {
        table: TableTarget {
            header_cells: vec!["Name".to_owned(), "Role".to_owned()],
            occurrence: None,
            handle: None,
        },
        after_column_header: "Role".to_owned(),
        after_column_handle: Some("t0:c1".to_owned()),
        header: "Location".to_owned(),
        cells: vec!["New York".to_owned(), String::new()],
        base_revision: None,
    };
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let output = insert_table_column_after_to_vec(&package, &main, &source, &operation).unwrap();
    let package = Package::from_bytes(output.clone()).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        all_table_rows(&source).unwrap()[0],
        vec![
            vec!["Name", "Role", "Location"],
            vec!["Alice", "CEO", "New York"],
            vec!["Bob", "CTO", ""]
        ]
    );
    let xml = String::from_utf8(
        package
            .read_part(&package.main_office_document().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(xml.matches("<w:gridCol").count(), 3);
    assert!(xml.contains("<w:rPr><w:b/></w:rPr><w:t>Location</w:t>"));
    fs::remove_file(input).unwrap();
}

#[test]
fn inserts_a_column_in_the_google_docs_table_without_rewriting_its_grid_change() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/google-docs-table.docx");
    let input = google_docs_table_fixture().to_vec();
    let inspection = crate::inspect_docx(
        input.clone(),
        &InspectDocx {
            focus: InspectDocxFocus::Tables {
                offset: 0,
                limit: 10,
            },
        },
    );
    let Some(InspectDocxContent::Tables(tables)) = inspection.content else {
        panic!("expected table inspection")
    };
    let table = &tables.items[0];
    assert_eq!(table.rows[0].cells, ["Name", "     Year"]);
    assert_eq!(table.rows[1].cells, ["OpenSuite ", "2026"]);
    assert_eq!(table.handle, "t0");
    assert_eq!(table.columns[1].handle, "t0:c1");
    assert_eq!(table.rows[1].cell_handles[1], "t0:r1:c1");

    let package = Package::from_bytes(input).unwrap();
    let before_payloads = payloads(&fixture);
    let (main, source) = crate::open_main_source(&package).unwrap();
    let operation = InsertTableColumnAfter {
        table: TableTarget {
            header_cells: Vec::new(),
            occurrence: None,
            handle: Some(table.handle.clone()),
        },
        after_column_header: String::new(),
        after_column_handle: Some(table.columns[1].handle.clone()),
        header: "Status".to_owned(),
        cells: vec!["Draft".to_owned()],
        base_revision: None,
    };
    let output = insert_table_column_after_to_vec(&package, &main, &source, &operation).unwrap();
    let output_path = path("google-docs-table-column-output");
    fs::write(&output_path, &output).unwrap();
    let output_package = Package::from_bytes(output).unwrap();
    let mut after_payloads = payloads(&output_path);
    let mut before_payloads = before_payloads;
    before_payloads.remove("word/document.xml");
    after_payloads.remove("word/document.xml");
    assert_eq!(after_payloads, before_payloads);

    let (_, output_source) = crate::open_main_source(&output_package).unwrap();
    assert_eq!(
        all_table_rows(&output_source).unwrap()[0],
        vec![
            vec!["Name", "     Year", "Status"],
            vec!["OpenSuite ", "2026", "Draft"],
        ]
    );
    let output_xml = String::from_utf8(output_package.read_part(&main).unwrap()).unwrap();
    assert!(output_xml.contains("<w:tblGridChange w:id=\"0\"><w:tblGrid><w:gridCol w:w=\"4680\"/><w:gridCol w:w=\"4680\"/></w:tblGrid></w:tblGridChange>"));
    assert!(output_xml.contains("<w:tblGrid><w:gridCol w:w=\"4680\"/><w:gridCol w:w=\"4680\"/><w:gridCol w:w=\"4680\"/><w:tblGridChange"));
    fs::remove_file(output_path).unwrap();
}

#[test]
fn fills_an_empty_simple_cell_and_rejects_ambiguous_or_complex_cells() {
    let empty = format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:p/></w:tc></w:tr></w:tbl></w:body></w:document>"
    );
    let input = table_fixture(&empty);
    let output = path("table-empty");
    let result = table_execute(
        &input,
        &output,
        &table_operation("Revenue", "Amount", "", " kept "),
    );
    assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
    assert!(
        String::from_utf8(entry(&output, "word/document.xml"))
            .unwrap()
            .contains("<w:p><w:r><w:t xml:space=\"preserve\"> kept </w:t></w:r></w:p>")
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();

    for xml in [
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=\"DATE\"><w:r><w:t>100</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
        ),
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>100</w:t></w:r></w:p><w:p><w:r><w:t>more</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
        ),
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:gridSpan w:val=\"2\"/></w:tcPr><w:p><w:r><w:t>100</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
        ),
    ] {
        let input = table_fixture(&xml);
        let output = path("table-unsupported");
        assert_eq!(
            table_execute(
                &input,
                &output,
                &table_operation("Revenue", "Amount", "100", "125")
            )
            .diagnostics[0]
                .code,
            "UNSUPPORTED_OPERATION"
        );
        fs::remove_file(input).unwrap();
    }
}

#[test]
fn table_targets_require_occurrence_and_current_text_preconditions() {
    let table = "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>100</w:t></w:r></w:p></w:tc></w:tr></w:tbl>";
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body>{table}{table}</w:body></w:document>"
    ));
    let output = path("table-ambiguous");
    let operation = table_operation("Revenue", "Amount", "100", "125");
    assert_eq!(
        table_execute(&input, &output, &operation).diagnostics[0].code,
        "TARGET_AMBIGUOUS"
    );
    let mut selected = operation.clone();
    selected.target.occurrence = Some(1);
    assert_eq!(
        table_execute(&input, &output, &selected).status,
        opensuite_protocol::OperationStatus::Applied
    );
    fs::remove_file(&output).unwrap();
    let mut wrong = table_operation("Revenue", "Amount", "wrong", "125");
    wrong.target.occurrence = Some(0);
    assert_eq!(
        table_execute(&input, &output, &wrong).diagnostics[0].code,
        "PRECONDITION_FAILED"
    );
    assert!(!output.exists());
    fs::remove_file(input).unwrap();
}
