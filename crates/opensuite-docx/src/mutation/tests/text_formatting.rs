use super::super::*;
use super::support::*;

#[test]
fn sets_text_formatting_without_rewriting_text_or_unknown_run_properties() {
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:rPr><w:unknown w:value=\"stay\"/><w:rFonts w:eastAsia=\"Keep\"/></w:rPr><w:t>Format run</w:t></w:r></w:p></w:body></w:document>"
    ));
    let output = path("text-formatting");
    let operation = SetTextFormatting {
        target: TextTarget {
            text: "Format run".to_owned(),
            occurrence: None,
        },
        formatting: TextFormattingPatch {
            bold: Some(PropertyPatch::Set(true)),
            italic: Some(PropertyPatch::Set(false)),
            font_size_half_points: Some(PropertyPatch::Set(28)),
            font_family: Some(PropertyPatch::Set("Aptos".to_owned())),
            ..TextFormattingPatch::default()
        },
        base_revision: None,
    };
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let result = set_text_formatting(&package, &main, &source, &operation, &output);
    assert_eq!(
        result.status,
        opensuite_protocol::OperationStatus::Applied,
        "{result:?}"
    );
    let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
    assert!(xml.contains("<w:t>Format run</w:t>"));
    assert!(xml.contains("<w:b />"));
    assert!(xml.contains("<w:i w:val=\"0\"/>"));
    assert!(xml.contains("w:sz w:val=\"28\""));
    assert!(xml.contains("w:eastAsia=\"Keep\""));
    assert!(xml.contains("w:ascii=\"Aptos\""));
    assert!(xml.contains("<w:unknown w:value=\"stay\"/>"));
    assert_eq!(
        entry(&input, "word/media/image.bin"),
        entry(&output, "word/media/image.bin")
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
}

#[test]
fn rejects_partial_or_cross_run_text_formatting_and_clears_direct_properties() {
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:rPr><w:b/><w:i/><w:sz w:val=\"24\"/><w:rFonts w:ascii=\"Aptos\" w:eastAsia=\"Keep\"/></w:rPr><w:t>Whole</w:t></w:r><w:r><w:t> run</w:t></w:r></w:p></w:body></w:document>"
    ));
    let output = path("text-formatting-clear");
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let clear = SetTextFormatting {
        target: TextTarget {
            text: "Whole".to_owned(),
            occurrence: None,
        },
        formatting: TextFormattingPatch {
            bold: Some(PropertyPatch::Clear),
            italic: Some(PropertyPatch::Clear),
            font_size_half_points: Some(PropertyPatch::Clear),
            font_family: Some(PropertyPatch::Clear),
            ..TextFormattingPatch::default()
        },
        base_revision: None,
    };
    let result = set_text_formatting(&package, &main, &source, &clear, &output);
    assert_eq!(
        result.status,
        opensuite_protocol::OperationStatus::Applied,
        "{result:?}"
    );
    let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
    assert!(!xml.contains("<w:b/>"));
    assert!(!xml.contains("<w:i/>"));
    assert!(!xml.contains("w:sz"));
    assert!(xml.contains("w:eastAsia=\"Keep\""));
    fs::remove_file(output).unwrap();
    fs::remove_file(input).unwrap();
}

#[test]
fn rejects_wrapped_or_tracked_text_formatting() {
    for body in [
        "<w:p><w:hyperlink><w:r><w:t>Unsafe</w:t></w:r></w:hyperlink></w:p>",
        "<w:p><w:ins><w:r><w:t>Unsafe</w:t></w:r></w:ins></w:p>",
        "<w:sdt><w:sdtPr><w:tag w:val=\"x\"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Unsafe</w:t></w:r></w:p></w:sdtContent></w:sdt>",
    ] {
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body>{body}</w:body></w:document>"
        ));
        let package = Package::open(&input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let result = set_text_formatting(
            &package,
            &main,
            &source,
            &SetTextFormatting {
                target: TextTarget {
                    text: "Unsafe".to_owned(),
                    occurrence: None,
                },
                formatting: TextFormattingPatch {
                    bold: Some(PropertyPatch::Set(true)),
                    ..Default::default()
                },
                base_revision: None,
            },
            path("text-formatting-wrapper"),
        );
        assert_eq!(result.diagnostics[0].code, "UNSUPPORTED_OPERATION");
        fs::remove_file(input).unwrap();
    }
}

