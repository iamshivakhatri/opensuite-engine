use super::super::*;
use super::support::*;

#[test]
fn page_numbers_coexist_with_simple_header_footer_text_and_update_in_place() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = set_header_footer_text_to_vec(
        &package,
        &main,
        &source,
        &SetHeaderFooterText {
            kind: HeaderFooterKind::Footer,
            text: Some("Confidential".to_owned()),
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = set_page_number_to_vec(
        &package,
        &main,
        &source,
        &SetPageNumber {
            kind: HeaderFooterKind::Footer,
            alignment: Some(PageNumberAlignment::Center),
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        inspect_header_footer(&package, &main, &source, HeaderFooterKind::Footer).unwrap(),
        HeaderFooterInspection::SimpleTextAndPageNumber(
            "Confidential".to_owned(),
            PageNumberAlignment::Center
        )
    );
    let bytes = set_header_footer_text_to_vec(
        &package,
        &main,
        &source,
        &SetHeaderFooterText {
            kind: HeaderFooterKind::Footer,
            text: Some("Internal".to_owned()),
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = set_page_number_to_vec(
        &package,
        &main,
        &source,
        &SetPageNumber {
            kind: HeaderFooterKind::Footer,
            alignment: Some(PageNumberAlignment::Right),
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        inspect_page_number(&package, &main, &source).unwrap(),
        PageNumberInspection::Footer(PageNumberAlignment::Right)
    );
    let bytes = set_page_number_to_vec(
        &package,
        &main,
        &source,
        &SetPageNumber {
            kind: HeaderFooterKind::Footer,
            alignment: None,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        inspect_header_footer(&package, &main, &source, HeaderFooterKind::Footer).unwrap(),
        HeaderFooterInspection::SimpleText("Internal".to_owned())
    );
}

#[test]
fn creates_header_page_number_and_rejects_unfamiliar_fields() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = set_page_number_to_vec(
        &package,
        &main,
        &source,
        &SetPageNumber {
            kind: HeaderFooterKind::Header,
            alignment: Some(PageNumberAlignment::Right),
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        inspect_page_number(&package, &main, &source).unwrap(),
        PageNumberInspection::Header(PageNumberAlignment::Right)
    );
    let header = package
        .part(&PartName::parse("/word/header1.xml").unwrap())
        .unwrap();
    let invalid = SourceDocument::parse(format!(r#"<w:hdr xmlns:w="{WORD}"><w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:fldSimple w:instr=" NUMPAGES "/></w:p></w:hdr>"#).into_bytes()).unwrap();
    assert!(simple_header_footer_content(&invalid, HeaderFooterKind::Header).is_none());
    assert!(
        package
            .read_part(&header)
            .unwrap()
            .windows(b" PAGE ".len())
            .any(|value| value == b" PAGE ")
    );
}
