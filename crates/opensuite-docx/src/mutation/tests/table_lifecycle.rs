use super::super::*;

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
