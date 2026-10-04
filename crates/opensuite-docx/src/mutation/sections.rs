use super::*;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionInspection {
    pub handle: String,
    pub index: usize,
    pub break_type: String,
    pub page_width_twips: Option<u32>,
    pub page_height_twips: Option<u32>,
    pub orientation: Option<String>,
    pub margins_twips: std::collections::BTreeMap<String, i32>,
    pub column_count: Option<u16>,
    pub different_first_page: bool,
    pub odd_even_headers: bool,
    pub page_number_start: Option<u32>,
    pub page_number_format: Option<String>,
    pub headers_footers: Vec<SectionHeaderFooterInspection>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionHeaderFooterInspection {
    pub kind: String,
    pub variant: String,
    pub linked_to_previous: bool,
    pub relationship_id: Option<String>,
    pub part_name: Option<String>,
    pub text: Option<String>,
    pub page_number_alignment: Option<String>,
    pub supported: bool,
}

pub fn inspect_sections(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
) -> Result<Vec<SectionInspection>, OperationResult> {
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    let sections: Vec<_> = document.sections().collect();
    let fingerprint = section_fingerprint(package, main, source)?;
    let odd_even_headers = odd_even_headers_enabled(package, main)?;
    let mut result = Vec::new();
    // Cache shared parts once; inheritance does not reparse the document or header part.
    let mut parts = std::collections::HashMap::new();
    let relationships = package.part_relationships(main).map_err(package_failure)?;
    let mut effective = std::collections::HashMap::new();
    for (index, section) in sections.iter().enumerate() {
        let id = section.source_id();
        let properties = section.properties();
        let size = properties.page_size().map_err(document_invalid)?;
        let margins = properties.page_margins().map_err(document_invalid)?;
        let columns = properties.columns().map_err(document_invalid)?;
        let number = section_child(source, id, "pgNumType")?.and_then(|id| source.node(id));
        let mut headers_footers = Vec::new();
        for kind in [HeaderFooterKind::Header, HeaderFooterKind::Footer] {
            for variant in [
                HeaderFooterVariant::Default,
                HeaderFooterVariant::First,
                HeaderFooterVariant::Even,
            ] {
                let key = (header_footer_root_name(kind), variant_name(variant));
                let owned = variant_reference(source, id, kind, variant)?;
                if let Some(reference) = owned {
                    effective.insert(key, reference);
                }
                let reference = effective.get(&key).copied();
                let mut item = SectionHeaderFooterInspection {
                    kind: if kind == HeaderFooterKind::Header {
                        "header"
                    } else {
                        "footer"
                    }
                    .into(),
                    variant: variant_name(variant).into(),
                    linked_to_previous: index > 0 && owned.is_none(),
                    relationship_id: reference
                        .and_then(|id| source.node(id)?.attribute("id").map(str::to_owned)),
                    part_name: None,
                    text: None,
                    page_number_alignment: None,
                    supported: true,
                };
                if let Some(reference) = reference {
                    let relationship_id = (
                        header_footer_root_name(kind),
                        item.relationship_id.clone().unwrap_or_default(),
                    );
                    let (part_name, content) = if let Some(cached) = parts.get(&relationship_id) {
                        cached
                    } else {
                        let part = header_footer_part_from_relationships(
                            package,
                            &relationships,
                            source,
                            reference,
                            kind,
                        )?;
                        let part_source = SourceDocument::parse(
                            package.read_part(&part).map_err(package_failure)?,
                        )
                        .map_err(document_invalid)?;
                        parts.entry(relationship_id).or_insert((
                            part.name.as_str().to_owned(),
                            simple_header_footer_content(&part_source, kind),
                        ))
                    };
                    item.part_name = Some(part_name.clone());
                    item.supported = content.is_some();
                    if let Some(content) = content {
                        item.text = content.text.clone();
                        item.page_number_alignment = content
                            .page_number
                            .map(|value| page_number_alignment_name(value).into());
                    }
                }
                headers_footers.push(item);
            }
        }
        let mut margins_twips = std::collections::BTreeMap::new();
        if let Some(margins) = margins {
            for (name, value) in [
                ("top", margins.top_twips),
                ("bottom", margins.bottom_twips),
                ("left", margins.left_twips),
                ("right", margins.right_twips),
                ("header", margins.header_twips),
                ("footer", margins.footer_twips),
                ("gutter", margins.gutter_twips),
            ] {
                if let Some(value) = value {
                    margins_twips.insert(name.into(), value);
                }
            }
        }
        result.push(SectionInspection {
            handle: format!("s{index}:{fingerprint:016x}"),
            index,
            break_type: properties
                .section_type()
                .map(|value| match value {
                    crate::SectionType::NextPage => "nextPage".into(),
                    crate::SectionType::Continuous => "continuous".into(),
                    crate::SectionType::OddPage => "oddPage".into(),
                    crate::SectionType::EvenPage => "evenPage".into(),
                    crate::SectionType::Unknown(value) => value,
                })
                .unwrap_or_else(|| "nextPage".into()),
            page_width_twips: size.as_ref().and_then(|v| v.width_twips),
            page_height_twips: size.as_ref().and_then(|v| v.height_twips),
            orientation: size.and_then(|v| v.orientation).map(|v| match v {
                crate::PageOrientation::Portrait => "portrait".into(),
                crate::PageOrientation::Landscape => "landscape".into(),
                crate::PageOrientation::Unknown(value) => value,
            }),
            margins_twips,
            column_count: columns.and_then(|v| v.count),
            different_first_page: section_child(source, id, "titlePg")?
                .is_some_and(|id| on_off(source, id)),
            odd_even_headers,
            page_number_start: number
                .and_then(|v| v.attribute("start"))
                .and_then(|v| v.parse().ok()),
            page_number_format: number.and_then(|v| v.attribute("fmt")).map(str::to_owned),
            headers_footers,
        });
    }
    Ok(result)
}

// Fixed FNV-1a fingerprint: deterministic, version-local safety, not a security identity.
fn section_fingerprint(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
) -> Result<u64, OperationResult> {
    let mut hash = 0xcbf29ce484222325u64;
    let mut add = |bytes: &[u8]| {
        for byte in bytes {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
    };
    add(source.original_bytes());
    let (_, relationships) = relationship_xml(package, main)?;
    add(&relationships);
    let mut names = std::collections::BTreeSet::new();
    for rel in package.part_relationships(main).map_err(package_failure)? {
        if ["/header", "/footer", "/settings"]
            .iter()
            .any(|suffix| rel.relationship_type.as_str().ends_with(suffix))
        {
            if let RelationshipTarget::Internal { part_name, .. } = rel.target {
                names.insert(part_name.as_str().to_owned());
            }
        }
    }
    for name in names {
        add(name.as_bytes());
        add(&package
            .read_part_by_name(&PartName::parse(name).map_err(package_failure)?)
            .map_err(package_failure)?);
    }
    Ok(hash)
}

pub(super) fn safe_sections(source: &SourceDocument) -> Result<Vec<NodeId>, OperationResult> {
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    let ids: Vec<_> = document.sections().map(|s| s.source_id()).collect();
    if ids.is_empty()
        || source.children(document.body_id()).last() != ids.last().copied()
        || source
            .node_ids()
            .filter(|id| word(source, *id, "sectPr"))
            .count()
            != ids.len()
    {
        return Err(unsupported(
            "section editing requires direct body boundaries and one final body section",
        ));
    }
    for id in &ids {
        if section_child(source, *id, "sectPrChange")?.is_some() {
            return Err(unsupported("tracked section properties cannot be edited"));
        }
        // A repeated section property must never choose the first silently.
        let mut names = HashSet::new();
        for child in source.children(*id) {
            if let Some(SourceNodeKind::Element { name, .. }) = source.node(child).map(|n| n.kind())
            {
                if name.namespace_uri().is_some_and(|uri| NS.contains(&uri))
                    && !matches!(name.local_name(), "headerReference" | "footerReference")
                    && !names.insert(name.local_name())
                {
                    return Err(unsupported("section properties contain duplicate children"));
                }
            }
        }
    }
    Ok(ids)
}

pub(super) fn resolve_section(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    target: &SectionTarget,
) -> Result<(Vec<NodeId>, usize), OperationResult> {
    let ids = safe_sections(source)?;
    let (position, stamp) = target.handle.split_once(':').ok_or_else(|| {
        OperationResult::failed("PRECONDITION_FAILED", "section handle is malformed")
    })?;
    let index = position
        .strip_prefix('s')
        .and_then(|v| v.parse::<usize>().ok())
        .ok_or_else(|| {
            OperationResult::failed("PRECONDITION_FAILED", "section handle is malformed")
        })?;
    if stamp != format!("{:016x}", section_fingerprint(package, main, source)?) {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "section handle is stale; inspect sections again",
        ));
    }
    if index >= ids.len() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "section handle was not found",
        ));
    }
    Ok((ids, index))
}

