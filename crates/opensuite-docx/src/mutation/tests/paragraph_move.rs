use super::support::*;
use opensuite_protocol::{MoveParagraph, ParagraphMovePosition, ParagraphMoveTarget};

fn document(body: &str) -> Vec<u8> {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, _) = crate::open_main_source(&package).unwrap();
    package.write_replaced_part_to_vec(&main, format!("<w:document xmlns:w=\"{WORD}\" xmlns:x=\"urn:keep\"><w:body>{body}<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/></w:sectPr></w:body></w:document>").as_bytes()).unwrap()
}
fn p(text: &str) -> String {
    format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>")
}
fn operation(
    input: &[u8],
    from: usize,
    to: usize,
    position: ParagraphMovePosition,
) -> MoveParagraph {
    let result = crate::inspect_docx(
        input.to_vec(),
        &InspectDocx {
            focus: InspectDocxFocus::BodyBlocks {
                offset: 0,
                limit: 100,
            },
        },
    );
    let Some(InspectDocxContent::BodyBlocks(page)) = result.content else {
        panic!("body blocks unavailable")
    };
    MoveParagraph {
        target: ParagraphMoveTarget {
            handle: page.items[from].movement_handle.clone().unwrap(),
        },
        destination: ParagraphMoveTarget {
            handle: page.items[to].movement_handle.clone().unwrap(),
        },
        position,
        base_revision: None,
    }
}
fn xml(input: &[u8]) -> Vec<u8> {
    let package = Package::from_bytes(input.to_vec()).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    source.original_bytes().to_vec()
}
fn moved(input: &[u8], from: usize, to: usize, position: ParagraphMovePosition) -> Vec<u8> {
    let result =
        crate::execute_docx_move_paragraph(input.to_vec(), &operation(input, from, to, position));
    assert!(
        result.operation.diagnostics.is_empty(),
        "{:?}",
        result.operation
    );
    result.output_artifact.unwrap()
}

#[test]
fn moves_original_styled_numbered_formatted_xml_in_both_directions() {
    let a = p("First");
    let b = "<w:p w:rsidR='00112233'><w:pPr><w:pStyle w:val='Normal'/><w:numPr><w:ilvl w:val='0'/><w:numId w:val='1'/></w:numPr><w:keepNext/><w:spacing w:after='160'/></w:pPr><w:r><w:rPr><w:b/><w:color w:val='FF0000'/></w:rPr><w:t>Styled</w:t><w:tab/><w:br/></w:r></w:p>";
    let c = p("Third");
    let input = document(&format!("{a}\n{b}\n{c}"));
    let output = moved(&input, 1, 2, ParagraphMovePosition::After);
    assert_eq!(xml(&output), xml(&document(&format!("{a}\n\n{c}{b}"))));
    let back = moved(&output, 2, 0, ParagraphMovePosition::Before);
    let text = crate::inspect_docx_references(back.clone(), 0, 20);
    assert!(text.ok);
    assert!(
        String::from_utf8(xml(&back))
            .unwrap()
            .contains(&format!("{b}{a}"))
    );
    let original = Package::from_bytes(input).unwrap();
    let changed = Package::from_bytes(back).unwrap();
    for part in original
        .part_names_with_prefix("/")
        .filter(|part| part.as_str() != "/word/document.xml")
    {
        assert_eq!(
            original.read_part_by_name(part).unwrap(),
            changed.read_part_by_name(part).unwrap()
        );
    }
}

#[test]
fn same_paragraph_and_adjacent_moves_are_xml_noops_and_empty_paragraphs_work() {
    let input = document(&format!(
        "{}\n<!-- keep -->\n{}<w:p/>",
        p("First"),
        p("Second")
    ));
    for (from, to, position) in [
        (0, 0, ParagraphMovePosition::Before),
        (1, 1, ParagraphMovePosition::After),
        (0, 1, ParagraphMovePosition::Before),
        (1, 0, ParagraphMovePosition::After),
    ] {
        assert_eq!(xml(&moved(&input, from, to, position)), xml(&input));
    }
    assert!(
        String::from_utf8(xml(&moved(&input, 2, 0, ParagraphMovePosition::Before)))
            .unwrap()
            .contains("<w:body><w:p/>")
    );
}

