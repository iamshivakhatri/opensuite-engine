use super::*;
const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn table(label: &str, row_properties: &str) -> String {
    let mut xml = "<w:tbl><w:tblPr><w:tblW w:w=\"9000\" w:type=\"dxa\"/></w:tblPr><w:tblGrid><w:gridCol w:w=\"9000\"/></w:tblGrid>".to_owned();
    for i in 0..4 {
        xml.push_str(&format!("<w:tr>{row_properties}<w:tc><w:tcPr><w:shd w:fill=\"FAFAFA\"/></w:tcPr><w:p><w:pPr><w:spacing w:after=\"60\"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>{label}{i}</w:t></w:r></w:p></w:tc></w:tr>"));
    }
    xml.push_str("</w:tbl>");
    xml
}

fn fixture(body: &str) -> Vec<u8> {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, _) = crate::open_main_source(&package).unwrap();
    package.write_replaced_part_to_vec(&main, format!("<w:document xmlns:w=\"{WORD}\" xmlns:x=\"urn:keep\"><w:body>{body}<w:sectPr/></w:body></w:document>").as_bytes()).unwrap()
}
fn target(input: &[u8], index: usize) -> TableLayoutTarget {
    let package = Package::from_bytes(input.to_vec()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    TableLayoutTarget {
        handle: format!(
            "t{index}:{:016x}",
            sections::section_fingerprint(&package, &main, &source).unwrap()
        ),
    }
}
fn xml(input: &[u8]) -> Vec<u8> {
    let package = Package::from_bytes(input.to_vec()).unwrap();
    crate::open_main_source(&package)
        .unwrap()
        .1
        .original_bytes()
        .to_vec()
}
fn spacing(input: &[u8], before: usize, after: usize, points: f64) -> crate::DocxExecutionResult {
    crate::execute_docx_set_table_spacing(
        input.to_vec(),
        &SetTableSpacing {
            before: target(input, before),
            after: target(input, after),
            spacing_points: points,
            base_revision: None,
        },
    )
}
fn layout(
    input: &[u8],
    headers: Option<u32>,
    row: Option<usize>,
    split: Option<bool>,
) -> crate::DocxExecutionResult {
    crate::execute_docx_set_table_row_layout(
        input.to_vec(),
        &SetTableRowLayout {
            table: target(input, 0),
            repeating_header_rows: headers,
            row: row.map(|i| TableRowTarget {
                handle: Some(format!("t0:r{i}")),
                first_cell_text: String::new(),
                occurrence: None,
            }),
            prevent_split: split,
            base_revision: None,
        },
    )
}
fn output(result: crate::DocxExecutionResult) -> Vec<u8> {
    assert!(result.output_artifact.is_some(), "{:?}", result.operation);
    result.output_artifact.unwrap()
}
fn refused(result: crate::DocxExecutionResult) {
    assert!(
        result.output_artifact.is_none(),
        "unexpected accepted output"
    );
    assert!(!result.operation.diagnostics.is_empty());
}
fn rows(input: &[u8]) -> Vec<(Option<String>, Option<String>)> {
    let package = Package::from_bytes(input.to_vec()).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    let table = crate::DocxDocument::new(&source)
        .unwrap()
        .blocks()
        .find_map(|b| match b {
            crate::BodyBlock::Table(t) => Some(t),
            _ => None,
        })
        .unwrap();
    table
        .rows()
        .map(|row| {
            let props = source
                .children(row.source_id())
                .find(|id| word(&source, *id, "trPr"));
            let flag = |name| {
                props
                    .and_then(|p| source.children(p).find(|id| word(&source, *id, name)))
                    .map(|id| {
                        source
                            .node(id)
                            .unwrap()
                            .attribute("val")
                            .unwrap_or("1")
                            .to_owned()
                    })
            };
            (flag("tblHeader"), flag("cantSplit"))
        })
        .collect()
}

#[test]
fn exact_eight_point_gap_preserves_both_tables_and_other_parts() {
    let a = table("A", "");
    let b = table("B", "");
    let input = fixture(&format!("{a}{b}"));
    let out = output(spacing(&input, 0, 1, 8.0));
    let expected = String::from_utf8(xml(&input)).unwrap().replace(
        &format!("{a}{b}"),
        &format!("{a}{}{b}", separator_xml("w", 160)),
    );
    assert_eq!(xml(&out), expected.as_bytes());
    let text = String::from_utf8(xml(&out)).unwrap();
    assert!(text.contains("w:before=\"0\" w:after=\"0\""));
    assert!(text.contains("w:line=\"160\" w:lineRule=\"exact\""));
    let before = Package::from_bytes(input).unwrap();
    let after = Package::from_bytes(out).unwrap();
    for name in before.part_names_with_prefix("/") {
        if name.as_str() != "/word/document.xml" {
            assert_eq!(
                before.read_part_by_name(name).unwrap(),
                after.read_part_by_name(name).unwrap()
            );
        }
    }
}

#[test]
fn three_tables_keep_other_boundary_and_repeated_requests_do_not_duplicate() {
    let input = fixture(&format!(
        "{}{}{}",
        table("A", ""),
        table("B", ""),
        table("C", "")
    ));
    let first = output(spacing(&input, 0, 1, 8.0));
    let repeat = output(spacing(&first, 0, 1, 8.0));
    assert_eq!(xml(&first), xml(&repeat));
    let second = output(spacing(&repeat, 1, 2, 8.0));
    assert_eq!(
        String::from_utf8(xml(&second))
            .unwrap()
            .matches("w:line=\"160\"")
            .count(),
        2
    );
    let updated = output(spacing(&second, 0, 1, 12.5));
    let text = String::from_utf8(xml(&updated)).unwrap();
    assert_eq!(text.matches("w:line=\"250\"").count(), 1);
    assert_eq!(text.matches("w:line=\"160\"").count(), 1);
    // Layout flags do not silently separate deliberately adjacent tables.
    let untouched = output(layout(&input, Some(1), None, None));
    assert!(
        !String::from_utf8(xml(&untouched))
            .unwrap()
            .contains("w:lineRule=\"exact\"")
    );
}

#[test]
fn refuses_existing_arbitrary_separator_and_nonadjacent_or_reversed_tables() {
    for between in [
        "<w:p/>",
        "<w:p><w:r><w:t>Keep</w:t></w:r></w:p>",
        "<w:p><w:pPr><w:spacing w:line=\"160\" w:lineRule=\"exact\"/></w:pPr></w:p>",
    ] {
        let input = fixture(&format!("{}{between}{}", table("A", ""), table("B", "")));
        refused(spacing(&input, 0, 1, 8.0));
    }
    let input = fixture(&format!(
        "{}{}{}",
        table("A", ""),
        table("B", ""),
        table("C", "")
    ));
    for (a, b) in [(0, 2), (1, 0), (0, 0), (0, 99)] {
        refused(spacing(&input, a, b, 8.0));
    }
}

#[test]
fn refuses_invalid_spacing_and_stale_or_legacy_handles() {
    let input = fixture(&format!("{}{}", table("A", ""), table("B", "")));
    for points in [0.0, -8.0, f64::NAN, f64::INFINITY, 0.01, 1584.05] {
        refused(spacing(&input, 0, 1, points));
    }
    let operation = SetTableSpacing {
        before: target(&input, 0),
        after: target(&input, 1),
        spacing_points: 8.0,
        base_revision: None,
    };
    let changed = output(layout(&input, Some(1), None, None));
    refused(crate::execute_docx_set_table_spacing(changed, &operation));
    let mut legacy = operation;
    legacy.before.handle = "t0".to_owned();
    refused(crate::execute_docx_set_table_spacing(
        input.clone(),
        &legacy,
    ));
    let mut stale = SetTableRowLayout {
        table: target(&input, 0),
        repeating_header_rows: Some(1),
        row: None,
        prevent_split: None,
        base_revision: None,
    };
    stale.table.handle.push('0');
    refused(crate::execute_docx_set_table_row_layout(input, &stale));
}

#[test]
fn repeat_one_then_multiple_leading_headers_and_explicitly_clear() {
    let input = fixture(&table("A", ""));
    let one = output(layout(&input, Some(1), None, None));
    assert_eq!(
        rows(&one),
        vec![
            (Some("1".into()), None),
            (Some("0".into()), None),
            (Some("0".into()), None),
            (Some("0".into()), None)
        ]
    );
    let two = output(layout(&one, Some(2), None, None));
    assert_eq!(rows(&two)[1].0.as_deref(), Some("1"));
    let repeat = output(layout(&two, Some(2), None, None));
    assert_eq!(xml(&two), xml(&repeat));
    let smaller = output(layout(&two, Some(1), None, None));
    assert_eq!(rows(&smaller)[1].0.as_deref(), Some("0"));
    let cleared = output(layout(&two, Some(0), None, None));
    assert!(
        rows(&cleared)
            .iter()
            .all(|(header, _)| header.as_deref() != Some("1"))
    );
    refused(layout(&input, Some(5), None, None));
}

#[test]
fn prevents_and_restores_row_splitting_preserving_headers_and_unrelated_properties() {
    let props = "<w:trPr data-keep='row'><w:trHeight w:val='400' w:hRule='atLeast'/><w:tblHeader/><x:unknown x:val='keep'/></w:trPr>";
    let input = fixture(&table("A", props));
    let out = output(layout(&input, None, Some(2), Some(true)));
    assert_eq!(rows(&out)[2], (Some("1".into()), Some("1".into())));
    assert_eq!(rows(&out)[1].1, None);
    assert_eq!(
        String::from_utf8(xml(&out))
            .unwrap()
            .replace("<w:cantSplit w:val=\"1\"/>", ""),
        String::from_utf8(xml(&input)).unwrap()
    );
    let repeat = output(layout(&out, None, Some(2), Some(true)));
    assert_eq!(xml(&out), xml(&repeat));
    let restored = output(layout(&out, None, Some(2), Some(false)));
    assert_eq!(rows(&restored)[2], (Some("1".into()), Some("0".into())));
    let text = String::from_utf8(xml(&restored)).unwrap();
    assert_eq!(text.matches("<x:unknown x:val='keep'/>").count(), 4);
    assert_eq!(
        text.matches("<w:trHeight w:val='400' w:hRule='atLeast'/>")
            .count(),
        4
    );
    assert_eq!(text.matches("data-keep='row'").count(), 4);
}

#[test]
fn combined_flags_and_empty_row_properties_are_valid_after_reopen() {
    for properties in ["", "<w:trPr/>", "<w:trPr></w:trPr>"] {
        let input = fixture(&table("A", properties));
        let out = output(layout(&input, Some(2), Some(0), Some(true)));
        assert_eq!(rows(&out)[0], (Some("1".into()), Some("1".into())));
        assert_eq!(rows(&out)[1], (Some("1".into()), None));
        Package::from_bytes(out).unwrap().verify().unwrap();
    }
}

#[test]
fn rejects_mismatched_ambiguous_and_invalid_row_selections() {
    let input = fixture(&table("A", ""));
    for (headers, row, split) in [
        (None, None, None),
        (None, None, Some(true)),
        (Some(1), Some(2), None),
        (None, Some(10), Some(true)),
    ] {
        refused(layout(&input, headers, row, split));
    }
    for row in [
        TableRowTarget {
            handle: Some("t1:r1".into()),
            first_cell_text: String::new(),
            occurrence: None,
        },
        TableRowTarget {
            handle: Some("t0:r1".into()),
            first_cell_text: "stale".into(),
            occurrence: None,
        },
        TableRowTarget {
            handle: None,
            first_cell_text: "missing".into(),
            occurrence: None,
        },
    ] {
        refused(crate::execute_docx_set_table_row_layout(
            input.clone(),
            &SetTableRowLayout {
                table: target(&input, 0),
                repeating_header_rows: None,
                row: Some(row),
                prevent_split: Some(true),
                base_revision: None,
            },
        ));
    }
    let repeated = fixture(&table("A", "").replace("A2", "A1"));
    let op = SetTableRowLayout {
        table: target(&repeated, 0),
        repeating_header_rows: None,
        row: Some(TableRowTarget {
            handle: None,
            first_cell_text: "A1".into(),
            occurrence: None,
        }),
        prevent_split: Some(true),
        base_revision: None,
    };
    refused(crate::execute_docx_set_table_row_layout(repeated, &op));
}

#[test]
fn refuses_duplicates_malformed_flags_and_property_history() {
    for props in [
        "<w:trPr/><w:trPr/>",
        "<w:trPr><w:tblHeader/><w:tblHeader/></w:trPr>",
        "<w:trPr><w:cantSplit/><w:cantSplit/></w:trPr>",
        "<w:trPr><w:cantSplit w:val='maybe'/></w:trPr>",
        "<w:trPr><w:trPrChange/></w:trPr>",
        "<w:trPr><w:cantSplit x:keep='yes'/></w:trPr>",
        "<w:trPr><w:cantSplit x:val='1'/></w:trPr>",
    ] {
        let input = fixture(&table("A", props));
        refused(layout(&input, Some(1), Some(0), Some(false)));
    }
}

#[test]
fn refuses_merged_nested_floating_wrapped_and_protected_tables() {
    let base = table("A", "");
    let cases = [
        base.replace("<w:tcPr>", "<w:tcPr><w:gridSpan w:val='2'/>"),
        base.replace("<w:tcPr>", "<w:tcPr><w:hMerge/>"),
        base.replace("<w:tr>", "<w:tr><w:trPr><w:gridBefore w:val='1'/></w:trPr>"),
        base.replace("<w:tblPr>", "<w:tblPr><w:tblpPr/>"),
        base.replacen(
            "</w:tc>",
            "<w:tbl><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl></w:tc>",
            1,
        ),
        base.replacen(
            "<w:p>",
            "<w:p><w:bookmarkStart w:id='4'/><w:bookmarkEnd w:id='4'/>",
            1,
        ),
        base.replacen(
            "<w:p>",
            "<w:p><w:permStart w:id='4'/><w:permEnd w:id='4'/>",
            1,
        ),
        base.replacen("<w:p>", "<w:p><w:fldSimple w:instr='PAGE'/>", 1),
        base.replacen("<w:tr>", "<w:sdt><w:sdtContent><w:tr>", 1)
            .replacen("</w:tr>", "</w:tr></w:sdtContent></w:sdt>", 1),
    ];
    for case in cases {
        let input = fixture(&format!("{case}{}", table("B", "")));
        refused(layout(&input, Some(1), Some(0), Some(true)));
        refused(spacing(&input, 0, 1, 8.0));
    }
    let input = fixture(&format!(
        "<w:bookmarkStart w:id='3'/>{base}{}<w:bookmarkEnd w:id='3'/>",
        table("B", "")
    ));
    refused(layout(&input, Some(1), None, None));
    refused(spacing(&input, 0, 1, 8.0));
}

#[test]
fn preserves_unrelated_complex_document_content_and_uses_inspected_identity() {
    let complex = "<w:p><w:bookmarkStart w:id='5'/><w:r><w:t>Keep</w:t></w:r><w:bookmarkEnd w:id='5'/></w:p><w:p><w:fldSimple w:instr='PAGE'><w:r><w:t>1</w:t></w:r></w:fldSimple><w:ins w:id='8' w:author='A'><w:r><w:t>Revision</w:t></w:r></w:ins></w:p>";
    let input = fixture(&format!("{complex}{}{}", table("A", ""), table("B", "")));
    let result = crate::inspect_docx(
        input.clone(),
        &opensuite_protocol::InspectDocx {
            focus: opensuite_protocol::InspectDocxFocus::Tables {
                offset: 0,
                limit: 10,
            },
        },
    );
    let Some(opensuite_protocol::InspectDocxContent::Tables(page)) = result.content else {
        panic!("tables")
    };
    assert_eq!(
        page.items[0].layout_handle.as_ref().unwrap(),
        &target(&input, 0).handle
    );
    let out = output(spacing(&input, 0, 1, 8.0));
    let out = output(layout(&out, Some(2), Some(2), Some(true)));
    assert!(String::from_utf8(xml(&out)).unwrap().contains(complex));
}

#[test]
fn document_protection_and_changed_settings_are_checked() {
    let input = fixture(&format!("{}{}", table("A", ""), table("B", "")));
    let package = Package::from_bytes(input.clone()).unwrap();
    let part = package
        .part(&PartName::parse("/word/settings.xml").unwrap())
        .unwrap();
    for enforcement in ["1", "0"] {
        let settings = format!(
            "<w:settings xmlns:w=\"{WORD}\"><w:documentProtection w:edit=\"readOnly\" w:enforcement=\"{enforcement}\"/></w:settings>"
        );
        let protected = package
            .write_replaced_part_to_vec(&part, settings.as_bytes())
            .unwrap();
        if enforcement == "1" {
            refused(layout(&protected, Some(1), None, None));
            refused(spacing(&protected, 0, 1, 8.0));
        } else {
            output(layout(&protected, Some(1), None, None));
            output(spacing(&protected, 0, 1, 8.0));
        }
        let operation = SetTableRowLayout {
            table: target(&input, 0),
            repeating_header_rows: Some(1),
            row: None,
            prevent_split: None,
            base_revision: None,
        };
        refused(crate::execute_docx_set_table_row_layout(
            protected, &operation,
        ));
    }
}

#[test]
fn alternate_word_prefixes_work_and_default_namespace_authoring_is_refused() {
    let input = fixture(&format!("{}{}", table("A", ""), table("B", "")));
    let package = Package::from_bytes(input.clone()).unwrap();
    let (main, _) = crate::open_main_source(&package).unwrap();
    let original = String::from_utf8(xml(&input)).unwrap();
    let prefixed = original.replace("xmlns:w=", "xmlns:q=").replace("w:", "q:");
    let alternate = package
        .write_replaced_part_to_vec(&main, prefixed.as_bytes())
        .unwrap();
    let out = output(spacing(&alternate, 0, 1, 8.0));
    assert!(
        String::from_utf8(xml(&out))
            .unwrap()
            .contains("q:line=\"160\"")
    );
    output(layout(&alternate, Some(1), Some(1), Some(true)));
    let default_xml = original.replace("xmlns:w=", "xmlns=").replace("w:", "");
    let default = package
        .write_replaced_part_to_vec(&main, default_xml.as_bytes())
        .unwrap();
    refused(spacing(&default, 0, 1, 8.0));
    refused(layout(&default, Some(1), None, None));
}

#[test]
fn row_property_namespace_shadowing_uses_the_property_container_prefix() {
    let props = format!(
        "<q:trPr xmlns:q=\"{WORD}\" xmlns:w=\"urn:other\"><q:trHeight q:val=\"400\"/></q:trPr>"
    );
    let input = fixture(&table("A", &props));
    let out = output(layout(&input, Some(1), Some(0), Some(true)));
    assert_eq!(rows(&out)[0], (Some("1".into()), Some("1".into())));
    assert!(String::from_utf8(xml(&out)).unwrap().contains(&format!(
        "<q:trPr xmlns:q=\"{WORD}\" xmlns:w=\"urn:other\">"
    )));
}

#[test]
fn long_multiline_rows_and_explicit_false_are_source_preserved() {
    let base = table("A", "");
    use std::fmt::Write as _;
    let mut lines = String::new();
    for i in 0..90 {
        write!(lines, "<w:t>Line {i}</w:t><w:br/>").unwrap();
    }
    let base = base.replace("<w:t>A2</w:t>", &lines);
    let input = fixture(&base);
    let out = output(layout(&input, None, Some(2), Some(true)));
    assert_eq!(
        String::from_utf8(xml(&out))
            .unwrap()
            .replace("<w:trPr><w:cantSplit w:val=\"1\"/></w:trPr>", ""),
        String::from_utf8(xml(&input)).unwrap()
    );
    let allow = output(layout(&input, None, Some(2), Some(false)));
    assert_eq!(rows(&allow)[2].1.as_deref(), Some("0"));
}
