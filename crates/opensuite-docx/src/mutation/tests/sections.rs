use super::support::*;
use opensuite_protocol::{
    HeaderFooterVariant, InsertSectionBreak, SectionBreakType, SectionHeaderFooterChange,
    SectionTarget, SetOddEvenHeaders, SetSectionHeaderFooter, SetSectionProperties,
};

fn read(bytes: &[u8]) -> (Package, Part, SourceDocument) {
    let package = Package::from_bytes(bytes.to_vec()).unwrap();
    package.verify().unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    (package, main, source)
}
fn target(bytes: &[u8], index: usize) -> SectionTarget {
    let (package, main, source) = read(bytes);
    SectionTarget {
        handle: inspect_sections(&package, &main, &source).unwrap()[index]
            .handle
            .clone(),
    }
}
fn split(bytes: &[u8], kind: SectionBreakType) -> Vec<u8> {
    let (package, main, source) = read(bytes);
    insert_section_break_to_vec(
        &package,
        &main,
        &source,
        &InsertSectionBreak {
            placement: ParagraphPlacement::End,
            break_type: kind,
        },
    )
    .unwrap()
}
fn header(
    bytes: &[u8],
    index: usize,
    kind: HeaderFooterKind,
    variant: HeaderFooterVariant,
    change: SectionHeaderFooterChange,
) -> Vec<u8> {
    let (package, main, source) = read(bytes);
    set_section_header_footer_to_vec(
        &package,
        &main,
        &source,
        &SetSectionHeaderFooter {
            target: target(bytes, index),
            kind,
            variant,
            change,
        },
    )
    .unwrap()
}

#[test]
fn three_sections_keep_independent_setup_and_reject_stale_target() {
    let bytes = split(
        &split(&crate::create_blank_docx(), SectionBreakType::NextPage),
        SectionBreakType::NextPage,
    );
    let (package, main, source) = read(&bytes);
    let stale = target(&bytes, 1);
    let section_before: Vec<_> = crate::DocxDocument::new(&source)
        .unwrap()
        .sections()
        .map(|section| {
            let span = source.node(section.source_id()).unwrap().span();
            source.original_bytes()[span.start..span.end].to_vec()
        })
        .collect();
    let operation = SetSectionProperties {
        target: stale.clone(),
        page_setup: Some(SetPageSetup {
            orientation: Some(PageOrientation::Landscape),
            margins: Some(PageMargins {
                left_twips: Some(720),
                right_twips: Some(900),
                ..Default::default()
            }),
            paper_size: Some(PaperSize::Letter),
            base_revision: None,
        }),
        different_first_page: Some(true),
        break_type: None,
        page_number_start: Some(PropertyPatch::Set(1)),
    };
    let output = set_section_properties_to_vec(&package, &main, &source, &operation).unwrap();
    let (package, main, source) = read(&output);
    let sections = inspect_sections(&package, &main, &source).unwrap();
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[1].orientation.as_deref(), Some("landscape"));
    assert_eq!(sections[1].page_width_twips, Some(15840));
    assert_eq!(sections[1].margins_twips["left"], 720);
    assert!(sections[1].different_first_page);
    assert_eq!(sections[1].page_number_start, Some(1));
    let actual: Vec<_> = crate::DocxDocument::new(&source)
        .unwrap()
        .sections()
        .map(|section| {
            let span = source.node(section.source_id()).unwrap().span();
            source.original_bytes()[span.start..span.end].to_vec()
        })
        .collect();
    assert_eq!(actual[0], section_before[0]);
    assert_eq!(actual[2], section_before[2]);
    assert!(set_section_properties_to_vec(&package, &main, &source, &operation).is_err());
    let mut invalid = operation.clone();
    invalid.target.handle = "bad".into();
    assert!(set_section_properties_to_vec(&package, &main, &source, &invalid).is_err());
    invalid.target.handle = sections[1].handle.replacen("s1:", "s99:", 1);
    assert!(set_section_properties_to_vec(&package, &main, &source, &invalid).is_err());
}

