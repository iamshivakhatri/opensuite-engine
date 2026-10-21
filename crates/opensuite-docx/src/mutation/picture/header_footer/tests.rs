use super::*;
use crate::mutation::header_footer::{header_footer_part, variant_reference};
use opensuite_protocol::{ParagraphAlignment, PicturePlacement};

fn image() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend(2000u32.to_be_bytes());
    bytes.extend(1000u32.to_be_bytes());
    bytes.extend([8, 2, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}
fn operation(
    bytes: &[u8],
    index: usize,
    kind: HeaderFooterKind,
    variant: HeaderFooterVariant,
) -> InsertPicture {
    let package = Package::from_bytes(bytes.to_vec()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    InsertPicture {
        image_bytes: image(),
        placement: PicturePlacement::HeaderFooter {
            target: SectionTarget {
                handle: sections::inspect_sections(&package, &main, &source).unwrap()[index]
                    .handle
                    .clone(),
            },
            kind,
            variant,
        },
        auto_fit: true,
        alignment: Some(ParagraphAlignment::Center),
        size: None,
        layout: None,
        alt_text: Some("Header logo".into()),
        base_revision: None,
    }
}
fn change_main(bytes: &[u8], edit: impl FnOnce(String) -> String) -> Vec<u8> {
    let package = Package::from_bytes(bytes.to_vec()).unwrap();
    let main = package.main_office_document().unwrap();
    let xml = String::from_utf8(package.read_part(&main).unwrap()).unwrap();
    package
        .write_replaced_part_to_vec(&main, edit(xml).as_bytes())
        .unwrap()
}
fn insert(bytes: Vec<u8>, op: &InsertPicture) -> Vec<u8> {
    let result = crate::execute_docx_insert_picture(bytes, op);
    assert!(result.output_artifact.is_some(), "{:?}", result.operation);
    result.output_artifact.unwrap()
}
fn refuse(bytes: Vec<u8>, op: &InsertPicture) {
    let result = crate::execute_docx_insert_picture(bytes, op);
    assert!(result.output_artifact.is_none(), "{:?}", result.operation);
    assert!(!result.operation.diagnostics.is_empty());
}
#[test]
fn creates_missing_default_first_even_and_footer_without_changing_page_flags() {
    for (kind, variant) in [
        (HeaderFooterKind::Header, HeaderFooterVariant::Default),
        (HeaderFooterKind::Header, HeaderFooterVariant::First),
        (HeaderFooterKind::Header, HeaderFooterVariant::Even),
        (HeaderFooterKind::Footer, HeaderFooterVariant::Default),
    ] {
        let mut bytes = crate::create_blank_docx();
        if variant == HeaderFooterVariant::First {
            bytes = change_main(&bytes, |s| {
                s.replace("</w:sectPr>", "<w:titlePg/></w:sectPr>")
            });
        }
        if variant == HeaderFooterVariant::Even {
            bytes = crate::execute_docx_set_odd_even_headers(
                bytes,
                &SetOddEvenHeaders { enabled: true },
            )
            .output_artifact
            .unwrap();
        }
        let op = operation(&bytes, 0, kind, variant);
        let output = insert(bytes, &op);
        let inspection = crate::inspect_docx_images(output.clone(), 0, 10);
        assert!(inspection.ok);
        assert_eq!(inspection.total, 1);
        let picture = &inspection.images[0];
        assert_eq!(picture.display_width_emu, Some(355_600));
        assert_eq!(picture.display_height_emu, Some(177_800));
        assert_eq!(
            crate::read_docx_embedded_image(output.clone(), picture.handle.as_ref().unwrap())
                .unwrap(),
            image()
        );
        let package = Package::from_bytes(output).unwrap();
        package.verify().unwrap();
        let part = package
            .part(&PartName::parse(&picture.part_name).unwrap())
            .unwrap();
        let xml = String::from_utf8(package.read_part(&part).unwrap()).unwrap();
        assert!(xml.contains("<w:jc w:val=\"center\"/>"));
    }
}
#[test]
fn appends_after_formatted_text_and_page_field_and_preserves_other_parts() {
    let bytes = crate::create_blank_docx();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let handle = sections::inspect_sections(&package, &main, &source).unwrap()[0]
        .handle
        .clone();
    let bytes = crate::execute_docx_set_section_header_footer(
        crate::create_blank_docx(),
        &SetSectionHeaderFooter {
            target: SectionTarget { handle },
            kind: HeaderFooterKind::Footer,
            variant: HeaderFooterVariant::Default,
            change: SectionHeaderFooterChange::SetText("Existing footer".into()),
        },
    )
    .output_artifact
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let part = header_footer_part(
        &package,
        &main,
        &source,
        variant_reference(
            &source,
            sections::safe_sections(&source).unwrap()[0],
            HeaderFooterKind::Footer,
            HeaderFooterVariant::Default,
        )
        .unwrap()
        .unwrap(),
        HeaderFooterKind::Footer,
    )
    .unwrap();
    let original = String::from_utf8(package.read_part(&part).unwrap()).unwrap().replace("<w:r>", "<w:r><w:rPr><w:b/></w:rPr>").replace("</w:ftr>", "<w:p><w:fldSimple w:instr=\"PAGE\"><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p></w:ftr>");
    let bytes = package
        .write_replaced_part_to_vec(&part, original.as_bytes())
        .unwrap();
    let op = operation(
        &bytes,
        0,
        HeaderFooterKind::Footer,
        HeaderFooterVariant::Default,
    );
    let before = Package::from_bytes(bytes.clone()).unwrap();
    let output = insert(bytes, &op);
    let after = Package::from_bytes(output.clone()).unwrap();
    let xml = String::from_utf8(after.read_part(&part).unwrap()).unwrap();
    assert_eq!(
        xml.replace(xml.get(original.len() - 8..xml.len() - 8).unwrap(), ""),
        original
    );
    for name in before.part_names_with_prefix("/") {
        if name != &part.name && name.as_str() != "/[Content_Types].xml" {
            assert_eq!(
                before.read_part_by_name(name).unwrap(),
                after.read_part_by_name(name).unwrap(),
                "{name:?}"
            );
        }
    }
    assert_eq!(
        crate::inspect_docx_fields(output, 0, 10).fields[0]
            .instruction
            .as_deref(),
        Some("PAGE")
    );
}
#[test]
fn refuses_inactive_variants_stale_sections_and_floating_layout() {
    let bytes = crate::create_blank_docx();
    for variant in [HeaderFooterVariant::First, HeaderFooterVariant::Even] {
        refuse(
            bytes.clone(),
            &operation(&bytes, 0, HeaderFooterKind::Header, variant),
        );
    }
    let mut op = operation(
        &bytes,
        0,
        HeaderFooterKind::Header,
        HeaderFooterVariant::Default,
    );
    let output = insert(bytes.clone(), &op);
    refuse(output, &op);
    op.layout = Some(Default::default());
    refuse(bytes.clone(), &op);
    op.layout = None;
    op.size = Some(opensuite_protocol::PictureSizeChange::WidthEmu(10));
    refuse(bytes, &op);
}
#[test]
fn refuses_inherited_parts_and_parts_inherited_by_the_following_section() {
    let bytes = crate::create_blank_docx();
    let bytes = insert(
        bytes.clone(),
        &operation(
            &bytes,
            0,
            HeaderFooterKind::Header,
            HeaderFooterVariant::Default,
        ),
    );
    let bytes = change_main(&bytes, |xml| {
        let start = xml.find("<w:sectPr>").unwrap();
        let end = xml.find("</w:sectPr>").unwrap() + 11;
        let first = xml[start..end].to_string();
        xml.replace(
            "<w:body>",
            &format!("<w:body><w:p><w:pPr>{first}</w:pPr></w:p>"),
        )
    });
    // Both explicit references resolve to the same part.
    refuse(
        bytes.clone(),
        &operation(
            &bytes,
            0,
            HeaderFooterKind::Header,
            HeaderFooterVariant::Default,
        ),
    );
    let inherited = change_main(&bytes, |xml| {
        let index = xml.rfind("<w:headerReference").unwrap();
        let end = index + xml[index..].find("/>").unwrap() + 2;
        format!("{}{}", &xml[..index], &xml[end..])
    });
    for index in [0, 1] {
        refuse(
            inherited.clone(),
            &operation(
                &inherited,
                index,
                HeaderFooterKind::Header,
                HeaderFooterVariant::Default,
            ),
        );
    }
}
