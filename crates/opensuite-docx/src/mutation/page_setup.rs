use super::*;

/// Inspects the effective page layout without exposing OOXML source details.
pub fn inspect_page_setup(source: &SourceDocument) -> Result<PageSetupInspection, OperationResult> {
    let section = main_section(source, false)?;
    page_setup_from_section(source, section)
}

/// Changes margins, paper size, or orientation for one safe terminal body section.
pub fn set_page_setup_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetPageSetup,
) -> Result<Vec<u8>, OperationResult> {
    if operation.margins.is_none()
        && operation.paper_size.is_none()
        && operation.orientation.is_none()
    {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "set_page_setup requires at least one change",
        ));
    }
    let section = main_section(source, true)?;
    let before = page_setup_from_section(source, section)?;
    let prefix = word_prefix_for(source, section, "sectPr")?;
    let name = |local: &str| qualify(&prefix, local);
    let attr = attr_prefix(&prefix);
    let size_node = source
        .children(section)
        .find(|id| word(source, *id, "pgSz"));
    let margin_node = source
        .children(section)
        .find(|id| word(source, *id, "pgMar"));
    let (mut width, mut height) = size_node
        .map(|id| page_dimensions(source, id))
        .transpose()?
        .unwrap_or(LETTER);
    let orientation = operation.orientation.unwrap_or(before.orientation);
    if let Some(paper) = operation.paper_size {
        (width, height) = paper_dimensions(paper);
    } else if orientation != before.orientation && before.orientation == PageOrientation::Landscape
    {
        std::mem::swap(&mut width, &mut height);
    }
    if orientation == PageOrientation::Landscape && width < height {
        std::mem::swap(&mut width, &mut height);
    }
    if orientation == PageOrientation::Portrait && width > height {
        std::mem::swap(&mut width, &mut height);
    }
    let margins = merged_margins(before.margins.clone(), operation.margins.as_ref())?;
    let size_xml = format!(
        "<{} {}w=\"{width}\" {}h=\"{height}\" {}orient=\"{}\"/>",
        name("pgSz"),
        attr,
        attr,
        attr,
        orientation_name(orientation)
    );
    let margin_xml = format!(
        "<{} {}top=\"{}\" {}right=\"{}\" {}bottom=\"{}\" {}left=\"{}\"{} />",
        name("pgMar"),
        attr,
        margins.top_twips.unwrap(),
        attr,
        margins.right_twips.unwrap(),
        attr,
        margins.bottom_twips.unwrap(),
        attr,
        margins.left_twips.unwrap(),
        preserved_margin_attributes(source, margin_node, &prefix)
    );
    let mut patches = Vec::new();
    if operation.paper_size.is_some() || operation.orientation.is_some() {
        section_property_patch(source, section, size_node, "pgSz", size_xml, &mut patches)?;
    }
    if operation.margins.is_some() {
        section_property_patch(
            source,
            section,
            margin_node,
            "pgMar",
            margin_xml,
            &mut patches,
        )?;
    }
    let output = write_patches_to_vec(package, main, source, patches)?;
    let (_, output_source) =
        crate::open_main_source(&Package::from_bytes(output.clone()).map_err(document_invalid)?)
            .map_err(document_invalid)?;
    let after = inspect_page_setup(&output_source)?;
    if operation.paper_size.is_some() && after.paper_size != operation.paper_size
        || operation.orientation.is_some() && after.orientation != orientation
        || operation.margins.is_some() && after.margins != margins
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output page setup does not match request",
        ));
    }
    Ok(output)
}

pub fn set_page_setup(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetPageSetup,
    output: impl AsRef<Path>,
) -> OperationResult {
    write_operation_output(
        package,
        output.as_ref(),
        set_page_setup_to_vec(package, main, source, operation),
    )
}

pub(super) fn main_section(
    source: &SourceDocument,
    allow_missing: bool,
) -> Result<NodeId, OperationResult> {
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    let body = document.body_id();
    let sections = document
        .sections()
        .map(|section| section.source_id())
        .collect::<Vec<_>>();
    if sections.len() > 1
        || source.children(body).any(|id| {
            word(source, id, "p")
                && source.children(id).any(|child| {
                    word(source, child, "pPr")
                        && source
                            .children(child)
                            .any(|value| word(source, value, "sectPr"))
                })
        })
    {
        return Err(unsupported(
            "set_page_setup supports only one terminal main-document section",
        ));
    }
    if let Some(section) = sections.first().copied() {
        if source.node(section).and_then(|node| node.parent()) != Some(body)
            || source.children(body).last() != Some(section)
        {
            return Err(unsupported(
                "set_page_setup requires terminal body section properties",
            ));
        }
        if source
            .children(section)
            .any(|id| word(source, id, "sectPrChange"))
        {
            return Err(unsupported(
                "set_page_setup does not edit tracked section properties",
            ));
        }
        return Ok(section);
    }
    if allow_missing {
        return Err(unsupported(
            "set_page_setup requires existing terminal section properties",
        ));
    }
    Err(OperationResult::failed(
        "TARGET_NOT_FOUND",
        "document has no effective page setup",
    ))
}

