use super::*;
use opensuite_protocol::{OperationStatus, SetParagraphStyle, TextTarget};

fn input() -> Vec<u8> {
    crate::create_blank_docx()
}
fn create(id: &str, parent: &str) -> CreateStyle {
    CreateStyle {
        style_id: id.into(),
        style_type: WordStyleType::Paragraph,
        properties: WordStylePatch {
            name: Some(id.into()),
            based_on: Some(PropertyPatch::Set(parent.into())),
            ..Default::default()
        },
        base_revision: None,
    }
}
fn read(bytes: Vec<u8>) -> (Package, Part, SourceDocument) {
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    (package, main, source)
}
fn apply(bytes: Vec<u8>, op: &CreateStyle) -> Vec<u8> {
    let (p, m, s) = read(bytes);
    create_style_to_vec(&p, &m, &s, op).unwrap()
}
#[test]
fn creates_updates_applies_and_reopens_with_inheritance_and_preservation() {
    let original = input();
    let (p, m, _) = read(original.clone());
    let before = crate::load_styles(&p, &m).unwrap().unwrap();
    let mut op = create("OpenSuiteReportHeading", "Heading1");
    op.properties.next = Some(PropertyPatch::Set("Normal".into()));
    op.properties.run = TextFormattingPatch {
        font_family: Some(PropertyPatch::Set("Arial".into())),
        font_size_half_points: Some(PropertyPatch::Set(32)),
        bold: Some(PropertyPatch::Set(true)),
        color: Some(PropertyPatch::Set("124733".into())),
        ..Default::default()
    };
    op.properties.paragraph = ParagraphFormattingPatch {
        spacing_before_twips: Some(PropertyPatch::Set(240)),
        spacing_after_twips: Some(PropertyPatch::Set(120)),
        keep_with_next: Some(PropertyPatch::Set(true)),
        ..Default::default()
    };
    let created = apply(original, &op);
    let (p, m, s) = read(created);
    let changed = update_style_to_vec(
        &p,
        &m,
        &s,
        &UpdateStyle {
            style_id: op.style_id.clone(),
            style_type: op.style_type,
            properties: WordStylePatch {
                run: TextFormattingPatch {
                    color: Some(PropertyPatch::Set("235744".into())),
                    bold: Some(PropertyPatch::Clear),
                    ..Default::default()
                },
                ..Default::default()
            },
            base_revision: None,
        },
    )
    .unwrap();
    let inserted = crate::execute_docx_insert_paragraph(
        changed,
        &opensuite_protocol::InsertParagraph {
            text: "Report heading".into(),
            placement: opensuite_protocol::ParagraphPlacement::End,
            base_revision: None,
        },
    );
    assert_eq!(inserted.operation.status, OperationStatus::Applied);
    let applied = crate::execute_docx_set_paragraph_style(
        inserted.output_artifact.unwrap(),
        &SetParagraphStyle {
            target: TextTarget {
                text: "Report heading".into(),
                occurrence: None,
            },
            style: PropertyPatch::Set(op.style_id.clone()),
            base_revision: None,
        },
    );
    assert_eq!(applied.operation.status, OperationStatus::Applied);
    let (p, m, s) = read(applied.output_artifact.unwrap());
    let styles = crate::load_styles(&p, &m).unwrap().unwrap();
    let style = styles.style(&StyleId::new(op.style_id)).unwrap();
    assert_eq!(style.next().unwrap().as_str(), "Normal");
    assert_eq!(style.run_formatting().bold, None);
    let doc = crate::DocxDocument::new(&s).unwrap();
    let paragraph = doc
        .paragraphs()
        .find(|p| p.text().unwrap() == "Report heading")
        .unwrap();
    assert_eq!(paragraph.style_id().unwrap(), style.id().clone());
    let effective = styles
        .effective_run_formatting(Some(style.id()), None, &crate::RunFormatting::default())
        .unwrap();
    assert_eq!(effective.bold, Some(true)); // inherited after Clear
    assert_eq!(effective.font_size_half_points, Some(32));
    assert_eq!(effective.color.as_deref(), Some("235744"));
    assert_eq!(
        paragraph
            .effective_formatting(&styles)
            .unwrap()
            .spacing_after_twips,
        Some(120)
    );
    for old in before.styles() {
        let a = before.source().node(old.source_id()).unwrap().span();
        let b = styles
            .source()
            .node(styles.style(old.id()).unwrap().source_id())
            .unwrap()
            .span();
        assert_eq!(
            &before.source().original_bytes()[a.start..a.end],
            &styles.source().original_bytes()[b.start..b.end]
        );
    }
}
#[test]
fn rejects_duplicate_missing_parent_type_next_and_indirect_cycle_without_output() {
    let first = apply(input(), &create("A", "Normal"));
    for (op, code) in [
        (create("A", "Normal"), "DUPLICATE_STYLE_ID"),
        (create("B", "Missing"), "MISSING_STYLE"),
    ] {
        let result = crate::execute_docx_create_style(first.clone(), &op);
        assert!(result.output_artifact.is_none());
        assert_eq!(result.operation.diagnostics[0].code, code);
    }
    let second = apply(first, &create("B", "A"));
    let third = apply(second, &create("C", "B"));
    let update = UpdateStyle {
        style_id: "A".into(),
        style_type: WordStyleType::Paragraph,
        properties: WordStylePatch {
            based_on: Some(PropertyPatch::Set("C".into())),
            ..Default::default()
        },
        base_revision: None,
    };
    let result = crate::execute_docx_update_style(third.clone(), &update);
    assert!(result.output_artifact.is_none());
    assert_eq!(
        result.operation.diagnostics[0].code,
        "STYLE_INHERITANCE_CYCLE"
    );
    let mut op = create("D", "Normal");
    op.properties.next = Some(PropertyPatch::Set("Missing".into()));
    assert_eq!(
        crate::execute_docx_create_style(third.clone(), &op)
            .operation
            .diagnostics[0]
            .code,
        "INVALID_STYLE_NEXT"
    );
    op.properties.next = None;
    op.properties.name = Some("normal".into());
    assert_eq!(
        crate::execute_docx_create_style(third.clone(), &op)
            .operation
            .diagnostics[0]
            .code,
        "DUPLICATE_STYLE_NAME"
    );
    op.properties.name = Some("D".into());
    op.style_type = WordStyleType::Character;
    assert_eq!(
        crate::execute_docx_create_style(third, &op)
            .operation
            .diagnostics[0]
            .code,
        "STYLE_TYPE_MISMATCH"
    );
}
#[test]
fn character_styles_reuse_run_formatting_and_empty_source_properties() {
    let mut op = create("Accent", "Normal");
    op.style_type = WordStyleType::Character;
    op.properties.based_on = None;
    let first = apply(input(), &op);
    let (p, m, s) = read(first);
    let part = crate::styles::styles_part(&p, &m).unwrap().unwrap();
    let raw = String::from_utf8(p.read_part(&part).unwrap())
        .unwrap()
        .replace(
            "<w:name w:val=\"Accent\"/>",
            "<w:name w:val=\"Accent\"/><w:rPr/>",
        );
    let bytes = p.write_replaced_part_to_vec(&part, raw.as_bytes()).unwrap();
    let (p, m, _) = read(bytes);
    let changed = update_style_to_vec(
        &p,
        &m,
        &s,
        &UpdateStyle {
            style_id: "Accent".into(),
            style_type: WordStyleType::Character,
            properties: WordStylePatch {
                run: TextFormattingPatch {
                    bold: Some(PropertyPatch::Set(true)),
                    italic: Some(PropertyPatch::Set(true)),
                    ..Default::default()
                },
                ..Default::default()
            },
            base_revision: None,
        },
    )
    .unwrap();
    let (p, m, _) = read(changed);
    let styles = crate::load_styles(&p, &m).unwrap().unwrap();
    let formatting = styles
        .style_run_formatting(&StyleId::new("Accent".into()), StyleType::Character)
        .unwrap();
    assert_eq!(formatting.bold, Some(true));
    assert_eq!(formatting.italic, Some(true));
}

