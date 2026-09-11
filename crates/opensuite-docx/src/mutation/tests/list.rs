use super::super::*;
use super::support::*;

#[test]
fn authors_one_shared_decimal_list_and_can_clear_it() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = insert_paragraphs_to_vec(
        &package,
        &main,
        &source,
        &InsertParagraphs {
            texts: vec![
                "one".to_owned(),
                "two".to_owned(),
                "three".to_owned(),
                "four".to_owned(),
            ],
            placement: ParagraphPlacement::Start,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (_main, source) = crate::open_main_source(&package).unwrap();
    let targets = ["one", "two", "three"]
        .into_iter()
        .map(|text| TextTarget {
            text: text.to_owned(),
            occurrence: None,
        })
        .collect::<Vec<_>>();
    assert!(
        set_paragraphs_list_to_vec(
            &package,
            &main,
            &source,
            &SetParagraphsList {
                targets: vec![targets[0].clone(), targets[2].clone()],
                kind: ParagraphListKind::Bullet,
                level: 0,
                continue_from_previous: false,
                base_revision: None,
            },
        )
        .is_err()
    );
    let bytes = set_paragraphs_list_to_vec(
        &package,
        &main,
        &source,
        &SetParagraphsList {
            targets: targets.clone(),
            kind: ParagraphListKind::Decimal,
            level: 0,
            continue_from_previous: false,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    package.verify().unwrap();
    let (main, _source) = crate::open_main_source(&package).unwrap();
    let xml = String::from_utf8(package.read_part(&main).unwrap()).unwrap();
    assert_eq!(xml.matches("<w:numId w:val=\"1\"/>").count(), 3);
    let relationships = String::from_utf8(
        package
            .read_part_by_name(&PartName::parse("/word/_rels/document.xml.rels").unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        relationships.matches(NUMBERING_RELATIONSHIP_TYPE).count(),
        1
    );
    let content_types = String::from_utf8(
        package
            .read_part_by_name(&PartName::parse("/[Content_Types].xml").unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(content_types.matches("/word/numbering.xml").count(), 1);
    let numbering = crate::load_numbering(&package, &main).unwrap().unwrap();
    assert_eq!(numbering.instance_count(), 1);
    assert_eq!(
        numbering
            .resolve(crate::ListReference {
                num_id: crate::NumberingId(1),
                level: 0
            })
            .unwrap()
            .format,
        crate::NumberFormat::Decimal
    );
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = set_paragraphs_list_to_vec(
        &package,
        &main,
        &source,
        &SetParagraphsList {
            targets,
            kind: ParagraphListKind::None,
            level: 0,
            continue_from_previous: false,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    package.verify().unwrap();
    let (main, _) = crate::open_main_source(&package).unwrap();
    let xml = String::from_utf8(package.read_part(&main).unwrap()).unwrap();
    assert!(!xml.contains("<w:numPr>"));
    assert!(
        package
            .part(&PartName::parse("/word/numbering.xml").unwrap())
            .is_ok()
    );
}

#[test]
fn preserves_imported_numbering_while_allocating_and_clearing_lists() {
    let input = imported_numbering_fixture();
    let inspected = crate::inspect_docx(
        input.clone(),
        &opensuite_protocol::InspectDocx {
            focus: opensuite_protocol::InspectDocxFocus::Paragraphs {
                offset: 0,
                limit: 20,
            },
        },
    );
    let Some(opensuite_protocol::InspectDocxContent::Paragraphs(page)) = inspected.content else {
        panic!("paragraph inspection expected")
    };
    let list = page
        .items
        .iter()
        .find(|paragraph| paragraph.text == "Existing")
        .and_then(|paragraph| paragraph.list.as_ref())
        .unwrap();
    assert_eq!(list.kind, "unknown");
    assert_eq!(list.level, 0);
    assert!(!list.supported);
    let package = Package::from_bytes(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let numbering_before = package
        .read_part_by_name(&PartName::parse("/word/numbering.xml").unwrap())
        .unwrap();
    let relationships_before = package
        .read_part_by_name(&PartName::parse("/word/_rels/document.xml.rels").unwrap())
        .unwrap();
    let content_types_before = package
        .read_part_by_name(&PartName::parse("/[Content_Types].xml").unwrap())
        .unwrap();
    let list = |kind, targets: &[&str]| SetParagraphsList {
        targets: targets
            .iter()
            .map(|text| TextTarget {
                text: (*text).to_owned(),
                occurrence: None,
            })
            .collect(),
        kind,
        level: 0,
        continue_from_previous: false,
        base_revision: None,
    };
    let bytes = set_paragraphs_list_to_vec(
        &package,
        &main,
        &source,
        &list(ParagraphListKind::Bullet, &["First", "Second"]),
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    package.verify().unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let numbering_after = package
        .read_part_by_name(&PartName::parse("/word/numbering.xml").unwrap())
        .unwrap();
    assert!(numbering_after.starts_with(&numbering_before[..numbering_before.len() - 14]));
    let numbering = String::from_utf8(numbering_after).unwrap();
    assert!(numbering.contains(r#"<w:abstractNum w:abstractNumId="78">"#));
    assert!(numbering.contains(r#"<w:num w:numId="43">"#));
    assert!(numbering.contains("<w:legacy w:legacy=\"preserve-me\"/>"));
    assert_eq!(
        package
            .read_part_by_name(&PartName::parse("/word/_rels/document.xml.rels").unwrap())
            .unwrap(),
        relationships_before
    );
    assert_eq!(
        package
            .read_part_by_name(&PartName::parse("/[Content_Types].xml").unwrap())
            .unwrap(),
        content_types_before
    );
    let document = String::from_utf8(package.read_part(&main).unwrap()).unwrap();
    assert_eq!(document.matches(r#"<w:numId w:val="43"/>"#).count(), 2);
    assert!(document.contains(r#"<w:pStyle w:val="HeadingOne"/><w:keepNext/><w:jc w:val="center"/><w:spacing w:before="120" w:after="240"/><w:ind w:left="360"/>"#));
    let imported_numbering = crate::load_numbering(&package, &main).unwrap().unwrap();
    let unknown = imported_numbering
        .resolve(crate::ListReference {
            num_id: crate::NumberingId(42),
            level: 0,
        })
        .unwrap();
    assert!(matches!(unknown.format, crate::NumberFormat::UpperRoman));

    let bytes = set_paragraphs_list_to_vec(
        &package,
        &main,
        &source,
        &list(ParagraphListKind::Decimal, &["Existing"]),
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let document = String::from_utf8(package.read_part(&main).unwrap()).unwrap();
    assert!(document.contains(r#"<w:numId w:val="44"/>"#));
    assert!(document.contains(r#"<w:numId w:val="43"/>"#));
    let numbering_before_clear = package
        .read_part_by_name(&PartName::parse("/word/numbering.xml").unwrap())
        .unwrap();

    let bytes = set_paragraphs_list_to_vec(
        &package,
        &main,
        &source,
        &list(ParagraphListKind::None, &["Existing"]),
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let document = String::from_utf8(package.read_part(&main).unwrap()).unwrap();
    assert!(!document.contains(r#"<w:numId w:val="42"/>"#));
    assert!(document.contains(r#"<w:numId w:val="43"/>"#));
    assert_eq!(
        package
            .read_part_by_name(&PartName::parse("/word/numbering.xml").unwrap())
            .unwrap(),
        numbering_before_clear
    );

    let output = path("rejected-list");
    let result = set_paragraphs_list(
        &package,
        &main,
        &source,
        &list(ParagraphListKind::Bullet, &["First", "Existing"]),
        &output,
    );
    assert_eq!(result.status, opensuite_protocol::OperationStatus::Failed);
    assert!(!output.exists());
}

fn apply_list(
    bytes: Vec<u8>,
    texts: &[&str],
    kind: ParagraphListKind,
    level: u8,
    continue_from_previous: bool,
) -> Result<Vec<u8>, opensuite_protocol::OperationResult> {
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    set_paragraphs_list_to_vec(
        &package,
        &main,
        &source,
        &SetParagraphsList {
            targets: texts
                .iter()
                .map(|text| TextTarget {
                    text: (*text).to_owned(),
                    occurrence: None,
                })
                .collect(),
            kind,
            level,
            continue_from_previous,
            base_revision: None,
        },
    )
}

#[test]
fn authors_multilevel_lists_with_continuation_restart_clear_and_inspection() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = insert_paragraphs_to_vec(
        &package,
        &main,
        &source,
        &InsertParagraphs {
            texts: [
                "Root action",
                "Second action",
                "Third action",
                "Following action",
                "Bulleted root",
                "Bulleted branch",
                "Restart",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            placement: ParagraphPlacement::Start,
            base_revision: None,
        },
    )
    .unwrap();
    let bytes = apply_list(
        bytes,
        &["Root action"],
        ParagraphListKind::Decimal,
        0,
        false,
    )
    .unwrap();
    let bytes = apply_list(
        bytes,
        &["Second action"],
        ParagraphListKind::Decimal,
        1,
        true,
    )
    .unwrap();
    let bytes = apply_list(
        bytes,
        &["Third action"],
        ParagraphListKind::Decimal,
        2,
        true,
    )
    .unwrap();
    let bytes = apply_list(
        bytes,
        &["Following action"],
        ParagraphListKind::Decimal,
        0,
        true,
    )
    .unwrap();
    let bytes = apply_list(
        bytes,
        &["Bulleted root"],
        ParagraphListKind::Bullet,
        0,
        false,
    )
    .unwrap();
    let bytes = apply_list(
        bytes,
        &["Bulleted branch"],
        ParagraphListKind::Bullet,
        1,
        true,
    )
    .unwrap();
    let bytes = apply_list(bytes, &["Restart"], ParagraphListKind::Decimal, 0, false).unwrap();

    let package = Package::from_bytes(bytes.clone()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let numbering = crate::load_numbering(&package, &main).unwrap().unwrap();
    let reference = |text: &str| {
        let paragraph = resolve_paragraph_anchor(
            &source,
            &TextTarget {
                text: text.to_owned(),
                occurrence: None,
            },
        )
        .unwrap()
        .1;
        let ppr = source
            .children(paragraph)
            .find(|id| word(&source, *id, "pPr"))
            .unwrap();
        crate::numbering::list_reference(&source, ppr)
            .unwrap()
            .unwrap()
    };
    let top = reference("Root action");
    assert_eq!(
        reference("Second action"),
        crate::ListReference {
            num_id: top.num_id,
            level: 1
        }
    );
    assert_eq!(
        reference("Third action"),
        crate::ListReference {
            num_id: top.num_id,
            level: 2
        }
    );
    assert_eq!(
        reference("Following action"),
        crate::ListReference {
            num_id: top.num_id,
            level: 0
        }
    );
    assert_ne!(reference("Restart").num_id, top.num_id);
    assert_eq!(
        numbering
            .resolve(reference("Bulleted root"))
            .unwrap()
            .format,
        crate::NumberFormat::Bullet
    );
    assert_eq!(
        numbering
            .resolve(reference("Bulleted branch"))
            .unwrap()
            .level,
        1
    );
    assert_eq!(numbering.resolve(top).unwrap().text.as_deref(), Some("%1."));
    assert_eq!(
        numbering
            .resolve(reference("Second action"))
            .unwrap()
            .text
            .as_deref(),
        Some("%1.%2")
    );
    assert_eq!(
        numbering
            .resolve(reference("Third action"))
            .unwrap()
            .text
            .as_deref(),
        Some("%1.%2.%3")
    );

    let inspected = crate::inspect_docx(
        bytes.clone(),
        &opensuite_protocol::InspectDocx {
            focus: opensuite_protocol::InspectDocxFocus::Paragraphs {
                offset: 0,
                limit: 20,
            },
        },
    );
    let Some(opensuite_protocol::InspectDocxContent::Paragraphs(page)) = inspected.content else {
        panic!("paragraph inspection expected")
    };
    assert_eq!(page.items[1].list.as_ref().unwrap().level, 1);
    assert_eq!(page.items[1].list.as_ref().unwrap().kind, "decimal");
    assert!(page.items[1].list.as_ref().unwrap().supported);

    let cleared = apply_list(
        bytes.clone(),
        &["Second action"],
        ParagraphListKind::None,
        1,
        false,
    )
    .unwrap();
    assert!(apply_list(bytes, &["Restart"], ParagraphListKind::Decimal, 3, false).is_err());
    let package = Package::from_bytes(cleared).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    let child = resolve_paragraph_anchor(
        &source,
        &TextTarget {
            text: "Second action".to_owned(),
            occurrence: None,
        },
    )
    .unwrap()
    .1;
    let ppr = source
        .children(child)
        .find(|id| word(&source, *id, "pPr"))
        .unwrap();
    assert!(
        crate::numbering::list_reference(&source, ppr)
            .unwrap()
            .is_none()
    );
}