#[test]
fn all_break_types_are_real_sections_and_preserve_package_parts() {
    for kind in [
        SectionBreakType::NextPage,
        SectionBreakType::Continuous,
        SectionBreakType::OddPage,
        SectionBreakType::EvenPage,
    ] {
        let before = crate::create_blank_docx();
        let output = split(&before, kind);
        let (old, _, _) = read(&before);
        let (package, main, source) = read(&output);
        let sections = inspect_sections(&package, &main, &source).unwrap();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[1].break_type, break_type_name(kind));
        assert!(!String::from_utf8_lossy(source.original_bytes()).contains("w:type=\"page\""));
        for name in old
            .part_names_with_prefix("/")
            .filter(|n| n.as_str() != "/word/document.xml")
        {
            assert_eq!(
                old.read_part_by_name(name).unwrap(),
                package.read_part_by_name(name).unwrap()
            );
        }
    }
}

#[test]
fn variants_link_unlink_and_shared_parts_remain_safe() {
    let bytes = header(
        &crate::create_blank_docx(),
        0,
        HeaderFooterKind::Header,
        HeaderFooterVariant::Default,
        SectionHeaderFooterChange::SetText("Shared".into()),
    );
    // Inserting boundaries retains safe relationship sharing.
    let bytes = split(
        &split(&bytes, SectionBreakType::NextPage),
        SectionBreakType::NextPage,
    );
    let (old, main, source) = read(&bytes);
    let before = inspect_sections(&old, &main, &source).unwrap();
    let shared = PartName::parse(before[0].headers_footers[0].part_name.as_ref().unwrap()).unwrap();
    let shared_bytes = old.read_part_by_name(&shared).unwrap();
    let bytes = header(
        &bytes,
        1,
        HeaderFooterKind::Header,
        HeaderFooterVariant::Default,
        SectionHeaderFooterChange::SetText("Only section two".into()),
    );
    let bytes = header(
        &bytes,
        2,
        HeaderFooterKind::Footer,
        HeaderFooterVariant::First,
        SectionHeaderFooterChange::SetText("First page".into()),
    );
    let (package, main, source) = read(&bytes);
    let sections = inspect_sections(&package, &main, &source).unwrap();
    assert_eq!(
        sections[0].headers_footers[0].text.as_deref(),
        Some("Shared")
    );
    assert_eq!(
        sections[1].headers_footers[0].text.as_deref(),
        Some("Only section two")
    );
    assert_eq!(
        sections[2].headers_footers[0].text.as_deref(),
        Some("Shared")
    );
    assert_eq!(package.read_part_by_name(&shared).unwrap(), shared_bytes);
    let bytes = header(
        &bytes,
        2,
        HeaderFooterKind::Header,
        HeaderFooterVariant::Default,
        SectionHeaderFooterChange::Inherit,
    );
    let (package, main, source) = read(&bytes);
    let sections = inspect_sections(&package, &main, &source).unwrap();
    assert!(sections[2].headers_footers[0].linked_to_previous);
    assert_eq!(
        sections[2].headers_footers[0].text.as_deref(),
        Some("Only section two")
    );
    let bytes = header(
        &bytes,
        2,
        HeaderFooterKind::Header,
        HeaderFooterVariant::Default,
        SectionHeaderFooterChange::Unlink,
    );
    let (package, main, source) = read(&bytes);
    assert!(
        !inspect_sections(&package, &main, &source).unwrap()[2].headers_footers[0]
            .linked_to_previous
    );
    let operation = SetSectionHeaderFooter {
        target: target(&bytes, 1),
        kind: HeaderFooterKind::Footer,
        variant: HeaderFooterVariant::Even,
        change: SectionHeaderFooterChange::SetText("Even".into()),
    };
    assert!(set_section_header_footer_to_vec(&package, &main, &source, &operation).is_err());
    let bytes = set_odd_even_headers_to_vec(
        &package,
        &main,
        &source,
        &SetOddEvenHeaders { enabled: true },
    )
    .unwrap();
    let bytes = header(
        &bytes,
        1,
        HeaderFooterKind::Footer,
        HeaderFooterVariant::Even,
        SectionHeaderFooterChange::SetText("Even".into()),
    );
    let bytes = header(
        &bytes,
        1,
        HeaderFooterKind::Footer,
        HeaderFooterVariant::Default,
        SectionHeaderFooterChange::SetPageNumber(Some(PageNumberAlignment::Center)),
    );
    let (package, main, source) = read(&bytes);
    let sections = inspect_sections(&package, &main, &source).unwrap();
    assert!(sections.iter().all(|s| s.odd_even_headers));
    assert_eq!(sections[1].headers_footers[5].text.as_deref(), Some("Even"));
    assert_eq!(
        sections[1].headers_footers[3]
            .page_number_alignment
            .as_deref(),
        Some("center")
    );
    let operation = SetSectionProperties {
        target: target(&bytes, 1),
        page_setup: None,
        different_first_page: Some(false),
        break_type: None,
        page_number_start: Some(PropertyPatch::Clear),
    };
    let bytes = set_section_properties_to_vec(&package, &main, &source, &operation).unwrap();
    let (package, main, source) = read(&bytes);
    assert!(!inspect_sections(&package, &main, &source).unwrap()[1].different_first_page);
}

