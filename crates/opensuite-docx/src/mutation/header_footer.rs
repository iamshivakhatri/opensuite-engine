use super::*;

/// Inspects one default header/footer without exposing package implementation details.
pub fn inspect_header_footer(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    kind: HeaderFooterKind,
) -> Result<HeaderFooterInspection, OperationResult> {
    let section = main_section(source, false)?;
    let references = default_header_footer_references(source, section, kind);
    if references.is_empty() {
        return Ok(HeaderFooterInspection::None);
    }
    if references.len() != 1 {
        return Ok(HeaderFooterInspection::Unsupported);
    }
    let part = header_footer_part(package, main, source, references[0], kind)?;
    let bytes = package.read_part(&part).map_err(package_failure)?;
    let part_source = SourceDocument::parse(bytes).map_err(document_invalid)?;
    let content = simple_header_footer_content(&part_source, kind)
        .ok_or_else(|| unsupported("header/footer is not a supported simple part"))?;
    Ok(match (content.text, content.page_number) {
        (None, None) => HeaderFooterInspection::SimpleText(String::new()),
        (Some(text), None) => HeaderFooterInspection::SimpleText(text),
        (None, Some(alignment)) => HeaderFooterInspection::SimplePageNumber(alignment),
        (Some(text), Some(alignment)) => {
            HeaderFooterInspection::SimpleTextAndPageNumber(text, alignment)
        }
    })
}

