use super::super::*;
use super::support::*;

#[test]
fn formats_simple_table_header_text_without_changing_cells() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = create_table_to_vec(
        &package,
        &main,
        &source,
        &CreateTable {
            rows: vec![
                vec!["Option".into(), "Scope".into()],
                vec!["A".into(), "Pilot".into()],
            ],
            placement: ParagraphPlacement::End,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = set_text_formatting_to_vec(
        &package,
        &main,
        &source,
        &SetTextFormatting {
            target: TextTarget {
                text: "Option".into(),
                occurrence: None,
            },
            formatting: TextFormattingPatch {
                bold: Some(PropertyPatch::Set(true)),
                ..Default::default()
            },
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    package.verify().unwrap();
    let xml = String::from_utf8(package.read_part(&main).unwrap()).unwrap();
    assert!(xml.contains("<w:b />"));
    assert!(xml.contains("<w:t>Option</w:t>"));
    assert!(xml.contains("<w:t>Pilot</w:t>"));
}

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

#[test]
fn text_formatting_does_not_duplicate_pending_property_revisions() {
    let input = table_fixture(&format!(
        r#"<w:document xmlns:w="{WORD}"><w:body><w:p><w:r><w:rPr><w:rPrChange w:id="1" w:author="Lee"><w:rPr><w:i/></w:rPr></w:rPrChange></w:rPr><w:t>Old wording</w:t></w:r></w:p></w:body></w:document>"#
    ));
    for text in ["Old wording", "wording"] {
        let result = crate::execute_docx_set_text_formatting(
            fs::read(&input).unwrap(),
            &SetTextFormatting {
                target: TextTarget {
                    text: text.into(),
                    occurrence: None,
                },
                formatting: TextFormattingPatch {
                    bold: Some(PropertyPatch::Set(true)),
                    ..Default::default()
                },
                base_revision: None,
            },
        );
        assert!(result.output_artifact.is_none());
        assert_eq!(
            result.operation.diagnostics[0].code,
            "UNSUPPORTED_OPERATION"
        );
    }
    fs::remove_file(input).unwrap();
}

fn character_style_fixture(body: &str) -> Vec<u8> {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, _) = crate::open_main_source(&package).unwrap();
    let bytes = package
        .write_replaced_part_to_vec(
            &main,
            format!("<w:document xmlns:w=\"{WORD}\"><w:body>{body}</w:body></w:document>")
                .as_bytes(),
        )
        .unwrap();
    crate::execute_docx_create_style(
        bytes,
        &opensuite_protocol::CreateStyle {
            style_id: "Accent".into(),
            style_type: opensuite_protocol::WordStyleType::Character,
            properties: opensuite_protocol::WordStylePatch {
                name: Some("Accent display name".into()),
                run: TextFormattingPatch {
                    color: Some(PropertyPatch::Set("124733".into())),
                    italic: Some(PropertyPatch::Set(true)),
                    ..Default::default()
                },
                ..Default::default()
            },
            base_revision: None,
        },
    )
    .output_artifact
    .unwrap()
}
fn character_style_operation(text: &str) -> SetTextFormatting {
    SetTextFormatting {
        target: TextTarget {
            text: text.into(),
            occurrence: None,
        },
        formatting: TextFormattingPatch {
            character_style_id: Some(PropertyPatch::Set("Accent".into())),
            ..Default::default()
        },
        base_revision: None,
    }
}
fn main_xml(bytes: &[u8]) -> String {
    let p = Package::from_bytes(bytes.to_vec()).unwrap();
    String::from_utf8(p.read_part(&p.main_office_document().unwrap()).unwrap()).unwrap()
}

#[test]
fn applies_and_removes_existing_character_style_across_partial_runs_with_package_preservation() {
    let original = character_style_fixture(
        "<w:p><w:r><w:rPr><w:b/><w:lang w:val=\"en-US\"/></w:rPr><w:t>Before selected</w:t></w:r><w:r><w:rPr><w:u/></w:rPr><w:t> text after</w:t></w:r></w:p>",
    );
    let op = character_style_operation("selected text");
    let output = crate::execute_docx_set_text_formatting(original.clone(), &op)
        .output_artifact
        .unwrap();
    let xml = main_xml(&output);
    assert_eq!(xml.matches("<w:rStyle w:val=\"Accent\"/>").count(), 2);
    assert!(xml.contains("<w:rStyle w:val=\"Accent\"/><w:b/><w:lang w:val=\"en-US\"/>"));
    assert!(xml.contains("<w:rStyle w:val=\"Accent\"/><w:u/>"));
    let p = Package::from_bytes(original.clone()).unwrap();
    let q = Package::from_bytes(output.clone()).unwrap();
    for name in p
        .part_names_with_prefix("/")
        .filter(|n| n.as_str() != "/word/document.xml")
    {
        assert_eq!(
            p.read_part_by_name(name).unwrap(),
            q.read_part_by_name(name).unwrap()
        );
    }
    let mut clear = op;
    clear.formatting.character_style_id = Some(PropertyPatch::Clear);
    let cleared = crate::execute_docx_set_text_formatting(output, &clear)
        .output_artifact
        .unwrap();
    assert!(!main_xml(&cleared).contains("<w:rStyle"));
    assert_eq!(
        crate::text_search::resolve_text(
            &crate::open_main_source(&Package::from_bytes(cleared.clone()).unwrap())
                .unwrap()
                .1,
            "Before selected text after"
        )
        .unwrap()
        .len(),
        1
    );
    assert!(main_xml(&cleared).contains("<w:b/><w:lang w:val=\"en-US\"/>"));
}

#[test]
fn character_style_validates_id_type_and_ambiguous_targets_without_output() {
    let bytes = character_style_fixture(
        "<w:p><w:r><w:t>Same</w:t></w:r></w:p><w:p><w:r><w:t>Same</w:t></w:r></w:p>",
    );
    let mut op = character_style_operation("Same");
    let ambiguous = crate::execute_docx_set_text_formatting(bytes.clone(), &op);
    assert!(ambiguous.output_artifact.is_none());
    assert_eq!(ambiguous.operation.diagnostics[0].code, "TARGET_AMBIGUOUS");
    op.target.occurrence = Some(1);
    for (id, code) in [
        ("Missing", "MISSING_STYLE"),
        ("Normal", "STYLE_TYPE_MISMATCH"),
    ] {
        op.formatting.character_style_id = Some(PropertyPatch::Set(id.into()));
        let result = crate::execute_docx_set_text_formatting(bytes.clone(), &op);
        assert!(result.output_artifact.is_none());
        assert_eq!(result.operation.diagnostics[0].code, code);
    }
    op.formatting.character_style_id = Some(PropertyPatch::Set("Accent".into()));
    assert!(
        crate::execute_docx_set_text_formatting(bytes, &op)
            .output_artifact
            .is_some()
    );
}

#[test]
fn character_style_preserves_imported_references_and_refuses_protected_or_ambiguous_run_xml() {
    let body = "<w:p><w:r><w:rPr><w:rStyle w:val=\"ImportedUnknown\"/><w:b/></w:rPr><w:t>Text</w:t></w:r></w:p>";
    let original = character_style_fixture(body);
    let mut op = character_style_operation("Text");
    op.formatting.character_style_id = Some(PropertyPatch::Clear);
    let cleared = crate::execute_docx_set_text_formatting(original, &op)
        .output_artifact
        .unwrap();
    assert!(main_xml(&cleared).contains("<w:rPr><w:b/></w:rPr>"));
    for body in [
        "<w:p><w:r><w:rPr><w:rStyle/><w:rStyle/></w:rPr><w:t>Text</w:t></w:r></w:p>",
        "<w:p><w:r><w:rPr/><w:rPr/><w:t>Text</w:t></w:r></w:p>",
        "<w:p><w:r><w:rPr><w:rStyle w:val=\"Accent\" extra=\"keep\"/></w:rPr><w:t>Text</w:t></w:r></w:p>",
        "<w:p><w:r><w:rPr><w:rPrChange/></w:rPr><w:t>Text</w:t></w:r></w:p>",
        "<w:bookmarkStart w:id=\"1\"/><w:p><w:r><w:t>Text</w:t></w:r></w:p><w:bookmarkEnd w:id=\"1\"/>",
        "<w:p><w:hyperlink><w:r><w:t>Text</w:t></w:r></w:hyperlink></w:p>",
        "<w:p><w:ins w:id=\"1\"><w:r><w:t>Text</w:t></w:r></w:ins></w:p>",
        "<w:tbl><w:tr><w:tc><w:tcPr><w:hMerge/></w:tcPr><w:p><w:r><w:t>Text</w:t></w:r></w:p></w:tc></w:tr></w:tbl>",
    ] {
        let bytes = character_style_fixture(body);
        for value in [PropertyPatch::Clear, PropertyPatch::Set("Accent".into())] {
            let mut op = character_style_operation("Text");
            op.formatting.character_style_id = Some(value);
            assert!(
                crate::execute_docx_set_text_formatting(bytes.clone(), &op)
                    .output_artifact
                    .is_none(),
                "{body}"
            );
        }
    }
}

#[test]
fn character_style_whole_runs_supports_alternate_word_prefix_and_simple_cells() {
    for body in [
        "<w:p><w:r><w:rPr/><w:t>Text</w:t></w:r></w:p>",
        "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Text</w:t></w:r></w:p></w:tc></w:tr></w:tbl>",
    ] {
        let bytes = character_style_fixture(body);
        let p = Package::from_bytes(bytes).unwrap();
        let (main, source) = crate::open_main_source(&p).unwrap();
        let xml = String::from_utf8(source.original_bytes().to_vec())
            .unwrap()
            .replace("w:", "x:")
            .replace("xmlns:w=", "xmlns:x=");
        let bytes = p.write_replaced_part_to_vec(&main, xml.as_bytes()).unwrap();
        let output =
            crate::execute_docx_set_text_formatting(bytes, &character_style_operation("Text"))
                .output_artifact
                .unwrap();
        assert!(main_xml(&output).contains("<x:rStyle x:val=\"Accent\"/>"));
    }
}
