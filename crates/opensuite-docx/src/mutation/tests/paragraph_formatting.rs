use super::super::*;
use super::support::*;

#[test]
fn sets_direct_paragraph_formatting_without_rewriting_unknown_properties() {
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr w:custom=\"keep\"><w:pStyle w:val=\"Body\"/><w:unknown w:value=\"stay\"/></w:pPr><w:r><w:t>Format me</w:t></w:r></w:p></w:body></w:document>"
    ));
    let output = path("formatting");
    let operation = SetParagraphFormatting {
        target: TextTarget {
            text: "Format me".to_owned(),
            occurrence: None,
        },
        formatting: ParagraphFormattingPatch {
            alignment: Some(PropertyPatch::Set(ParagraphAlignment::Center)),
            spacing_before_twips: Some(PropertyPatch::Set(120)),
            keep_with_next: Some(PropertyPatch::Set(true)),
            ..Default::default()
        },
        base_revision: None,
    };
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        set_paragraph_formatting(&package, &main, &source, &operation, &output).status,
        opensuite_protocol::OperationStatus::Applied
    );
    let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
    assert!(xml.contains("w:custom=\"keep\""));
    assert!(xml.contains("<w:unknown w:value=\"stay\"/>"));
    assert!(xml.contains("<w:jc w:val=\"center\"/>"));
    assert!(xml.contains("w:before=\"120\""));
    assert_eq!(
        entry(&input, "word/media/image.bin"),
        entry(&output, "word/media/image.bin")
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
}

#[test]
fn clears_direct_paragraph_formatting_and_rejects_table_paragraphs() {
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr><w:jc w:val=\"right\"/><w:spacing w:before=\"120\"/></w:pPr><w:r><w:t>Clear me</w:t></w:r></w:p></w:body></w:document>"
    ));
    let output = path("formatting-clear");
    let operation = SetParagraphFormatting {
        target: TextTarget {
            text: "Clear me".to_owned(),
            occurrence: None,
        },
        formatting: ParagraphFormattingPatch {
            alignment: Some(PropertyPatch::Clear),
            spacing_before_twips: Some(PropertyPatch::Clear),
            ..Default::default()
        },
        base_revision: None,
    };
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        set_paragraph_formatting(&package, &main, &source, &operation, &output).status,
        opensuite_protocol::OperationStatus::Applied
    );
    assert!(
        !String::from_utf8(entry(&output, "word/document.xml"))
            .unwrap()
            .contains("w:before=\"120\"")
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();

    let table = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>In table</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
    ));
    let package = Package::open(&table).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        set_paragraph_formatting(
            &package,
            &main,
            &source,
            &SetParagraphFormatting {
                target: TextTarget {
                    text: "In table".to_owned(),
                    occurrence: None
                },
                formatting: ParagraphFormattingPatch::default(),
                base_revision: None
            },
            path("formatting-table")
        )
        .diagnostics[0]
            .code,
        "UNSUPPORTED_OPERATION"
    );
    fs::remove_file(table).unwrap();
}

#[test]
fn sets_and_clears_existing_paragraph_styles_by_name_without_touching_styles_part() {
    let input = styled_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr w:unknown=\"keep\"><w:pStyle w:val=\"Normal\"/><w:jc w:val=\"center\"/><w:unknown/></w:pPr><w:r><w:t>Style me</w:t></w:r></w:p><w:p><w:r><w:t>Other</w:t></w:r></w:p></w:body></w:document>"
    ));
    let output = path("style-output");
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let operation = SetParagraphStyle {
        target: TextTarget {
            text: "Style me".to_owned(),
            occurrence: None,
        },
        style: PropertyPatch::Set("Heading 1".to_owned()),
        base_revision: None,
    };
    assert_eq!(
        set_paragraph_style(&package, &main, &source, &operation, &output).status,
        opensuite_protocol::OperationStatus::Applied
    );
    let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
    assert!(xml.contains("<w:pStyle w:val=\"HeadingOne\"/>"));
    assert!(xml.contains("<w:jc w:val=\"center\"/>"));
    assert!(xml.contains("<w:unknown/>"));
    assert_eq!(
        entry(&input, "word/styles.xml"),
        entry(&output, "word/styles.xml")
    );
    assert_eq!(
        entry(&input, "word/media/image.bin"),
        entry(&output, "word/media/image.bin")
    );
    let updated = Package::open(&output).unwrap();
    let (main, source) = crate::open_main_source(&updated).unwrap();
    let clear = SetParagraphStyle {
        target: TextTarget {
            text: "Style me".to_owned(),
            occurrence: None,
        },
        style: PropertyPatch::Clear,
        base_revision: None,
    };
    let cleared = path("style-cleared");
    assert_eq!(
        set_paragraph_style(&updated, &main, &source, &clear, &cleared).status,
        opensuite_protocol::OperationStatus::Applied
    );
    assert!(
        !String::from_utf8(entry(&cleared, "word/document.xml"))
            .unwrap()
            .contains("pStyle")
    );
    for (style, code) in [
        ("Emphasis", "UNSUPPORTED_OPERATION"),
        ("Missing", "TARGET_NOT_FOUND"),
    ] {
        let result = set_paragraph_style(
            &package,
            &main,
            &source,
            &SetParagraphStyle {
                target: TextTarget {
                    text: "Style me".to_owned(),
                    occurrence: None,
                },
                style: PropertyPatch::Set(style.to_owned()),
                base_revision: None,
            },
            path("style-rejected"),
        );
        assert_eq!(result.diagnostics[0].code, code);
    }
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
    fs::remove_file(cleared).unwrap();
}