pub(super) fn section_child(
    source: &SourceDocument,
    section: NodeId,
    name: &str,
) -> Result<Option<NodeId>, OperationResult> {
    let mut children = source
        .children(section)
        .filter(|id| word(source, *id, name));
    let first = children.next();
    if children.next().is_some() {
        return Err(unsupported(format!("duplicate {name} section property")));
    }
    Ok(first)
}

pub(super) fn section_value_patch(
    source: &SourceDocument,
    section: NodeId,
    name: &str,
    xml: String,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    section_property_patch(
        source,
        section,
        section_child(source, section, name)?,
        name,
        xml,
        patches,
    )
}

pub(super) fn break_type_name(value: SectionBreakType) -> &'static str {
    match value {
        SectionBreakType::NextPage => "nextPage",
        SectionBreakType::Continuous => "continuous",
        SectionBreakType::OddPage => "oddPage",
        SectionBreakType::EvenPage => "evenPage",
    }
}

pub fn insert_section_break_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertSectionBreak,
) -> Result<Vec<u8>, OperationResult> {
    let ids = safe_sections(source)?;
    let placement = resolve_paragraph_placement(source, &operation.placement)?;
    let (body, blocks) = direct_body_blocks(source)?;
    let index = placement_index(&blocks, placement)?;
    let at = body_insertion(source, body, &blocks, index)?;
    let section = ids
        .iter()
        .copied()
        .find(|id| source.node(*id).unwrap().span().start >= at)
        .ok_or_else(|| unsupported("section boundary is not inside a supported section"))?;
    let prefix = word_prefix_for(source, body, "body")?;
    let node = source.node(section).unwrap();
    let properties =
        std::str::from_utf8(&source.original_bytes()[node.span().start..node.span().end])
            .map_err(document_invalid)?;
    // Keep namespace declarations in their original scope by rejecting local declarations
    // on ancestors that would not cover a new direct body paragraph.
    for ancestor in [
        node.parent(),
        node.parent().and_then(|id| source.node(id)?.parent()),
    ]
    .into_iter()
    .flatten()
    {
        if ancestor != body && ancestor != source.root() {
            if let SourceNodeKind::Element { start_tag, .. } = source.node(ancestor).unwrap().kind()
            {
                if source.original_bytes()[start_tag.start..start_tag.end]
                    .windows(5)
                    .any(|bytes| bytes == b"xmlns")
                {
                    return Err(unsupported(
                        "section boundary has local namespace declarations",
                    ));
                }
            }
        }
    }
    let fragment = format!(
        "<{p}><{ppr}>{properties}</{ppr}></{p}>",
        p = qualify(&prefix, "p"),
        ppr = qualify(&prefix, "pPr")
    );
    let mut patches = vec![Patch {
        span: SourceSpan { start: at, end: at },
        replacement: fragment.into_bytes(),
    }];
    section_value_patch(
        source,
        section,
        "type",
        format!(
            "<{} {}val=\"{}\"/>",
            qualify(&prefix, "type"),
            attr_prefix(&prefix),
            break_type_name(operation.break_type)
        ),
        &mut patches,
    )?;
    let output = write_patches_to_vec(package, main, source, section_patches(source, patches)?)?;
    let after_package = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    let (_, after) = crate::open_main_source(&after_package).map_err(document_invalid)?;
    let mut expected = body_block_signatures(source)?;
    expected.insert(index, ("paragraph".into(), String::new()));
    if safe_sections(&after)?.len() != ids.len() + 1 || body_block_signatures(&after)? != expected {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "section insertion changed body content or section count",
        ));
    }
    Ok(output)
}

