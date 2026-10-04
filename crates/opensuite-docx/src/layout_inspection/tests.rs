use super::*;
use opensuite_protocol::*;

fn inspect(bytes: Vec<u8>) -> LayoutSnapshot {
    inspect_docx_layout(
        bytes,
        &LayoutOptions {
            block_limit: 100,
            ..Default::default()
        },
    )
}
fn fixture(xml: &str) -> Vec<u8> {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let main = package.main_office_document().unwrap();
    package
        .write_replaced_part_to_vec(&main, xml.as_bytes())
        .unwrap()
}
#[test]
fn geometry_section_ownership_and_effective_pagination_controls() {
    let bytes = fixture(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/><w:pageBreakBefore/><w:keepLines/><w:widowControl w:val="0"/><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:bottom="1440" w:left="1440" w:right="1440" w:gutter="0"/></w:sectPr></w:pPr><w:r><w:br w:type="page"/></w:r></w:p><w:p><w:r><w:t>Landscape</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/><w:pgMar w:top="720" w:bottom="720" w:left="720" w:right="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#,
    );
    std::fs::write("/private/tmp/opensuite-layout-explicit.docx", &bytes).unwrap();
    let original = bytes.clone();
    let layout = inspect(bytes.clone());
    assert!(layout.ok);
    assert_eq!(bytes, original);
    assert_eq!(layout.sections[0].usable_width_twips, Some(9360));
    assert_eq!(layout.sections[0].usable_height_twips, Some(12960));
    assert_eq!(layout.sections[1].usable_width_twips, Some(14400));
    assert_eq!(layout.blocks[0].section_index, Some(0));
    assert_eq!(layout.blocks[1].section_index, Some(1));
    let formatting = layout.blocks[0].paragraph_formatting.as_ref().unwrap();
    assert_eq!(formatting.page_break_before, Some(true));
    assert_eq!(formatting.keep_with_next, Some(true));
    assert_eq!(formatting.keep_lines, Some(true));
    assert_eq!(formatting.widow_control, Some(false));
    assert_eq!(layout.explicit_page_break_count, 1);
    assert_eq!(layout.section_break_count, 1);
    assert_eq!(layout.rendered_page_count, None);
}
#[test]
fn wide_table_row_controls_and_invalid_unknown_geometry() {
    let bytes = fixture(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:tbl><w:tblPr><w:tblW w:w="10000" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="10000"/></w:tblGrid><w:tr><w:trPr><w:cantSplit/><w:trHeight w:val="400" w:hRule="atLeast"/></w:trPr><w:tc><w:p/></w:tc></w:tr></w:tbl><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:bottom="1440" w:left="1440" w:right="1440"/></w:sectPr></w:body></w:document>"#,
    );
    let layout = inspect(bytes);
    assert!(layout.ok);
    assert!(
        layout
            .diagnostics
            .iter()
            .any(|d| d.code == "TABLE_WIDTH_EXCEEDS_PAGE")
    );
    assert_eq!(layout.tables[0].cannot_split_row_count, 1);
    assert_eq!(layout.tables[0].rows[0].height_twips, Some(400));
    assert_eq!(layout.tables[0].section_index, Some(0));
    let unknown = inspect(fixture(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:sectPr><w:pgSz w:w="100"/></w:sectPr></w:body></w:document>"#,
    ));
    assert_eq!(unknown.sections[0].usable_width_twips, None);
    assert_eq!(unknown.sections[0].usable_height_twips, None);
}
#[test]
fn multi_section_roundtrip_and_large_input_are_bounded_read_only() {
    let operation = InsertSectionBreak {
        placement: ParagraphPlacement::End,
        break_type: SectionBreakType::Continuous,
    };
    let bytes = crate::execute_docx_insert_section_break(crate::create_blank_docx(), &operation)
        .output_artifact
        .unwrap();
    let original = bytes.clone();
    let layout = inspect(bytes.clone());
    assert_eq!(layout.section_count, 2);
    assert_eq!(bytes, original);
    let xml = format!(
        "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{}<w:sectPr/></w:body></w:document>",
        "<w:p><w:r><w:t>body</w:t></w:r></w:p>".repeat(5000)
    );
    let large = fixture(&xml);
    let layout = inspect_docx_layout(large.clone(), &LayoutOptions::default());
    assert!(layout.ok);
    assert_eq!(layout.paragraph_count, 5000);
    assert!(layout.blocks.is_empty());
    assert_eq!(layout.paragraph_patterns.len(), 1);
    let bounded = inspect_docx_layout(
        large,
        &LayoutOptions {
            block_limit: 20,
            block_offset: 10,
            ..Default::default()
        },
    );
    assert_eq!(bounded.blocks.len(), 20);
    assert!(bounded.has_more_blocks);
}
