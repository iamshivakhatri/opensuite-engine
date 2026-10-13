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

#[test]
fn header_footer_edits_refuse_pending_property_revisions() {
    use opensuite_opc::PartName;
    for kind in [HeaderFooterKind::Header, HeaderFooterKind::Footer] {
        let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let bytes = set_header_footer_text_to_vec(
            &package,
            &main,
            &source,
            &SetHeaderFooterText {
                kind,
                text: Some("Review".into()),
                base_revision: None,
            },
        )
        .unwrap();
        let package = Package::from_bytes(bytes).unwrap();
        let name = PartName::parse(match kind {
            HeaderFooterKind::Header => "/word/header1.xml",
            HeaderFooterKind::Footer => "/word/footer1.xml",
        })
        .unwrap();
        let xml = String::from_utf8(package.read_part_by_name(&name).unwrap()).unwrap()
            .replace("<w:r>", r#"<w:r><w:rPr><w:rPrChange w:id="1" w:author="Lee"><w:rPr/></w:rPrChange></w:rPr>"#);
        let bytes = package
            .write_package_with_named_changes_to_vec(&[(name, xml.as_bytes())], &[])
            .unwrap();
        let fields = crate::inspect_docx_fields(bytes.clone(), 0, 20);
        assert!(
            fields
                .diagnostics
                .iter()
                .any(|d| d.code == "FIELD_PART_HAS_REVISIONS")
        );
        for text in [None, Some("Updated".into())] {
            let result = crate::execute_docx_set_header_footer_text(
                bytes.clone(),
                &SetHeaderFooterText {
                    kind,
                    text,
                    base_revision: None,
                },
            );
            assert!(result.output_artifact.is_none());
            assert_eq!(
                result.operation.diagnostics[0].code,
                "UNSUPPORTED_OPERATION"
            );
        }
    }
}