#[test]
fn updates_imported_properties_preserving_unknown_content_and_quotes() {
    let (p, m, _) = read(input());
    let part = crate::styles::styles_part(&p, &m).unwrap().unwrap();
    let raw = String::from_utf8(p.read_part(&part).unwrap()).unwrap()
        .replace("<w:styles ", "<w:styles xmlns:x=\"urn:unknown\" ")
        .replace("<w:rPr><w:b/><w:sz w:val=\"32\"/></w:rPr>", "<w:rPr><w:rFonts w:ascii='Theme old' w:asciiTheme='majorHAnsi' w:hAnsiTheme='majorHAnsi' x:font='keep'/><w:b x:bold='keep'/><w:sz w:val='32'/><x:extra/></w:rPr>")
        .replace("<w:spacing w:before=\"240\" w:after=\"120\"/>", "<w:spacing w:before = '240' w:after='120' x:spacing='keep'/>");
    let bytes = p.write_replaced_part_to_vec(&part, raw.as_bytes()).unwrap();
    let (p, m, s) = read(bytes);
    let output = update_style_to_vec(
        &p,
        &m,
        &s,
        &UpdateStyle {
            style_id: "Heading1".into(),
            style_type: WordStyleType::Paragraph,
            properties: WordStylePatch {
                run: TextFormattingPatch {
                    font_family: Some(PropertyPatch::Set("Arial".into())),
                    ..Default::default()
                },
                paragraph: ParagraphFormattingPatch {
                    spacing_before_twips: Some(PropertyPatch::Clear),
                    spacing_after_twips: Some(PropertyPatch::Set(160)),
                    ..Default::default()
                },
                ..Default::default()
            },
            base_revision: None,
        },
    )
    .unwrap();
    let (p, m, _) = read(output);
    let styles = crate::load_styles(&p, &m).unwrap().unwrap();
    let style = styles.style(&StyleId::new("Heading1".into())).unwrap();
    assert_eq!(style.run_formatting().font_family.as_deref(), Some("Arial"));
    assert_eq!(style.paragraph_formatting().spacing_before_twips, None);
    assert_eq!(style.paragraph_formatting().spacing_after_twips, Some(160));
    let xml = String::from_utf8(styles.source().original_bytes().to_vec()).unwrap();
    for value in [
        "x:font='keep'",
        "x:spacing='keep'",
        "<w:b x:bold='keep'/>",
        "<x:extra/>",
    ] {
        assert!(xml.contains(value));
    }
    assert!(!xml.contains("asciiTheme"));
    assert!(!xml.contains("hAnsiTheme"));
}