#[test]
fn refuses_stale_malformed_and_nonparagraph_targets() {
    let input = document(&format!("{}{}{}", p("First"), p("Second"), p("Third")));
    let mut op = operation(&input, 0, 2, ParagraphMovePosition::After);
    let changed = moved(&input, 0, 2, ParagraphMovePosition::After);
    assert!(
        crate::execute_docx_move_paragraph(changed, &op)
            .output_artifact
            .is_none()
    );
    for handle in ["b0", "b999:bad", "junk"] {
        op.target.handle = handle.into();
        assert!(
            crate::execute_docx_move_paragraph(input.clone(), &op)
                .output_artifact
                .is_none()
        );
    }
    let input = document(&format!(
        "{}<w:p><w:r><w:br w:type='page'/></w:r></w:p><w:tbl><w:tr><w:tc>{}</w:tc></w:tr></w:tbl>",
        p("First"),
        p("Cell")
    ));
    let package = Package::from_bytes(input.clone()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let stamp = section_fingerprint(&package, &main, &source).unwrap();
    for index in [1, 2] {
        let op = MoveParagraph {
            target: ParagraphMoveTarget {
                handle: format!("b{index}:{stamp:016x}"),
            },
            destination: ParagraphMoveTarget {
                handle: format!("b0:{stamp:016x}"),
            },
            position: ParagraphMovePosition::Before,
            base_revision: None,
        };
        assert!(
            crate::execute_docx_move_paragraph(input.clone(), &op)
                .output_artifact
                .is_none()
        );
    }
}

#[test]
fn duplicate_text_uses_source_identity_and_unknown_body_structures_refuse() {
    let first = "<w:p data-keep='first'><w:r><w:t>Same</w:t></w:r></w:p>";
    let second = "<w:p data-keep='second'><w:r><w:t>Same</w:t></w:r></w:p>";
    let input = document(&format!("{first}{second}"));
    assert_eq!(
        xml(&moved(&input, 1, 0, ParagraphMovePosition::Before)),
        xml(&document(&format!("{second}{first}")))
    );
    let input = document(&format!("{}<x:unknown/>{}", p("First"), p("Last")));
    assert!(
        crate::execute_docx_move_paragraph(
            input.clone(),
            &operation(&input, 0, 1, ParagraphMovePosition::After)
        )
        .output_artifact
        .is_none()
    );
}

#[test]
fn refuses_sections_comments_fields_revisions_bookmarks_and_unknown_inline_xml() {
    let protected = [
        "<w:p><w:pPr><w:sectPr/></w:pPr><w:r><w:t>Unsafe</w:t></w:r></w:p>",
        "<w:p><w:r><w:t>Unsafe</w:t></w:r><w:bookmarkStart w:id='1' w:name='Mark'/><w:bookmarkEnd w:id='1'/></w:p>",
        "<w:p><w:commentRangeStart w:id='1'/><w:r><w:t>Unsafe</w:t></w:r><w:commentRangeEnd w:id='1'/></w:p>",
        "<w:p><w:fldSimple w:instr='PAGE'><w:r><w:t>Unsafe</w:t></w:r></w:fldSimple></w:p>",
        "<w:p><w:ins w:id='1'><w:r><w:t>Unsafe</w:t></w:r></w:ins></w:p>",
        "<w:p><w:pPr><w:pPrChange/></w:pPr><w:r><w:t>Unsafe</w:t></w:r></w:p>",
        "<w:p><w:pPr><w:framePr/></w:pPr><w:r><w:t>Unsafe</w:t></w:r></w:p>",
        "<w:p><w:r><w:rPr><w:rPrChange/></w:rPr><w:t>Unsafe</w:t></w:r></w:p>",
        "<w:p><w:r><w:t>Unsafe</w:t><w:footnoteReference w:id='1'/></w:r></w:p>",
        "<w:p><w:r><w:rPr><x:unknown/></w:rPr><w:t>Unsafe</w:t></w:r></w:p>",
        "<w:p><w:hyperlink w:anchor='Mark'><w:r><w:t>Unsafe</w:t></w:r></w:hyperlink></w:p>",
    ];
    for unsafe_xml in protected {
        let input = document(&format!("{}{}", unsafe_xml, p("Safe")));
        for (from, to) in [(0, 1), (1, 0)] {
            let result = crate::execute_docx_move_paragraph(
                input.clone(),
                &operation(&input, from, to, ParagraphMovePosition::After),
            );
            assert!(result.output_artifact.is_none(), "accepted {unsafe_xml}");
        }
    }
    for markers in [
        (
            "<w:bookmarkStart w:id='1' w:name='Mark'/>",
            "<w:bookmarkEnd w:id='1'/>",
        ),
        (
            "<w:r><w:fldChar w:fldCharType='begin'/><w:instrText>PAGE</w:instrText><w:fldChar w:fldCharType='separate'/></w:r>",
            "<w:r><w:fldChar w:fldCharType='end'/></w:r>",
        ),
    ] {
        let input = document(&format!(
            "<w:p>{}</w:p>{}<w:p>{}</w:p>{}",
            markers.0,
            p("Enclosed"),
            markers.1,
            p("Outside")
        ));
        assert!(
            crate::execute_docx_move_paragraph(
                input.clone(),
                &operation(&input, 1, 3, ParagraphMovePosition::After)
            )
            .output_artifact
            .is_none()
        );
    }
    let input = document(&format!(
        "{}<w:p><w:pPr><w:sectPr/></w:pPr></w:p>{}",
        p("Section one"),
        p("Section two")
    ));
    assert!(
        crate::execute_docx_move_paragraph(
            input.clone(),
            &operation(&input, 0, 2, ParagraphMovePosition::After)
        )
        .output_artifact
        .is_none()
    );
}