/// Changes one default header/footer in the same safe single-section boundary as page setup.
pub fn set_header_footer_text_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetHeaderFooterText,
) -> Result<Vec<u8>, OperationResult> {
    let section = main_section(source, true)?;
    let before_page_setup = page_setup_from_section(source, section)?;
    let before_target = inspect_header_footer(package, main, source, operation.kind)?;
    let opposite =
        inspect_header_footer(package, main, source, other_header_footer(operation.kind))?;
    let references = default_header_footer_references(source, section, operation.kind);
    if references.len() > 1 {
        return Err(unsupported("default header/footer reference is ambiguous"));
    }
    let mut document = source.original_bytes().to_vec();
    let mut replaced: Vec<(PartName, Vec<u8>)> = Vec::new();
    let mut added: Vec<(PartName, Vec<u8>)> = Vec::new();
    if let Some(text) = &operation.text {
        if let Some(reference) = references.first().copied() {
            let part = header_footer_part(package, main, source, reference, operation.kind)?;
            let bytes = package.read_part(&part).map_err(package_failure)?;
            let part_source = SourceDocument::parse(bytes).map_err(document_invalid)?;
            let content = simple_header_footer_content(&part_source, operation.kind)
                .ok_or_else(|| unsupported("header/footer is not a supported simple part"))?;
            let patch = if content.text.is_some() {
                simple_header_footer_patch(&part_source, operation.kind, text)?
            } else {
                let at = source_end_tag_start(&part_source, part_source.root())?;
                Patch {
                    span: SourceSpan { start: at, end: at },
                    replacement: simple_header_footer_text_xml(&part_source, operation.kind, text)?
                        .into_bytes(),
                }
            };
            replaced.push((part.name, apply_patches(&part_source, vec![patch])?));
        } else {
            let relationship_id = add_header_footer_part(
                package,
                main,
                operation.kind,
                canonical_header_footer_xml(operation.kind, text).into_bytes(),
                &mut replaced,
                &mut added,
            )?;
            let reference =
                header_footer_reference_xml(source, section, operation.kind, &relationship_id)?;
            document = apply_patches(
                source,
                vec![section_reference_patch(
                    source,
                    section,
                    operation.kind,
                    reference,
                )?],
            )?;
        }
    } else if let Some(reference) = references.first().copied() {
        let part = header_footer_part(package, main, source, reference, operation.kind)?;
        let bytes = package.read_part(&part).map_err(package_failure)?;
        let part_source = SourceDocument::parse(bytes).map_err(document_invalid)?;
        let content = simple_header_footer_content(&part_source, operation.kind)
            .ok_or_else(|| unsupported("header/footer is not a supported simple part"))?;
        if content.page_number.is_some() {
            let paragraph = header_footer_text_paragraph(&part_source)
                .ok_or_else(|| unsupported("header/footer has no simple text paragraph"))?;
            replaced.push((
                part.name,
                apply_patches(
                    &part_source,
                    vec![Patch {
                        span: part_source
                            .node(paragraph)
                            .expect("paragraph exists")
                            .span(),
                        replacement: Vec::new(),
                    }],
                )?,
            ));
        } else {
            document = apply_patches(
                source,
                vec![Patch {
                    span: source.node(reference).expect("reference exists").span(),
                    replacement: Vec::new(),
                }],
            )?;
        }
    } else {
        return package
            .write_replaced_part_to_vec(main, &document)
            .map_err(package_failure);
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
    verify_header_footer_output(
        &output,
        operation,
        before_page_setup,
        before_target,
        opposite,
    )?;
    Ok(output)
}

pub fn set_header_footer_text(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetHeaderFooterText,
    output: impl AsRef<Path>,
) -> OperationResult {
    write_operation_output(
        package,
        output.as_ref(),
        set_header_footer_text_to_vec(package, main, source, operation),
    )
}

pub(super) fn default_header_footer_references(
    source: &SourceDocument,
    section: NodeId,
    kind: HeaderFooterKind,
) -> Vec<NodeId> {
    source
        .children(section)
        .filter(|id| {
            word(source, *id, header_footer_reference_name(kind))
                && source.node(*id).and_then(|node| node.attribute("type")) == Some("default")
        })
        .collect()
}

pub(super) fn header_footer_part(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    reference: NodeId,
    kind: HeaderFooterKind,
) -> Result<Part, OperationResult> {
    header_footer_part_from_relationships(
        package,
        &package.part_relationships(main).map_err(package_failure)?,
        source,
        reference,
        kind,
    )
}

pub(super) fn header_footer_part_from_relationships(
    package: &Package,
    relationships: &[opensuite_opc::Relationship],
    source: &SourceDocument,
    reference: NodeId,
    kind: HeaderFooterKind,
) -> Result<Part, OperationResult> {
    let id = source
        .node(reference)
        .and_then(|node| node.attribute("id"))
        .ok_or_else(|| unsupported("header/footer reference has no relationship id"))?;
    let relationship = relationships
        .iter()
        .find(|value| value.id.as_str() == id)
        .ok_or_else(|| unsupported("header/footer relationship is missing"))?;
    if relationship.relationship_type.as_str() != header_footer_relationship_type(kind) {
        return Err(unsupported(
            "header/footer relationship has an unexpected type",
        ));
    }
    let RelationshipTarget::Internal { part_name, .. } = &relationship.target else {
        return Err(unsupported(
            "header/footer relationship must target an internal part",
        ));
    };
    let part = package.part(part_name).map_err(package_failure)?;
    if part.content_type.as_str() != header_footer_content_type(kind) {
        return Err(unsupported(
            "header/footer part has an unexpected content type",
        ));
    }
    Ok(part)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SimpleHeaderFooterContent {
    pub(super) text: Option<String>,
    pub(super) page_number: Option<PageNumberAlignment>,
}

pub(super) fn simple_header_footer_content(
    source: &SourceDocument,
    kind: HeaderFooterKind,
) -> Option<SimpleHeaderFooterContent> {
    let root = source.root();
    if !word(source, root, header_footer_root_name(kind)) {
        return None;
    }
    let paragraphs = source.children(root).collect::<Vec<_>>();
    if paragraphs.len() > 2 || paragraphs.iter().any(|id| !word(source, *id, "p")) {
        return None;
    }
    let mut text = None;
    let mut page_number = None;
    for paragraph in paragraphs {
        if let Some(value) = simple_header_footer_paragraph_text(source, paragraph) {
            if text.replace(value).is_some() {
                return None;
            }
        } else if let Some(value) = page_number_alignment(source, paragraph) {
            if page_number.replace(value).is_some() {
                return None;
            }
        } else {
            return None;
        }
    }
    Some(SimpleHeaderFooterContent { text, page_number })
}

pub(super) fn simple_header_footer_paragraph_text(
    source: &SourceDocument,
    paragraph: NodeId,
) -> Option<String> {
    let runs = source
        .children(paragraph)
        .filter(|id| !word(source, *id, "pPr"))
        .collect::<Vec<_>>();
    if runs.len() != 1 || !word(source, runs[0], "r") {
        return None;
    }
    let values = source
        .children(runs[0])
        .filter(|id| !word(source, *id, "rPr"))
        .collect::<Vec<_>>();
    if values.len() != 1 || !word(source, values[0], "t") {
        return None;
    }
    let text = source.children(values[0]).collect::<Vec<_>>();
    if text.len() > 1
        || text.iter().any(|id| {
            !matches!(
                source.node(*id).map(|node| node.kind()),
                Some(SourceNodeKind::Text)
            )
        })
    {
        return None;
    }
    text.first()
        .and_then(|id| source.text_bytes(*id))
        .and_then(|bytes| quick_xml::escape::unescape(std::str::from_utf8(bytes).ok()?).ok())
        .map(|value| value.into_owned())
        .or_else(|| text.is_empty().then(String::new))
}

pub(super) fn header_footer_text_paragraph(source: &SourceDocument) -> Option<NodeId> {
    source
        .children(source.root())
        .find(|id| simple_header_footer_paragraph_text(source, *id).is_some())
}

pub(super) fn simple_header_footer_patch(
    source: &SourceDocument,
    kind: HeaderFooterKind,
    text: &str,
) -> Result<Patch, OperationResult> {
    simple_header_footer_content(source, kind)
        .ok_or_else(|| unsupported("header/footer is not a supported simple part"))?;
    let paragraph = header_footer_text_paragraph(source)
        .ok_or_else(|| unsupported("header/footer has no simple text paragraph"))?;
    let value = source
        .children(
            source
                .children(paragraph)
                .find(|id| word(source, *id, "r"))
                .expect("run"),
        )
        .find(|id| word(source, *id, "t"))
        .expect("text");
    let tag = qualify(word_prefix(source, paragraph)?, "t");
    let space = if requires_space_preservation(text) {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    Ok(Patch {
        span: source.node(value).expect("text exists").span(),
        replacement: format!("<{tag}{space}>{}</{tag}>", escape(text)).into_bytes(),
    })
}

pub(super) fn simple_header_footer_text_xml(
    source: &SourceDocument,
    kind: HeaderFooterKind,
    text: &str,
) -> Result<String, OperationResult> {
    let prefix = word_prefix_for(source, source.root(), header_footer_root_name(kind))?;
    let tag = qualify(&prefix, "p");
    let run = qualify(&prefix, "r");
    let value = qualify(&prefix, "t");
    let space = if requires_space_preservation(text) {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    Ok(format!(
        "<{tag}><{run}><{value}{space}>{}</{value}></{run}></{tag}>",
        escape(text)
    ))
}

pub(super) fn next_header_footer_part_name(
    package: &Package,
    kind: HeaderFooterKind,
) -> Result<PartName, OperationResult> {
    let stem = match kind {
        HeaderFooterKind::Header => "header",
        HeaderFooterKind::Footer => "footer",
    };
    let next = package
        .part_names_with_prefix("/word/")
        .filter_map(|name| {
            name.as_str()
                .strip_prefix(&format!("/word/{stem}"))
                .and_then(|value| value.strip_suffix(".xml"))
                .and_then(|value| value.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| {
            OperationResult::failed("PACKAGE_CONFLICT", "cannot allocate header/footer part")
        })?;
    PartName::parse(format!("/word/{stem}{next}.xml")).map_err(package_failure)
}

pub(super) fn relationship_xml(
    package: &Package,
    main: &Part,
) -> Result<(bool, Vec<u8>), OperationResult> {
    let name = relationship_part_name(main)?;
    match package.read_part_by_name(&name) { Ok(value) => Ok((true, value)), Err(PackageError::MissingTargetPart(_)) => Ok((false, b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"></Relationships>".to_vec())), Err(error) => Err(package_failure(error)) }
}

pub(super) fn section_reference_patch(
    source: &SourceDocument,
    section: NodeId,
    kind: HeaderFooterKind,
    reference: String,
) -> Result<Patch, OperationResult> {
    let children = source.children(section).collect::<Vec<_>>();
    let at = match kind {
        HeaderFooterKind::Header => children
            .iter()
            .find(|id| {
                word(source, **id, "footerReference") || !word(source, **id, "headerReference")
            })
            .copied(),
        HeaderFooterKind::Footer => children
            .iter()
            .find(|id| {
                !word(source, **id, "headerReference") && !word(source, **id, "footerReference")
            })
            .copied(),
    }
    .map(|id| source.node(id).expect("child exists").span().start)
    .unwrap_or_else(|| {
        source
            .node(section)
            .and_then(|node| match node.kind() {
                SourceNodeKind::Element { end_tag, .. } => end_tag.map(|span| span.start),
                _ => None,
            })
            .unwrap_or_else(|| source.node(section).unwrap().span().end - 2)
    });
    Ok(Patch {
        span: SourceSpan { start: at, end: at },
        replacement: reference.into_bytes(),
    })
}

pub(super) fn header_footer_reference_xml(
    source: &SourceDocument,
    section: NodeId,
    kind: HeaderFooterKind,
    id: &str,
) -> Result<String, OperationResult> {
    let prefix = word_prefix_for(source, section, "sectPr")?;
    Ok(format!(
        r#"<{} {}type="default" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="{id}"/>"#,
        qualify(&prefix, header_footer_reference_name(kind)),
        attr_prefix(&prefix)
    ))
}

pub(super) fn canonical_header_footer_xml(kind: HeaderFooterKind, text: &str) -> String {
    let root = header_footer_root_name(kind);
    let space = if requires_space_preservation(text) {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:{root} xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t{space}>{}</w:t></w:r></w:p></w:{root}>"#,
        escape(text)
    )
}

pub(super) fn source_end_tag_start(
    source: &SourceDocument,
    id: NodeId,
) -> Result<usize, OperationResult> {
    match source.node(id).expect("source node exists").kind() {
        SourceNodeKind::Element {
            end_tag: Some(span),
            ..
        } => Ok(span.start),
        _ => Err(unsupported("header/footer root has no end tag")),
    }
}

pub(super) fn header_footer_root_name(kind: HeaderFooterKind) -> &'static str {
    match kind {
        HeaderFooterKind::Header => "hdr",
        HeaderFooterKind::Footer => "ftr",
    }
}

pub(super) fn header_footer_reference_name(kind: HeaderFooterKind) -> &'static str {
    match kind {
        HeaderFooterKind::Header => "headerReference",
        HeaderFooterKind::Footer => "footerReference",
    }
}

pub(super) fn header_footer_relationship_type(kind: HeaderFooterKind) -> &'static str {
    match kind {
        HeaderFooterKind::Header => HEADER_RELATIONSHIP_TYPE,
        HeaderFooterKind::Footer => FOOTER_RELATIONSHIP_TYPE,
    }
}

pub(super) fn header_footer_content_type(kind: HeaderFooterKind) -> &'static str {
    match kind {
        HeaderFooterKind::Header => HEADER_CONTENT_TYPE,
        HeaderFooterKind::Footer => FOOTER_CONTENT_TYPE,
    }
}

pub(super) fn other_header_footer(kind: HeaderFooterKind) -> HeaderFooterKind {
    match kind {
        HeaderFooterKind::Header => HeaderFooterKind::Footer,
        HeaderFooterKind::Footer => HeaderFooterKind::Header,
    }
}

pub(super) fn package_failure(error: PackageError) -> OperationResult {
    OperationResult::failed(error.code(), error.to_string())
}

pub(super) fn verify_header_footer_output(
    output: &[u8],
    operation: &SetHeaderFooterText,
    page_setup: PageSetupInspection,
    before_target: HeaderFooterInspection,
    opposite: HeaderFooterInspection,
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(package_failure)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    if page_setup_from_section(&source, main_section(&source, true)?)? != page_setup
        || inspect_header_footer(
            &package,
            &main,
            &source,
            other_header_footer(operation.kind),
        )? != opposite
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output changed unrelated header/footer or page setup",
        ));
    }
    let actual = inspect_header_footer(&package, &main, &source, operation.kind)?;
    let page_number = match before_target {
        HeaderFooterInspection::SimplePageNumber(value)
        | HeaderFooterInspection::SimpleTextAndPageNumber(_, value) => Some(value),
        _ => None,
    };
    let expected = match (operation.text.as_ref(), page_number) {
        (Some(text), Some(alignment)) => {
            HeaderFooterInspection::SimpleTextAndPageNumber(text.clone(), alignment)
        }
        (Some(text), None) => HeaderFooterInspection::SimpleText(text.clone()),
        (None, Some(alignment)) => HeaderFooterInspection::SimplePageNumber(alignment),
        (None, None) => HeaderFooterInspection::None,
    };
    if actual != expected {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output header/footer does not match request",
        ));
    }
    Ok(())
}