#[test]
fn sets_and_clears_professional_run_properties() {
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:rPr><w:b/><w:i w:val=\"0\"/><w:sz w:val=\"24\"/><w:rFonts w:ascii=\"Aptos\"/><w:unknown w:x=\"keep\"/></w:rPr><w:t>Styled</w:t></w:r></w:p></w:body></w:document>"
    ));
    let output = path("text-formatting-v2");
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let operation = SetTextFormatting {
        target: TextTarget {
            text: "Styled".into(),
            occurrence: None,
        },
        formatting: TextFormattingPatch {
            color: Some(PropertyPatch::Set("0057B8".into())),
            underline: Some(PropertyPatch::Set(true)),
            highlight: Some(PropertyPatch::Set("yellow".into())),
            strikethrough: Some(PropertyPatch::Set(true)),
            vertical_alignment: Some(PropertyPatch::Set(
                opensuite_protocol::VerticalAlignment::Superscript,
            )),
            ..Default::default()
        },
        base_revision: None,
    };
    assert_eq!(
        set_text_formatting(&package, &main, &source, &operation, &output).status,
        opensuite_protocol::OperationStatus::Applied
    );
    let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
    for value in [
        "w:color w:val=\"0057B8\"",
        "<w:u />",
        "w:highlight w:val=\"yellow\"",
        "<w:strike />",
        "w:vertAlign w:val=\"superscript\"",
        "<w:b/>",
        "w:i w:val=\"0\"",
        "w:sz w:val=\"24\"",
        "w:ascii=\"Aptos\"",
        "w:unknown w:x=\"keep\"",
    ] {
        assert!(xml.contains(value), "{value}");
    }
    let package = Package::open(&output).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let cleared = path("text-formatting-v2-cleared");
    let operation = SetTextFormatting {
        target: TextTarget {
            text: "Styled".into(),
            occurrence: None,
        },
        formatting: TextFormattingPatch {
            color: Some(PropertyPatch::Clear),
            underline: Some(PropertyPatch::Clear),
            highlight: Some(PropertyPatch::Clear),
            strikethrough: Some(PropertyPatch::Clear),
            vertical_alignment: Some(PropertyPatch::Clear),
            ..Default::default()
        },
        base_revision: None,
    };
    assert_eq!(
        set_text_formatting(&package, &main, &source, &operation, &cleared).status,
        opensuite_protocol::OperationStatus::Applied
    );
    let xml = String::from_utf8(entry(&cleared, "word/document.xml")).unwrap();
    for value in [
        "<w:color",
        "<w:u ",
        "<w:highlight",
        "<w:strike",
        "<w:vertAlign",
    ] {
        assert!(!xml.contains(value), "{value}");
    }
    assert!(xml.contains("w:unknown w:x=\"keep\""));
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
    fs::remove_file(cleared).unwrap();
}

#[test]
fn formats_every_run_and_returns_to_baseline() {
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>Multi </w:t></w:r><w:r><w:t>run</w:t></w:r></w:p></w:body></w:document>"
    ));
    let output = path("text-formatting-multi-run");
    let operation = |vertical_alignment| SetTextFormatting {
        target: TextTarget {
            text: "Multi run".into(),
            occurrence: None,
        },
        formatting: TextFormattingPatch {
            vertical_alignment: Some(PropertyPatch::Set(vertical_alignment)),
            underline: Some(PropertyPatch::Set(true)),
            ..Default::default()
        },
        base_revision: None,
    };
    let package = Package::open(&input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        set_text_formatting(
            &package,
            &main,
            &source,
            &operation(opensuite_protocol::VerticalAlignment::Superscript),
            &output,
        )
        .status,
        opensuite_protocol::OperationStatus::Applied
    );
    let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
    assert_eq!(xml.matches("w:vertAlign w:val=\"superscript\"").count(), 2);
    let subscript = path("text-formatting-subscript");
    let package = Package::open(&output).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        set_text_formatting(
            &package,
            &main,
            &source,
            &operation(opensuite_protocol::VerticalAlignment::Subscript),
            &subscript,
        )
        .status,
        opensuite_protocol::OperationStatus::Applied
    );
    let baseline = path("text-formatting-baseline");
    let package = Package::open(&subscript).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        set_text_formatting(
            &package,
            &main,
            &source,
            &operation(opensuite_protocol::VerticalAlignment::Baseline),
            &baseline,
        )
        .status,
        opensuite_protocol::OperationStatus::Applied
    );
    let xml = String::from_utf8(entry(&baseline, "word/document.xml")).unwrap();
    assert_eq!(xml.matches("w:vertAlign w:val=\"baseline\"").count(), 2);
    for file in [input, output, subscript, baseline] {
        fs::remove_file(file).unwrap();
    }
}

#[test]
fn rejects_invalid_text_property_values() {
    for formatting in [
        TextFormattingPatch {
            color: Some(PropertyPatch::Set("not-a-color".into())),
            ..Default::default()
        },
        TextFormattingPatch {
            highlight: Some(PropertyPatch::Set("FFF2CC".into())),
            ..Default::default()
        },
    ] {
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>Invalid</w:t></w:r></w:p></w:body></w:document>"
        ));
        let package = Package::open(&input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let result = set_text_formatting(
            &package,
            &main,
            &source,
            &SetTextFormatting {
                target: TextTarget {
                    text: "Invalid".into(),
                    occurrence: None,
                },
                formatting,
                base_revision: None,
            },
            path("text-formatting-invalid"),
        );
        assert_eq!(result.diagnostics[0].code, "INVALID_OPERATION");
        fs::remove_file(input).unwrap();
    }
}
