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
