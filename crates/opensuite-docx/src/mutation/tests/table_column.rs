use super::super::*;
use super::support::*;

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
