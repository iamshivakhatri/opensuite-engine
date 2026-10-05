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
fn unsafe_table_deletion_refuses_without_output() {
    let xml = format!(
        r#"<w:document xmlns:w="{WORD}"><w:body><w:p><w:commentRangeStart w:id="7"/><w:r><w:t>Before</w:t></w:r></w:p><w:tbl><w:tblGrid><w:gridCol w:w="3000"/><w:gridCol w:w="3000"/></w:tblGrid><w:tr><w:tc><w:p><w:commentRangeEnd w:id="7"/><w:r><w:commentReference w:id="7"/></w:r><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>one</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>two</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#
    );
    let input = table_fixture(&xml);
    let bytes = fs::read(&input).unwrap();
    let table = TableTarget {
        header_cells: Vec::new(),
        occurrence: None,
        handle: Some("t0".to_owned()),
    };
    let result = crate::execute_docx_delete_table(
        bytes.clone(),
        &DeleteTable {
            table,
            base_revision: None,
        },
    );
    assert_eq!(
        result.operation.status,
        opensuite_protocol::OperationStatus::Failed
    );
    assert!(result.output_artifact.is_none());
    assert_eq!(
        result.operation.diagnostics[0].reason_code.as_deref(),
        Some("UNSUPPORTED_STRUCTURAL_DELETE")
    );
    assert_eq!(fs::read(&input).unwrap(), bytes);
    fs::remove_file(input).unwrap();
}