pub(super) fn variant_name(value: HeaderFooterVariant) -> &'static str {
    match value {
        HeaderFooterVariant::Default => "default",
        HeaderFooterVariant::First => "first",
        HeaderFooterVariant::Even => "even",
    }
}

pub(super) fn variant_reference(
    source: &SourceDocument,
    section: NodeId,
    kind: HeaderFooterKind,
    variant: HeaderFooterVariant,
) -> Result<Option<NodeId>, OperationResult> {
    let mut refs = source.children(section).filter(|id| {
        word(source, *id, header_footer_reference_name(kind))
            && source.node(*id).and_then(|v| v.attribute("type")) == Some(variant_name(variant))
    });
    let first = refs.next();
    if refs.next().is_some() {
        return Err(unsupported("header/footer variant reference is ambiguous"));
    }
    Ok(first)
}

fn variant_reference_xml(
    source: &SourceDocument,
    section: NodeId,
    kind: HeaderFooterKind,
    variant: HeaderFooterVariant,
    id: &str,
) -> Result<String, OperationResult> {
    Ok(
        header_footer_reference_xml(source, section, kind, id)?.replace(
            "type=\"default\"",
            &format!("type=\"{}\"", variant_name(variant)),
        ),
    )
}

// The same OPC allocation path serves text, PAGE fields, and section-owned parts.
pub(super) fn add_header_footer_part(
    package: &Package,
    main: &Part,
    kind: HeaderFooterKind,
    bytes: Vec<u8>,
    replaced: &mut Vec<(PartName, Vec<u8>)>,
    added: &mut Vec<(PartName, Vec<u8>)>,
) -> Result<String, OperationResult> {
    let mut name = next_header_footer_part_name(package, kind)?;
    while added.iter().any(|(n, _)| n == &name) {
        let value = name.as_str();
        let stem = if kind == HeaderFooterKind::Header {
            "header"
        } else {
            "footer"
        };
        let number = value
            .strip_prefix(&format!("/word/{stem}"))
            .and_then(|v| v.strip_suffix(".xml"))
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap();
        name =
            PartName::parse(format!("/word/{stem}{}.xml", number + 1)).map_err(package_failure)?;
    }
    let rels_name = relationship_part_name(main)?;
    let (exists, original) = relationship_xml(package, main)?;
    let rels = replaced
        .iter()
        .chain(added.iter())
        .find(|(n, _)| n == &rels_name)
        .map(|(_, v)| v.as_slice())
        .unwrap_or(&original);
    let rels_source = SourceDocument::parse(rels.to_vec()).map_err(document_invalid)?;
    let used: HashSet<_> = rels_source
        .children(rels_source.root())
        .filter_map(|id| rels_source.node(id)?.attribute("Id"))
        .collect();
    let id = (1u32..)
        .map(|v| format!("rId{v}"))
        .find(|id| !used.contains(id.as_str()))
        .unwrap();
    let target = name.as_str().rsplit('/').next().unwrap();
    let rels = append_xml_element(
        rels,
        &format!(
            r#"<Relationship Id="{id}" Type="{}" Target="{target}"/>"#,
            header_footer_relationship_type(kind)
        ),
    )?;
    let list = if exists { &mut *replaced } else { &mut *added };
    if let Some((_, value)) = list.iter_mut().find(|(n, _)| n == &rels_name) {
        *value = rels;
    } else {
        list.push((rels_name, rels));
    }
    let types_name = PartName::parse("/[Content_Types].xml").map_err(package_failure)?;
    let original = package
        .read_part_by_name(&types_name)
        .map_err(package_failure)?;
    let types = replaced
        .iter()
        .find(|(n, _)| n == &types_name)
        .map(|(_, v)| v.as_slice())
        .unwrap_or(&original);
    let types = append_xml_element(
        types,
        &format!(
            r#"<Override PartName="{}" ContentType="{}"/>"#,
            name.as_str(),
            header_footer_content_type(kind)
        ),
    )?;
    if let Some((_, value)) = replaced.iter_mut().find(|(n, _)| n == &types_name) {
        *value = types;
    } else {
        replaced.push((types_name, types));
    }
    added.push((name, bytes));
    Ok(id)
}

