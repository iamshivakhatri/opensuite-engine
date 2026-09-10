use super::super::*;

#[test]
fn applies_and_clears_a_hyperlink_without_touching_existing_links() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/references.docx");
    let package = Package::open(fixture).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let operation = SetHyperlink {
        target: TextTarget {
            text: "Visit ".to_owned(),
            occurrence: None,
        },
        url: Some("https://example.com/visit".to_owned()),
        base_revision: None,
    };
    let bytes = set_hyperlink_to_vec(&package, &main, &source, &operation).unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let links = crate::DocxDocument::new(&source)
        .unwrap()
        .hyperlinks()
        .collect::<Vec<_>>();
    assert_eq!(links.len(), 3);
    assert_eq!(links[0].text().unwrap(), "Visit ");
    assert_eq!(
        links[0].target(&package, &main).unwrap(),
        Some(crate::HyperlinkTarget::External(
            "https://example.com/visit".to_owned()
        ))
    );
    assert_eq!(links[1].text().unwrap(), "OpenSuite");
    let clear = SetHyperlink {
        target: operation.target,
        url: None,
        base_revision: None,
    };
    let bytes = set_hyperlink_to_vec(&package, &main, &source, &clear).unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (_main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        crate::DocxDocument::new(&source)
            .unwrap()
            .hyperlinks()
            .count(),
        2
    );
    assert_eq!(
        crate::DocxDocument::new(&source)
            .unwrap()
            .paragraphs()
            .next()
            .unwrap()
            .text()
            .unwrap(),
        "Visit OpenSuite and Revenue"
    );
}
