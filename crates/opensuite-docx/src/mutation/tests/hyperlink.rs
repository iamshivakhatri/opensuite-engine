use super::super::*;

fn reference_document(body: &str) -> Vec<u8> {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, _) = crate::open_main_source(&package).unwrap();
    package.write_replaced_part_to_vec(&main,format!("<w:document xmlns:w=\"{}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><w:body>{body}</w:body></w:document>",NS[0]).as_bytes()).unwrap()
}
fn internal_link(input: Vec<u8>, text: &str, name: &str) -> crate::DocxExecutionResult {
    crate::execute_docx_set_hyperlink(
        input,
        &SetHyperlink {
            target: TextTarget {
                text: text.into(),
                occurrence: None,
            },
            url: None,
            bookmark_name: Some(name.into()),
            base_revision: None,
        },
    )
}

#[test]
fn internal_links_preserve_partial_multirun_formatting_and_all_unrelated_parts() {
    let body = "<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Before link</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t> text after</w:t></w:r></w:p><w:p><w:bookmarkStart w:id='5' w:name='Destination'/><w:r><w:t>Heading</w:t></w:r><w:bookmarkEnd w:id='5'/></w:p>";
    let input = reference_document(body);
    let output = internal_link(input.clone(), "link text", "Destination")
        .output_artifact
        .unwrap();
    let original = Package::from_bytes(input).unwrap();
    let changed = Package::from_bytes(output.clone()).unwrap();
    let (main, source) = crate::open_main_source(&changed).unwrap();
    let xml = std::str::from_utf8(source.original_bytes()).unwrap();
    assert!(xml.contains("<w:hyperlink w:anchor=\"Destination\"><w:r><w:rPr><w:b/></w:rPr><w:t>link</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t xml:space=\"preserve\"> text</w:t></w:r></w:hyperlink>"),"{xml}");
    assert!(xml.contains("<w:bookmarkStart w:id='5' w:name='Destination'/>"));
    assert_eq!(
        crate::DocxDocument::new(&source)
            .unwrap()
            .hyperlinks()
            .next()
            .unwrap()
            .target(&changed, &main)
            .unwrap(),
        Some(crate::HyperlinkTarget::InternalAnchor("Destination".into()))
    );
    for part in original
        .part_names_with_prefix("/")
        .filter(|part| **part != main.name)
    {
        assert_eq!(
            original.read_part_by_name(part).unwrap(),
            changed.read_part_by_name(part).unwrap()
        );
    }
    assert_eq!(crate::inspect_docx_references(output, 0, 100).total, 2);
}

#[test]
fn internal_destinations_refuse_missing_duplicate_and_malformed_bookmarks() {
    let text = "<w:p><w:r><w:t>Link</w:t></w:r></w:p>";
    for marks in [
        "",
        "<w:bookmarkStart w:id='1' w:name='Mark'/>",
        "<w:bookmarkStart w:id='1' w:name='Mark'/><w:bookmarkEnd w:id='1'/><w:bookmarkStart w:id='2' w:name='Mark'/><w:bookmarkEnd w:id='2'/>",
        "<w:bookmarkStart w:id='1' w:name='Mark'/><w:bookmarkEnd w:id='1'/><w:bookmarkStart w:id='1' w:name='Other'/><w:bookmarkEnd w:id='1'/>",
    ] {
        let input = reference_document(&format!("{text}<w:p>{marks}</w:p>"));
        assert!(
            internal_link(input, "Link", "Mark")
                .output_artifact
                .is_none()
        );
    }
}

#[test]
fn hyperlink_authoring_refuses_enclosing_and_hidden_protected_structures() {
    let marks = "<w:p><w:bookmarkStart w:id='9' w:name='Mark'/><w:bookmarkEnd w:id='9'/></w:p>";
    for middle in [
        "<w:bookmarkStart w:id='1' w:name='Other'/><w:bookmarkEnd w:id='1'/>",
        "<w:commentRangeStart w:id='1'/><w:commentRangeEnd w:id='1'/>",
        "<w:ins w:id='1'/>",
        "<w:r><w:fldChar w:fldCharType='begin'/><w:instrText>PAGE</w:instrText><w:fldChar w:fldCharType='end'/></w:r>",
    ] {
        let input = reference_document(&format!(
            "<w:p><w:r><w:t>One</w:t></w:r>{middle}<w:r><w:t>Two</w:t></w:r></w:p>{marks}"
        ));
        assert!(
            internal_link(input, "OneTwo", "Mark")
                .output_artifact
                .is_none(),
            "{middle}"
        );
    }
    let input = reference_document(&format!(
        "<w:p><w:bookmarkStart w:id='1' w:name='Other'/><w:r><w:t>Link</w:t></w:r><w:bookmarkEnd w:id='1'/></w:p>{marks}"
    ));
    assert!(
        internal_link(input, "Link", "Mark")
            .output_artifact
            .is_none()
    );
}

#[test]
fn existing_hyperlink_conversion_preserves_tooltips_attributes_and_inner_xml() {
    let input = reference_document(
        "<w:p><w:hyperlink w:anchor='Old' w:tooltip='Keep me' w:history='1' xmlns:x='urn:keep' x:data='keep'><w:r><w:rPr><w:b/></w:rPr><w:t>Link</w:t></w:r></w:hyperlink></w:p><w:p><w:bookmarkStart w:id='1' w:name='Mark'/><w:bookmarkEnd w:id='1'/></w:p>",
    );
    let output = internal_link(input, "Link", "Mark")
        .output_artifact
        .unwrap();
    let mut bytes = output;
    for url in [
        Some("https://example.com/one"),
        Some("https://example.com/two"),
    ] {
        bytes = crate::execute_docx_set_hyperlink(
            bytes,
            &SetHyperlink {
                target: TextTarget {
                    text: "Link".into(),
                    occurrence: None,
                },
                url: url.map(str::to_owned),
                bookmark_name: None,
                base_revision: None,
            },
        )
        .output_artifact
        .unwrap();
    }
    let package = Package::from_bytes(bytes).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    let xml = std::str::from_utf8(source.original_bytes()).unwrap();
    assert!(xml.contains("w:tooltip='Keep me'"));
    assert!(xml.contains("x:data='keep'"));
    assert!(xml.contains("<w:r><w:rPr><w:b/></w:rPr><w:t>Link</w:t></w:r>"));
}

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
        bookmark_name: None,
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
    let replace = SetHyperlink {
        target: operation.target.clone(),
        url: Some("https://example.com/updated".to_owned()),
        bookmark_name: None,
        base_revision: None,
    };
    let bytes = set_hyperlink_to_vec(&package, &main, &source, &replace).unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        crate::DocxDocument::new(&source)
            .unwrap()
            .hyperlinks()
            .next()
            .unwrap()
            .target(&package, &main)
            .unwrap(),
        Some(crate::HyperlinkTarget::External(
            "https://example.com/updated".to_owned()
        ))
    );
    let clear = SetHyperlink {
        target: replace.target,
        url: None,
        bookmark_name: None,
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
