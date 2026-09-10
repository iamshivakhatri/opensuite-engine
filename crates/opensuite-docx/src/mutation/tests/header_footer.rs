use super::super::*;

#[test]
fn creates_replaces_and_clears_independent_default_header_footer_text() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let header = SetHeaderFooterText {
        kind: HeaderFooterKind::Header,
        text: Some("ACME Corporation".to_owned()),
        base_revision: None,
    };
    let bytes = set_header_footer_text_to_vec(&package, &main, &source, &header).unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        inspect_header_footer(&package, &main, &source, HeaderFooterKind::Header).unwrap(),
        HeaderFooterInspection::SimpleText("ACME Corporation".to_owned())
    );
    let footer = SetHeaderFooterText {
        kind: HeaderFooterKind::Footer,
        text: Some("Confidential".to_owned()),
        base_revision: None,
    };
    let bytes = set_header_footer_text_to_vec(&package, &main, &source, &footer).unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        inspect_header_footer(&package, &main, &source, HeaderFooterKind::Header).unwrap(),
        HeaderFooterInspection::SimpleText("ACME Corporation".to_owned())
    );
    assert_eq!(
        inspect_header_footer(&package, &main, &source, HeaderFooterKind::Footer).unwrap(),
        HeaderFooterInspection::SimpleText("Confidential".to_owned())
    );
    let clear = SetHeaderFooterText {
        kind: HeaderFooterKind::Header,
        text: None,
        base_revision: None,
    };
    let bytes = set_header_footer_text_to_vec(&package, &main, &source, &clear).unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        inspect_header_footer(&package, &main, &source, HeaderFooterKind::Header).unwrap(),
        HeaderFooterInspection::None
    );
    assert_eq!(
        inspect_header_footer(&package, &main, &source, HeaderFooterKind::Footer).unwrap(),
        HeaderFooterInspection::SimpleText("Confidential".to_owned())
    );
}