#[test]
fn empty_section_properties_expand_and_unsafe_boundaries_fail_safely() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let main = package.main_office_document().unwrap();
    let xml = format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>Keep</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"
    );
    let bytes = package
        .write_replaced_part_to_vec(&main, xml.as_bytes())
        .unwrap();
    let (package, main, source) = read(&bytes);
    let operation = SetSectionProperties {
        target: target(&bytes, 0),
        page_setup: Some(SetPageSetup {
            paper_size: Some(PaperSize::Letter),
            orientation: Some(PageOrientation::Landscape),
            margins: Some(PageMargins {
                top_twips: Some(900),
                ..Default::default()
            }),
            base_revision: None,
        }),
        different_first_page: Some(true),
        break_type: Some(SectionBreakType::Continuous),
        page_number_start: Some(PropertyPatch::Set(3)),
    };
    let bytes = set_section_properties_to_vec(&package, &main, &source, &operation).unwrap();
    let (package, main, source) = read(&bytes);
    let sections = inspect_sections(&package, &main, &source).unwrap();
    assert_eq!(sections[0].orientation.as_deref(), Some("landscape"));
    assert_eq!(sections[0].page_number_start, Some(3));
    assert_eq!(sections[0].break_type, "continuous");
    let bytes = header(
        &bytes,
        0,
        HeaderFooterKind::Header,
        HeaderFooterVariant::Default,
        SectionHeaderFooterChange::SetText("Before".into()),
    );
    let stale = target(&bytes, 0);
    let bytes = header(
        &bytes,
        0,
        HeaderFooterKind::Header,
        HeaderFooterVariant::Default,
        SectionHeaderFooterChange::SetText("After".into()),
    );
    let (package, main, source) = read(&bytes);
    let mut operation = operation.clone();
    operation.target = stale;
    assert_eq!(
        set_section_properties_to_vec(&package, &main, &source, &operation)
            .unwrap_err()
            .diagnostics[0]
            .code,
        "PRECONDITION_FAILED"
    );
    for xml in [
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:sectPr><w:sectPrChange><w:sectPr/></w:sectPrChange></w:sectPr></w:body></w:document>"
        ),
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:sectPr><w:pgSz/><w:pgSz/></w:sectPr></w:body></w:document>"
        ),
    ] {
        let unsafe_source = SourceDocument::parse(xml.into_bytes()).unwrap();
        assert!(safe_sections(&unsafe_source).is_err());
    }
}
