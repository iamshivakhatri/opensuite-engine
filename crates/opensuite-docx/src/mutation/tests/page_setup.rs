use super::super::*;
use super::support::*;

#[test]
fn page_setup_updates_one_terminal_section_without_touching_body_or_margin_extras() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = insert_paragraph_to_vec(
        &package,
        &main,
        &source,
        &InsertParagraph {
            text: "body stays here".to_owned(),
            placement: ParagraphPlacement::End,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = set_page_setup_to_vec(
        &package,
        &main,
        &source,
        &SetPageSetup {
            margins: Some(PageMargins {
                top_twips: Some(720),
                right_twips: None,
                bottom_twips: None,
                left_twips: None,
            }),
            paper_size: Some(PaperSize::A4),
            orientation: Some(PageOrientation::Landscape),
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    let setup = inspect_page_setup(&source).unwrap();
    assert_eq!(setup.paper_size, Some(PaperSize::A4));
    assert_eq!(setup.orientation, PageOrientation::Landscape);
    assert_eq!(setup.margins.top_twips, Some(720));
    assert_eq!(setup.margins.right_twips, Some(1440));
    assert_eq!(
        body_texts(&source)
            .unwrap()
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>(),
        ["body stays here"]
    );
    let xml = String::from_utf8(source.original_bytes().to_vec()).unwrap();
    assert!(xml.contains("w:header=\"720\" w:footer=\"720\" w:gutter=\"0\""));
}

#[test]
fn page_setup_rejects_multiple_sections() {
    let source = SourceDocument::parse(
        format!("<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr><w:sectPr/></w:pPr></w:p><w:sectPr/></w:body></w:document>").into_bytes(),
    )
    .unwrap();
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let main = package.main_office_document().unwrap();
    let result = set_page_setup_to_vec(
        &package,
        &main,
        &source,
        &SetPageSetup {
            margins: None,
            paper_size: Some(PaperSize::Letter),
            orientation: None,
            base_revision: None,
        },
    );
    assert_eq!(
        result.unwrap_err().status,
        opensuite_protocol::OperationStatus::Failed
    );
}
