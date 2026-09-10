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
