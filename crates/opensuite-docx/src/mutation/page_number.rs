use super::*;

/// Inspects one supported default header/footer page number without exposing field XML.
pub fn inspect_page_number(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
) -> Result<PageNumberInspection, OperationResult> {
    let mut found = None;
    for kind in [HeaderFooterKind::Header, HeaderFooterKind::Footer] {
        match inspect_header_footer(package, main, source, kind)? {
            HeaderFooterInspection::SimplePageNumber(alignment)
            | HeaderFooterInspection::SimpleTextAndPageNumber(_, alignment) => {
                if found.replace((kind, alignment)).is_some() {
                    return Ok(PageNumberInspection::Unsupported);
                }
            }
            HeaderFooterInspection::Unsupported => return Ok(PageNumberInspection::Unsupported),
            HeaderFooterInspection::None | HeaderFooterInspection::SimpleText(_) => {}
        }
    }
    Ok(match found {
        None => PageNumberInspection::None,
        Some((HeaderFooterKind::Header, alignment)) => PageNumberInspection::Header(alignment),
        Some((HeaderFooterKind::Footer, alignment)) => PageNumberInspection::Footer(alignment),
    })
}

/// Creates, updates, or removes one canonical PAGE field in a default header/footer.
pub fn set_page_number_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetPageNumber,
) -> Result<Vec<u8>, OperationResult> {
    let section = main_section(source, true)?;
    let before_page_setup = page_setup_from_section(source, section)?;
    let before_body = body_block_signatures(source)?;
    let opposite =
        inspect_header_footer(package, main, source, other_header_footer(operation.kind))?;
    let references = default_header_footer_references(source, section, operation.kind);
    if references.len() > 1 {
        return Err(unsupported("default header/footer reference is ambiguous"));
    }
    let mut document = source.original_bytes().to_vec();
    let mut replaced: Vec<(PartName, Vec<u8>)> = Vec::new();
    let mut added: Vec<(PartName, Vec<u8>)> = Vec::new();
    if let Some(reference) = references.first().copied() {
        let part = header_footer_part(package, main, source, reference, operation.kind)?;
        let part_source = SourceDocument::parse(package.read_part(&part).map_err(package_failure)?)
            .map_err(document_invalid)?;
        let content = simple_header_footer_content(&part_source, operation.kind)
            .ok_or_else(|| unsupported("header/footer is not a supported simple part"))?;
        let patch = match (operation.alignment, content.page_number) {
            (Some(alignment), Some(_)) => page_number_alignment_patch(&part_source, alignment)?,
            (Some(alignment), None) => {
                let at = source_end_tag_start(&part_source, part_source.root())?;
                Patch {
                    span: SourceSpan { start: at, end: at },
                    replacement: page_number_xml(&part_source, alignment)?.into_bytes(),
                }
            }
            (None, Some(_)) => {
                let paragraph =
                    page_number_paragraph(&part_source).expect("supported page number exists");
                Patch {
                    span: part_source
                        .node(paragraph)
                        .expect("paragraph exists")
                        .span(),
                    replacement: Vec::new(),
                }
            }
            (None, None) => {
                return Err(OperationResult::failed(
                    "TARGET_NOT_FOUND",
                    "header/footer has no supported page number",
                ));
            }
        };
        replaced.push((part.name, apply_patches(&part_source, vec![patch])?));
    } else if let Some(alignment) = operation.alignment {
        let part_name = next_header_footer_part_name(package, operation.kind)?;
        let relationship_id = next_relationship_id(package, main)?;
        document = apply_patches(
            source,
            vec![section_reference_patch(
                source,
                section,
                operation.kind,
                header_footer_reference_xml(source, section, operation.kind, &relationship_id)?,
            )?],
        )?;
        let rels_name = relationship_part_name(main)?;
        let (exists, rels) = relationship_xml(package, main)?;
        let target = part_name
            .as_str()
            .rsplit('/')
            .next()
            .expect("part file name");
        let relation = format!(
            r#"<Relationship Id="{relationship_id}" Type="{}" Target="{target}"/>"#,
            header_footer_relationship_type(operation.kind)
        );
        let rels = append_xml_element(&rels, &relation)?;
        if exists {
            replaced.push((rels_name, rels));
        } else {
            added.push((rels_name, rels));
        }
        added.push((
            part_name.clone(),
            canonical_page_number_header_footer_xml(operation.kind, alignment).into_bytes(),
        ));
        let types_name = PartName::parse("/[Content_Types].xml").map_err(package_failure)?;
        let types = package
            .read_part_by_name(&types_name)
            .map_err(package_failure)?;
        let marker = format!(r#"PartName="{}""#, part_name.as_str());
        if !types
            .windows(marker.len())
            .any(|value| value == marker.as_bytes())
        {
            replaced.push((
                types_name,
                append_xml_element(
                    &types,
                    &format!(
                        r#"<Override PartName="{}" ContentType="{}"/>"#,
                        part_name.as_str(),
                        header_footer_content_type(operation.kind)
                    ),
                )?,
            ));
        }
    } else {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "header/footer has no supported page number",
        ));
    }
    replaced.push((main.name.clone(), document));
    let replaced_refs = replaced
        .iter()
        .map(|(name, bytes)| (name.clone(), bytes.as_slice()))
        .collect::<Vec<_>>();
    let added_refs = added
        .iter()
        .map(|(name, bytes)| (name.clone(), bytes.as_slice()))
        .collect::<Vec<_>>();
    let output = package
        .write_package_with_named_changes_to_vec(&replaced_refs, &added_refs)
        .map_err(package_failure)?;
    verify_page_number_output(&output, operation, before_page_setup, before_body, opposite)?;
    Ok(output)
}