pub fn set_section_header_footer_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetSectionHeaderFooter,
) -> Result<Vec<u8>, OperationResult> {
    use super::sections::*;
    let (ids, index) = resolve_section(package, main, source, &operation.target)?;
    let section = ids[index];
    if source.node(section).is_some_and(|node| matches!(node.kind(), SourceNodeKind::Element { name, .. } if name.namespace_uri()==Some(NS[1]))) {
        return Err(unsupported("section header/footer authoring supports transitional DOCX only"));
    }
    let own = variant_reference(source, section, operation.kind, operation.variant)?;
    let effective = ids[..=index]
        .iter()
        .rev()
        .find_map(
            |id| match variant_reference(source, *id, operation.kind, operation.variant) {
                Ok(Some(reference)) => Some(Ok(reference)),
                Err(error) => Some(Err(error)),
                _ => None,
            },
        )
        .transpose()?;
    let mut patches = Vec::new();
    let mut replaced = Vec::new();
    let mut added = Vec::new();
    let new_id = match &operation.change {
        SectionHeaderFooterChange::Inherit => {
            if index == 0 {
                return Err(OperationResult::failed(
                    "INVALID_OPERATION",
                    "first section has no previous section",
                ));
            }
            None
        }
        change => {
            if operation.variant == HeaderFooterVariant::Even
                && !odd_even_headers_enabled(package, main)?
            {
                return Err(unsupported(
                    "enable document-wide odd/even headers explicitly before editing the even variant",
                ));
            }
            let (old_part, bytes) = if let Some(reference) = effective {
                let part = header_footer_part(package, main, source, reference, operation.kind)?;
                let bytes = package.read_part(&part).map_err(package_failure)?;
                (Some(part), bytes)
            } else {
                (
                    None,
                    canonical_header_footer_xml(operation.kind, "").into_bytes(),
                )
            };
            let part_source = SourceDocument::parse(bytes).map_err(document_invalid)?;
            let content =
                simple_header_footer_content(&part_source, operation.kind).ok_or_else(|| {
                    unsupported("section header/footer is not a supported simple part")
                })?;
            let bytes = match change {
                SectionHeaderFooterChange::Unlink => part_source.original_bytes().to_vec(),
                SectionHeaderFooterChange::SetText(text) => {
                    let patch = if content.text.is_some() {
                        simple_header_footer_patch(&part_source, operation.kind, text)?
                    } else {
                        let at = source_end_tag_start(&part_source, part_source.root())?;
                        Patch {
                            span: SourceSpan { start: at, end: at },
                            replacement: simple_header_footer_text_xml(
                                &part_source,
                                operation.kind,
                                text,
                            )?
                            .into_bytes(),
                        }
                    };
                    apply_patches(&part_source, vec![patch])?
                }
                SectionHeaderFooterChange::SetPageNumber(alignment) => {
                    let patch = match (alignment, content.page_number) {
                        (Some(alignment), Some(_)) => {
                            page_number_alignment_patch(&part_source, *alignment)?
                        }
                        (Some(alignment), None) => {
                            let at = source_end_tag_start(&part_source, part_source.root())?;
                            Patch {
                                span: SourceSpan { start: at, end: at },
                                replacement: page_number_xml(&part_source, *alignment)?
                                    .into_bytes(),
                            }
                        }
                        (None, Some(_)) => Patch {
                            span: part_source
                                .node(page_number_paragraph(&part_source).unwrap())
                                .unwrap()
                                .span(),
                            replacement: Vec::new(),
                        },
                        (None, None) => {
                            return Err(OperationResult::failed(
                                "TARGET_NOT_FOUND",
                                "section header/footer has no PAGE field",
                            ));
                        }
                    };
                    apply_patches(&part_source, vec![patch])?
                }
                SectionHeaderFooterChange::Inherit => unreachable!(),
            };
            // Only edit an existing part when nobody else owns or inherits it.
            let part_shared = if let Some(part) = &old_part {
                let relationships = package.part_relationships(main).map_err(package_failure)?;
                let same_part_ids: HashSet<_> = relationships.iter().filter(|rel| matches!(&rel.target, RelationshipTarget::Internal { part_name, .. } if part_name==&part.name)).map(|rel| rel.id.as_str()).collect();
                ids.get(index + 1).is_some_and(|next| {
                    variant_reference(source, *next, operation.kind, operation.variant)
                        .ok()
                        .flatten()
                        .is_none()
                }) || source
                    .node_ids()
                    .filter(|id| {
                        word(source, *id, "headerReference") || word(source, *id, "footerReference")
                    })
                    .filter(|id| Some(*id) != own)
                    .any(|id| {
                        source
                            .node(id)
                            .and_then(|n| n.attribute("id"))
                            .is_some_and(|id| same_part_ids.contains(id))
                    })
            } else {
                false
            };
            if own.is_some() && !part_shared {
                let part = old_part.unwrap();
                replaced.push((part.name, bytes));
                Some(
                    source
                        .node(own.unwrap())
                        .unwrap()
                        .attribute("id")
                        .unwrap()
                        .to_owned(),
                )
            } else {
                if let Some(part) = old_part {
                    let has_relationships = match package.part_relationships(&part) {
                        Ok(relationships) => !relationships.is_empty(),
                        Err(PackageError::MissingPartRelationships(_)) => false,
                        Err(error) => return Err(package_failure(error)),
                    };
                    if has_relationships {
                        return Err(unsupported(
                            "cannot copy a header/footer with its own relationships",
                        ));
                    }
                }
                Some(add_header_footer_part(
                    package,
                    main,
                    operation.kind,
                    bytes,
                    &mut replaced,
                    &mut added,
                )?)
            }
        }
    };
    let reference = new_id
        .as_ref()
        .map(|id| variant_reference_xml(source, section, operation.kind, operation.variant, id))
        .transpose()?
        .unwrap_or_default();
    if let Some(own) = own {
        patches.push(Patch {
            span: source.node(own).unwrap().span(),
            replacement: reference.into_bytes(),
        });
    } else if !reference.is_empty() {
        patches.push(section_reference_patch(
            source,
            section,
            operation.kind,
            reference,
        )?);
    }
    // Preserve the following section's previous effective content when its reference
    // is absent. An explicit link request changes only the selected section as well.
    if let Some(next) = ids.get(index + 1).copied() {
        if variant_reference(source, next, operation.kind, operation.variant)?.is_none() {
            let id = if let Some(reference) = effective {
                source
                    .node(reference)
                    .unwrap()
                    .attribute("id")
                    .unwrap()
                    .to_owned()
            } else {
                add_header_footer_part(
                    package,
                    main,
                    operation.kind,
                    canonical_header_footer_xml(operation.kind, "").into_bytes(),
                    &mut replaced,
                    &mut added,
                )?
            };
            patches.push(section_reference_patch(
                source,
                next,
                operation.kind,
                variant_reference_xml(source, next, operation.kind, operation.variant, &id)?,
            )?);
        }
    }
    replaced.push((
        main.name.clone(),
        apply_patches(source, section_patches(source, patches)?)?,
    ));
    let replaced_refs: Vec<_> = replaced
        .iter()
        .map(|(n, b)| (n.clone(), b.as_slice()))
        .collect();
    let added_refs: Vec<_> = added
        .iter()
        .map(|(n, b)| (n.clone(), b.as_slice()))
        .collect();
    let output = package
        .write_package_with_named_changes_to_vec(&replaced_refs, &added_refs)
        .map_err(package_failure)?;
    let check = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    check.verify().map_err(document_invalid)?;
    let (check_main, after) = crate::open_main_source(&check).map_err(document_invalid)?;
    if body_block_signatures(source)? != body_block_signatures(&after)? {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "header/footer edit changed body content",
        ));
    }
    let before = inspect_sections(package, main, source)?;
    let actual = inspect_sections(&check, &check_main, &after)?;
    if before.len() != actual.len() {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "header/footer edit changed section count",
        ));
    }
    let selected = actual[index]
        .headers_footers
        .iter()
        .find(|item| {
            item.kind
                == if operation.kind == HeaderFooterKind::Header {
                    "header"
                } else {
                    "footer"
                }
                && item.variant == variant_name(operation.variant)
        })
        .unwrap();
    let matches = match &operation.change {
        SectionHeaderFooterChange::Inherit => selected.linked_to_previous,
        SectionHeaderFooterChange::Unlink => !selected.linked_to_previous,
        SectionHeaderFooterChange::SetText(text) => {
            selected.text.as_deref() == Some(text.as_str()) && !selected.linked_to_previous
        }
        SectionHeaderFooterChange::SetPageNumber(alignment) => {
            selected.page_number_alignment.as_deref() == alignment.map(page_number_alignment_name)
        }
    };
    if !matches {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "section header/footer does not match request",
        ));
    }
    for (other, (before, after)) in before.iter().zip(&actual).enumerate() {
        for (a, b) in before.headers_footers.iter().zip(&after.headers_footers) {
            if (other != index
                || a.kind
                    != if operation.kind == HeaderFooterKind::Header {
                        "header"
                    } else {
                        "footer"
                    }
                || a.variant != variant_name(operation.variant))
                && (a.text.as_deref().unwrap_or("") != b.text.as_deref().unwrap_or("")
                    || a.page_number_alignment != b.page_number_alignment
                    || a.supported != b.supported)
            {
                return Err(OperationResult::failed(
                    "DOCUMENT_INVALID",
                    "header/footer edit changed another variant or section",
                ));
            }
        }
    }
    Ok(output)
}