pub fn set_section_properties_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetSectionProperties,
) -> Result<Vec<u8>, OperationResult> {
    if operation.page_setup.is_none()
        && operation.different_first_page.is_none()
        && operation.break_type.is_none()
        && operation.page_number_start.is_none()
    {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "section properties requires a change",
        ));
    }
    let (ids, index) = resolve_section(package, main, source, &operation.target)?;
    let section = ids[index];
    let prefix = word_prefix_for(source, section, "sectPr")?;
    let mut patches = Vec::new();
    if let Some(setup) = &operation.page_setup {
        patches.extend(page_setup_patches(source, section, setup)?);
    }
    if let Some(value) = operation.different_first_page {
        section_value_patch(
            source,
            section,
            "titlePg",
            format!(
                "<{} {}val=\"{}\"/>",
                qualify(&prefix, "titlePg"),
                attr_prefix(&prefix),
                u8::from(value)
            ),
            &mut patches,
        )?;
    }
    if let Some(value) = operation.break_type {
        section_value_patch(
            source,
            section,
            "type",
            format!(
                "<{} {}val=\"{}\"/>",
                qualify(&prefix, "type"),
                attr_prefix(&prefix),
                break_type_name(value)
            ),
            &mut patches,
        )?;
    }
    if let Some(change) = &operation.page_number_start {
        let number = section_child(source, section, "pgNumType")?;
        let value = match change {
            PropertyPatch::Set(value) => Some(value.to_string()),
            PropertyPatch::Clear => None,
        };
        if let Some(number) = number {
            let SourceNodeKind::Element { start_tag, .. } = source.node(number).unwrap().kind()
            else {
                unreachable!()
            };
            let number_prefix = word_prefix_for(source, number, "pgNumType")?;
            let tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
                .map_err(document_invalid)?;
            patches.push(Patch {
                span: *start_tag,
                replacement: edit_attribute(
                    tag.to_owned(),
                    &format!("{}start", attr_prefix(&number_prefix)),
                    value.as_deref(),
                )
                .into_bytes(),
            });
        } else if let Some(value) = value {
            section_value_patch(
                source,
                section,
                "pgNumType",
                format!(
                    "<{} {}start=\"{value}\"/>",
                    qualify(&prefix, "pgNumType"),
                    attr_prefix(&prefix)
                ),
                &mut patches,
            )?;
        }
    }
    let output = write_patches_to_vec(package, main, source, section_patches(source, patches)?)?;
    let after = verify_section_edit(&output, source, index)?;
    let after_section = safe_sections(&after)?[index];
    if let Some(setup) = &operation.page_setup {
        let before = page_setup_from_section(source, section)?;
        let actual = page_setup_from_section(&after, after_section)?;
        if setup
            .orientation
            .is_some_and(|value| value != actual.orientation)
            || setup.paper_size.is_some() && actual.paper_size != setup.paper_size
            || setup.margins.is_some()
                && actual.margins != merged_margins(before.margins, setup.margins.as_ref())?
        {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "section page setup does not match request",
            ));
        }
    }
    if operation.different_first_page.is_some_and(|value| {
        value
            != section_child(&after, after_section, "titlePg")
                .ok()
                .flatten()
                .is_some_and(|id| on_off(&after, id))
    }) {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "section first-page behavior does not match request",
        ));
    }
    if let Some(value) = operation.break_type {
        let actual = section_child(&after, after_section, "type")?
            .and_then(|id| after.node(id)?.attribute("val"));
        if actual != Some(break_type_name(value)) {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "section break type does not match request",
            ));
        }
    }
    if let Some(change) = &operation.page_number_start {
        let actual = section_child(&after, after_section, "pgNumType")?
            .and_then(|id| after.node(id)?.attribute("start"))
            .and_then(|v| v.parse::<u32>().ok());
        let expected = match change {
            PropertyPatch::Set(value) => Some(*value),
            PropertyPatch::Clear => None,
        };
        if actual != expected {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "section page numbering does not match request",
            ));
        }
    }
    Ok(output)
}

