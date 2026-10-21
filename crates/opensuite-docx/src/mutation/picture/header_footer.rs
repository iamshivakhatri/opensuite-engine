use super::*;

// Shared and inherited parts require a separate, explicit section edit first.
pub(super) fn insert(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertPicture,
    original: (i64, i64),
) -> Result<Vec<u8>, OperationResult> {
    use super::super::header_footer::*;
    let opensuite_protocol::PicturePlacement::HeaderFooter {
        target,
        kind,
        variant,
    } = &operation.placement
    else {
        unreachable!("header/footer placement");
    };
    let (kind, variant) = (*kind, *variant);
    let (ids, index) = sections::resolve_section(package, main, source, target)?;
    if operation.layout.is_some() {
        return Err(unsupported("header/footer pictures require inline layout"));
    }
    let section = ids[index];
    if source.node(section).is_some_and(|node| matches!(node.kind(), SourceNodeKind::Element { name, .. } if name.namespace_uri() == Some(NS[1]))) {
        return Err(unsupported("header/footer picture insertion supports transitional DOCX only"));
    }
    if variant == HeaderFooterVariant::Even && !sections::odd_even_headers_enabled(package, main)? {
        return Err(unsupported(
            "enable document-wide odd/even headers before inserting into the even variant",
        ));
    }
    if variant == HeaderFooterVariant::First
        && !sections::section_child(source, section, "titlePg")?
            .is_some_and(|id| sections::on_off(source, id))
    {
        return Err(unsupported(
            "enable different first page before inserting into the first variant",
        ));
    }
    let own = variant_reference(source, section, kind, variant)?;
    if own.is_none() {
        if ids[..index]
            .iter()
            .any(|id| variant_reference(source, *id, kind, variant).map_or(true, |v| v.is_some()))
        {
            return Err(unsupported(
                "header/footer is inherited; unlink it explicitly before image insertion",
            ));
        }
        // The existing section writer creates the missing part and retains the
        // following section's previous content with an explicit reference.
        let bytes = set_section_header_footer_to_vec(
            package,
            main,
            source,
            &SetSectionHeaderFooter {
                target: target.clone(),
                kind,
                variant,
                change: SectionHeaderFooterChange::Unlink,
            },
        )?;
        let created = Package::from_bytes(bytes).map_err(document_invalid)?;
        let (created_main, created_source) =
            crate::open_main_source(&created).map_err(document_invalid)?;
        let fresh = SectionTarget {
            handle: sections::inspect_sections(&created, &created_main, &created_source)?[index]
                .handle
                .clone(),
        };
        let mut operation = operation.clone();
        operation.placement = opensuite_protocol::PicturePlacement::HeaderFooter {
            target: fresh,
            kind,
            variant,
        };
        return insert(
            &created,
            &created_main,
            &created_source,
            &operation,
            original,
        );
    }
    let own = own.expect("owned reference");
    let part = header_footer_part(package, main, source, own, kind)?;
    let relationships = package.part_relationships(main).map_err(package_failure)?;
    let mut used_ids = HashSet::new();
    if relationships
        .iter()
        .any(|rel| !used_ids.insert(rel.id.as_str()))
    {
        return Err(unsupported("main document has duplicate relationship ids"));
    }
    let same_part_ids: HashSet<_> = relationships.iter().filter(|rel| matches!(&rel.target, RelationshipTarget::Internal { part_name, .. } if part_name == &part.name)).map(|rel| rel.id.as_str()).collect();
    for id in &ids {
        for other_kind in [HeaderFooterKind::Header, HeaderFooterKind::Footer] {
            for other_variant in [
                HeaderFooterVariant::Default,
                HeaderFooterVariant::First,
                HeaderFooterVariant::Even,
            ] {
                variant_reference(source, *id, other_kind, other_variant)?;
            }
        }
    }
    if source
        .node_ids()
        .filter(|id| word(source, *id, "headerReference") || word(source, *id, "footerReference"))
        .any(|id| {
            id != own
                && source
                    .node(id)
                    .and_then(|n| n.attribute("id"))
                    .is_some_and(|id| same_part_ids.contains(id))
        })
    {
        return Err(unsupported(
            "header/footer part is shared; isolate it explicitly before image insertion",
        ));
    }
    if let Some(next) = ids.get(index + 1) {
        if variant_reference(source, *next, kind, variant)?.is_none() {
            return Err(unsupported(
                "following section inherits this header/footer; isolate it explicitly before image insertion",
            ));
        }
    }
    let part_source = SourceDocument::parse(package.read_part(&part).map_err(package_failure)?)
        .map_err(document_invalid)?;
    let root = part_source.root();
    if !word(&part_source, root, header_footer_root_name(kind))
        || part_source.children(root).any(|id| {
            matches!(
                part_source.node(id).map(|n| n.kind()),
                Some(SourceNodeKind::Element { .. })
            ) && !word(&part_source, id, "p")
        })
        || part_source
            .node_ids()
            .any(|id| crate::tracked_change::revision_node(&part_source, id))
    {
        return Err(unsupported(
            "header/footer insertion requires direct paragraphs without tracked changes",
        ));
    }
    let insertion = source_end_tag_start(&part_source, root)?;
    protected_range::ensure_safe_source_ranges(
        &part_source,
        &[SourceSpan {
            start: insertion,
            end: insertion + 1,
        }],
        "UNSUPPORTED_PICTURE_INSERT",
        false,
    )?;
    let properties = crate::DocxDocument::new(source)
        .map_err(document_invalid)?
        .sections()
        .nth(index)
        .expect("section")
        .properties();
    for local in ["pgSz", "pgMar"] {
        sections::section_child(source, section, local)?;
    }
    let invalid = || {
        OperationResult::failed(
            "INVALID_PAGE_GEOMETRY",
            "header/footer picture requires positive declared page margins and header/footer distance",
        )
    };
    let page = properties
        .page_size()
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    if matches!(page.orientation, Some(crate::PageOrientation::Unknown(_))) {
        return Err(invalid());
    }
    let margins = properties
        .page_margins()
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    let positive = |v: Option<i32>| v.filter(|v| *v >= 0).map(i64::from).ok_or_else(invalid);
    if i64::from(page.height_twips.ok_or_else(invalid)?)
        <= positive(margins.top_twips)? + positive(margins.bottom_twips)?
    {
        return Err(invalid());
    }
    let width = i64::from(page.width_twips.ok_or_else(invalid)?)
        - positive(margins.left_twips)?
        - positive(margins.right_twips)?
        - positive(Some(margins.gutter_twips.unwrap_or(0)))?;
    let (margin, distance) = if kind == HeaderFooterKind::Header {
        (margins.top_twips, margins.header_twips)
    } else {
        (margins.bottom_twips, margins.footer_twips)
    };
    // Reserve a small baseline allowance; remaining rendered space is unknown.
    let height = positive(margin)? - positive(distance)? - 120;
    let extent = if let Some(size) = operation.size {
        let extent = sized_extent(original, Some(size))?;
        if inline::fit_to_bounds(package, main, extent, (width, height))? != extent {
            return Err(unsupported(
                "explicit picture size exceeds header/footer bounds",
            ));
        }
        extent
    } else {
        inline::fit_to_bounds(package, main, original, (width, height))?
    };
    insert_at(
        package,
        &part,
        &part_source,
        operation,
        root,
        insertion,
        extent,
    )
}

#[cfg(test)]
mod tests;
