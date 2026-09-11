use super::super::*;
use super::support::*;

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