pub(super) fn verify_section_edit(
    output: &[u8],
    before: &SourceDocument,
    selected: usize,
) -> Result<SourceDocument, OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, after) = crate::open_main_source(&package).map_err(document_invalid)?;
    let old = safe_sections(before)?;
    let new = safe_sections(&after)?;
    if old.len() != new.len() || body_block_signatures(before)? != body_block_signatures(&after)? {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "section edit changed body or section count",
        ));
    }
    for (index, (old, new)) in old.iter().zip(new.iter()).enumerate() {
        let bytes = |source: &SourceDocument, id| {
            let span = source.node(id).unwrap().span();
            source.original_bytes()[span.start..span.end].to_vec()
        };
        if index != selected && bytes(before, *old) != bytes(&after, *new) {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "section edit changed another section",
            ));
        }
    }
    Ok(after)
}

pub(super) fn on_off(source: &SourceDocument, id: NodeId) -> bool {
    !matches!(
        source.node(id).and_then(|v| v.attribute("val")),
        Some("0" | "false" | "off")
    )
}

fn settings_part(package: &Package, main: &Part) -> Result<Option<Part>, OperationResult> {
    let relationships = package.part_relationships(main).map_err(package_failure)?;
    let mut settings = relationships
        .into_iter()
        .filter(|r| r.relationship_type.as_str().ends_with("/settings"));
    let rel = settings.next();
    if settings.next().is_some() {
        return Err(unsupported("settings relationship is ambiguous"));
    }
    let Some(rel) = rel else {
        return Ok(None);
    };
    let RelationshipTarget::Internal { part_name, .. } = rel.target else {
        return Err(unsupported("settings must be an internal part"));
    };
    package.part(&part_name).map(Some).map_err(package_failure)
}