pub(super) fn page_setup_from_section(
    source: &SourceDocument,
    section: NodeId,
) -> Result<PageSetupInspection, OperationResult> {
    let properties = crate::SectionProperties {
        source,
        source_id: section,
    };
    let size = properties
        .page_size()
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    let margins = properties
        .page_margins()
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    let orientation = size
        .as_ref()
        .and_then(|size| size.orientation.clone())
        .map(|value| match value {
            crate::PageOrientation::Landscape => PageOrientation::Landscape,
            _ => PageOrientation::Portrait,
        })
        .unwrap_or_else(|| {
            match size
                .as_ref()
                .and_then(|size| size.width_twips.zip(size.height_twips))
            {
                Some((width, height)) if width > height => PageOrientation::Landscape,
                _ => PageOrientation::Portrait,
            }
        });
    let paper_size = size
        .and_then(|size| size.width_twips.zip(size.height_twips))
        .and_then(|(width, height)| paper_from_dimensions(width, height));
    Ok(PageSetupInspection {
        margins: PageMargins {
            top_twips: margins.as_ref().and_then(|value| value.top_twips),
            right_twips: margins.as_ref().and_then(|value| value.right_twips),
            bottom_twips: margins.as_ref().and_then(|value| value.bottom_twips),
            left_twips: margins.as_ref().and_then(|value| value.left_twips),
        },
        paper_size,
        orientation,
    })
}

pub(super) fn page_dimensions(
    source: &SourceDocument,
    size: NodeId,
) -> Result<(u32, u32), OperationResult> {
    let node = source.node(size).expect("page size exists");
    let width = node
        .attribute("w")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| unsupported("set_page_setup requires complete page dimensions"))?;
    let height = node
        .attribute("h")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| unsupported("set_page_setup requires complete page dimensions"))?;
    Ok((width, height))
}

pub(super) fn paper_dimensions(paper: PaperSize) -> (u32, u32) {
    match paper {
        PaperSize::Letter => LETTER,
        PaperSize::A4 => A4,
    }
}

pub(super) fn paper_from_dimensions(width: u32, height: u32) -> Option<PaperSize> {
    match (width.min(height), width.max(height)) {
        LETTER => Some(PaperSize::Letter),
        A4 => Some(PaperSize::A4),
        _ => None,
    }
}

pub(super) fn orientation_name(value: PageOrientation) -> &'static str {
    match value {
        PageOrientation::Portrait => "portrait",
        PageOrientation::Landscape => "landscape",
    }
}

pub(super) fn merged_margins(
    current: PageMargins,
    requested: Option<&PageMargins>,
) -> Result<PageMargins, OperationResult> {
    let requested = requested.cloned().unwrap_or_default();
    let result = PageMargins {
        top_twips: requested
            .top_twips
            .or(current.top_twips)
            .or(Some(DEFAULT_MARGIN_TWIPS)),
        right_twips: requested
            .right_twips
            .or(current.right_twips)
            .or(Some(DEFAULT_MARGIN_TWIPS)),
        bottom_twips: requested
            .bottom_twips
            .or(current.bottom_twips)
            .or(Some(DEFAULT_MARGIN_TWIPS)),
        left_twips: requested
            .left_twips
            .or(current.left_twips)
            .or(Some(DEFAULT_MARGIN_TWIPS)),
    };
    if [
        result.top_twips,
        result.right_twips,
        result.bottom_twips,
        result.left_twips,
    ]
    .into_iter()
    .flatten()
    .any(|value| !(0..=MAX_MARGIN_TWIPS).contains(&value))
    {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "page margins must be between 0 and 31680 twips",
        ));
    }
    Ok(result)
}

pub(super) fn preserved_margin_attributes(
    source: &SourceDocument,
    margin: Option<NodeId>,
    prefix: &str,
) -> String {
    let Some(margin) = margin else {
        return String::new();
    };
    let attribute = attr_prefix(prefix);
    ["header", "footer", "gutter"]
        .into_iter()
        .filter_map(|key| {
            source
                .node(margin)
                .and_then(|node| node.attribute(key))
                .map(|value| format!(" {attribute}{key}=\"{value}\""))
        })
        .collect()
}

pub(super) fn section_property_patch(
    source: &SourceDocument,
    section: NodeId,
    existing: Option<NodeId>,
    local: &str,
    replacement: String,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    if let Some(existing) = existing {
        patches.push(Patch {
            span: source
                .node(existing)
                .expect("section property exists")
                .span(),
            replacement: replacement.into_bytes(),
        });
        return Ok(());
    }
    let at = source.node(section).expect("section exists").span().end
        - format!(
            "</{}>",
            qualify(&word_prefix_for(source, section, "sectPr")?, "sectPr")
        )
        .len();
    let _ = local;
    patches.push(Patch {
        span: SourceSpan { start: at, end: at },
        replacement: replacement.into_bytes(),
    });
    Ok(())
}