pub fn set_page_number(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetPageNumber,
    output: impl AsRef<Path>,
) -> OperationResult {
    write_operation_output(
        package,
        output.as_ref(),
        set_page_number_to_vec(package, main, source, operation),
    )
}

pub(super) fn page_number_alignment(
    source: &SourceDocument,
    paragraph: NodeId,
) -> Option<PageNumberAlignment> {
    let children = source.children(paragraph).collect::<Vec<_>>();
    if children.len() != 2
        || !word(source, children[0], "pPr")
        || !word(source, children[1], "fldSimple")
    {
        return None;
    }
    let ppr = source.children(children[0]).collect::<Vec<_>>();
    if ppr.len() != 1 || !word(source, ppr[0], "jc") {
        return None;
    }
    let alignment = match source.node(ppr[0])?.attribute("val")? {
        "left" => PageNumberAlignment::Left,
        "center" => PageNumberAlignment::Center,
        "right" => PageNumberAlignment::Right,
        _ => return None,
    };
    let field = source.node(children[1])?;
    (field.attribute("instr") == Some(" PAGE ") && source.children(children[1]).next().is_none())
        .then_some(alignment)
}

pub(super) fn page_number_paragraph(source: &SourceDocument) -> Option<NodeId> {
    source
        .children(source.root())
        .find(|id| page_number_alignment(source, *id).is_some())
}

pub(super) fn page_number_xml(
    source: &SourceDocument,
    alignment: PageNumberAlignment,
) -> Result<String, OperationResult> {
    let root = source.root();
    let local = source
        .node(root)
        .and_then(|node| match node.kind() {
            SourceNodeKind::Element { name, .. } => Some(name.local_name()),
            _ => None,
        })
        .ok_or_else(|| unsupported("header/footer root is invalid"))?;
    let prefix = word_prefix_for(source, root, local)?;
    let p = qualify(&prefix, "p");
    let ppr = qualify(&prefix, "pPr");
    let jc = qualify(&prefix, "jc");
    let field = qualify(&prefix, "fldSimple");
    let attr = attr_prefix(&prefix);
    Ok(format!(
        "<{p}><{ppr}><{jc} {attr}val=\"{}\"/></{ppr}><{field} {attr}instr=\" PAGE \"></{field}></{p}>",
        page_number_alignment_name(alignment)
    ))
}

pub(super) fn page_number_alignment_name(value: PageNumberAlignment) -> &'static str {
    match value {
        PageNumberAlignment::Left => "left",
        PageNumberAlignment::Center => "center",
        PageNumberAlignment::Right => "right",
    }
}

pub(super) fn canonical_page_number_header_footer_xml(
    kind: HeaderFooterKind,
    alignment: PageNumberAlignment,
) -> String {
    let root = header_footer_root_name(kind);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:{root} xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:pPr><w:jc w:val="{}"/></w:pPr><w:fldSimple w:instr=" PAGE "></w:fldSimple></w:p></w:{root}>"#,
        page_number_alignment_name(alignment)
    )
}

pub(super) fn page_number_alignment_patch(
    source: &SourceDocument,
    alignment: PageNumberAlignment,
) -> Result<Patch, OperationResult> {
    let paragraph = page_number_paragraph(source)
        .ok_or_else(|| unsupported("header/footer has no supported page number"))?;
    let jc = source
        .children(
            source
                .children(paragraph)
                .find(|id| word(source, *id, "pPr"))
                .expect("pPr"),
        )
        .find(|id| word(source, *id, "jc"))
        .expect("jc");
    let prefix = word_prefix_for(source, jc, "jc")?;
    let tag = qualify(&prefix, "jc");
    let attr = attr_prefix(&prefix);
    Ok(Patch {
        span: source.node(jc).expect("jc exists").span(),
        replacement: format!(
            "<{tag} {attr}val=\"{}\"/>",
            page_number_alignment_name(alignment)
        )
        .into_bytes(),
    })
}

pub(super) fn verify_page_number_output(
    output: &[u8],
    operation: &SetPageNumber,
    page_setup: PageSetupInspection,
    body: Vec<(String, String)>,
    opposite: HeaderFooterInspection,
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(package_failure)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    if page_setup_from_section(&source, main_section(&source, true)?)? != page_setup
        || body_block_signatures(&source)? != body
        || inspect_header_footer(
            &package,
            &main,
            &source,
            other_header_footer(operation.kind),
        )? != opposite
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output changed unrelated document content",
        ));
    }
    let actual = inspect_header_footer(&package, &main, &source, operation.kind)?;
    let alignment = match actual {
        HeaderFooterInspection::SimplePageNumber(value)
        | HeaderFooterInspection::SimpleTextAndPageNumber(_, value) => Some(value),
        _ => None,
    };
    if alignment != operation.alignment {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output page number does not match request",
        ));
    }
    Ok(())
}