pub(super) fn odd_even_headers_enabled(
    package: &Package,
    main: &Part,
) -> Result<bool, OperationResult> {
    let Some(part) = settings_part(package, main)? else {
        return Ok(false);
    };
    let source = SourceDocument::parse(package.read_part(&part).map_err(package_failure)?)
        .map_err(document_invalid)?;
    Ok(section_child(&source, source.root(), "evenAndOddHeaders")?
        .is_some_and(|id| on_off(&source, id)))
}

pub fn set_odd_even_headers_to_vec(
    package: &Package,
    main: &Part,
    _source: &SourceDocument,
    operation: &SetOddEvenHeaders,
) -> Result<Vec<u8>, OperationResult> {
    let part = settings_part(package, main)?.ok_or_else(|| {
        unsupported(
            "document has no settings part; odd/even editing requires an existing settings part",
        )
    })?;
    let source = SourceDocument::parse(package.read_part(&part).map_err(package_failure)?)
        .map_err(document_invalid)?;
    let existing = section_child(&source, source.root(), "evenAndOddHeaders")?;
    let prefix = word_prefix_for(&source, source.root(), "settings")?;
    const SUCCESSORS: &str = "bookFoldRevPrinting bookFoldPrinting bookFoldPrintingSheets drawingGridHorizontalSpacing drawingGridVerticalSpacing displayHorizontalDrawingGridEvery displayVerticalDrawingGridEvery doNotUseMarginsForDrawingGridOrigin drawingGridHorizontalOrigin drawingGridVerticalOrigin doNotShadeFormData noPunctuationKerning characterSpacingControl printTwoOnOne strictFirstAndLastChars noLineBreaksAfter noLineBreaksBefore savePreviewPicture doNotValidateAgainstSchema saveInvalidXml ignoreMixedContent alwaysShowPlaceholderText doNotDemarcateInvalidXml saveXmlDataOnly useXSLTWhenSaving saveThroughXslt showXMLTags alwaysMergeEmptyNamespace updateFields hdrShapeDefaults footnotePr endnotePr compat docVars rsids mathPr attachedSchema themeFontLang clrSchemeMapping doNotIncludeSubdocsInStats doNotAutoCompressPictures forceUpgrade captions readModeInkLockDown smartTagType schemaLibrary shapeDefaults doNotEmbedSmartTags decimalSymbol listSeparator";
    let at = source.children(source.root()).find(|id| matches!(source.node(*id).map(|n|n.kind()), Some(SourceNodeKind::Element { name, .. }) if SUCCESSORS.split_whitespace().any(|value|value==name.local_name())))
        .map(|id|source.node(id).unwrap().span().start).unwrap_or(source_end_tag_start(&source, source.root())?);
    let output = write_patches_to_vec(
        package,
        &part,
        &source,
        vec![Patch {
            span: existing
                .map(|id| source.node(id).unwrap().span())
                .unwrap_or(SourceSpan { start: at, end: at }),
            replacement: format!(
                "<{} {}val=\"{}\"/>",
                qualify(&prefix, "evenAndOddHeaders"),
                attr_prefix(&prefix),
                u8::from(operation.enabled)
            )
            .into_bytes(),
        }],
    )?;
    let check = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    if odd_even_headers_enabled(&check, main)? != operation.enabled {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "odd/even setting did not match request",
        ));
    }
    Ok(output)
}
