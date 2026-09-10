use super::*;

/// Applies or clears one external hyperlink without changing visible text or run properties.
pub fn set_hyperlink_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetHyperlink,
) -> Result<Vec<u8>, OperationResult> {
    let matched = resolve_single_hyperlink_match(source, &operation.target)?;
    let paragraph = matched
        .segments
        .first()
        .and_then(|s| paragraph_ancestor(source, s.id))
        .ok_or_else(|| unsupported("text range has no paragraph"))?;
    if matched
        .segments
        .iter()
        .any(|s| s.inside_tracked_change || paragraph_ancestor(source, s.id) != Some(paragraph))
        || !safe_body_paragraph(source, paragraph)
    {
        return Err(unsupported(
            "set_hyperlink supports only ordinary direct body text",
        ));
    }
    let link = matched
        .segments
        .first()
        .and_then(|s| source.node(s.id))
        .and_then(|n| n.parent())
        .and_then(|r| source.node(r))
        .and_then(|n| n.parent())
        .filter(|id| word(source, *id, "hyperlink"));
    match (operation.url.as_deref(), link) {
        (None, Some(link)) => clear_supported_hyperlink(package, main, source, &matched, link),
        (None, None) => Err(unsupported("target is not a supported hyperlink")),
        (Some(url), Some(link)) => {
            replace_supported_hyperlink(package, main, source, &matched, link, url)
        }
        (Some(url), None) => {
            apply_supported_hyperlink(package, main, source, &matched, paragraph, url)
        }
    }
}

pub(super) fn resolve_single_hyperlink_match(
    source: &SourceDocument,
    target: &TextTarget,
) -> Result<crate::text_search::ResolvedTextMatch, OperationResult> {
    let matches = crate::text_search::resolve_text(source, &target.text)
        .map_err(|e| OperationResult::failed(e.code(), e.to_string()))?;
    if matches.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "text target was not found",
        ));
    }
    if let Some(n) = target.occurrence {
        return matches.into_iter().nth(n).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "text target occurrence was not found")
        });
    }
    if matches.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "text target matches more than one current semantic range",
        ));
    }
    Ok(matches.into_iter().next().expect("one match"))
}

pub(super) fn valid_hyperlink_url(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    matches!(scheme, "http" | "https") && !host.is_empty() && !rest.chars().any(char::is_whitespace)
}

pub(super) fn apply_supported_hyperlink(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    matched: &crate::text_search::ResolvedTextMatch,
    paragraph: NodeId,
    url: &str,
) -> Result<Vec<u8>, OperationResult> {
    if !valid_hyperlink_url(url) {
        return Err(OperationResult::failed(
            "INVALID_HYPERLINK_URL",
            "URL must be an absolute http:// or https:// URL",
        ));
    }
    let mut runs = Vec::new();
    for s in &matched.segments {
        let run = ordinary_run(source, s.id)
            .ok_or_else(|| unsupported("set_hyperlink does not edit inline wrappers"))?;
        if source
            .children(run)
            .any(|c| !word(source, c, "rPr") && !word(source, c, "t"))
        {
            return Err(unsupported("set_hyperlink requires simple direct runs"));
        }
        runs.push((
            run,
            matched.start.max(s.start) - s.start,
            matched.end.min(s.end) - s.start,
            s.source_text.as_str(),
        ));
    }
    let prefix = word_prefix(source, paragraph)?;
    let name = |local| qualify(prefix, local);
    let mut before = String::new();
    let mut selected = String::new();
    let mut after = String::new();
    for (index, (run, start, end, text)) in runs.iter().enumerate() {
        let (a, b, c) = hyperlink_run_pieces(source, *run, text, *start, *end)?;
        if index == 0 {
            before = a;
        }
        if index + 1 == runs.len() {
            after = c;
        }
        selected.push_str(&b);
    }
    let (rels_name, rels_exist, rels, id) = hyperlink_relationship(package, main, url)?;
    let first = source.node(runs[0].0).unwrap().span();
    let last = source.node(runs.last().unwrap().0).unwrap().span();
    let patch = Patch { span: SourceSpan { start: first.start, end: last.end }, replacement: format!("{before}<{} xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:id=\"{id}\">{selected}</{}>{after}", name("hyperlink"), name("hyperlink")).into_bytes() };
    write_hyperlink_changes(
        package,
        main,
        source,
        vec![patch],
        rels_name,
        rels_exist,
        rels,
    )
}

pub(super) fn hyperlink_run_pieces(
    source: &SourceDocument,
    run: NodeId,
    text: &str,
    start: usize,
    end: usize,
) -> Result<(String, String, String), OperationResult> {
    let paragraph = source
        .node(run)
        .and_then(|n| n.parent())
        .expect("run parent");
    let prefix = word_prefix(source, paragraph)?;
    let name = |local| qualify(prefix, local);
    let SourceNodeKind::Element { start_tag, .. } = source.node(run).unwrap().kind() else {
        return Err(unsupported("run has no source tag"));
    };
    let open = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
        .map_err(|_| unsupported("run tag is not UTF-8"))?;
    let rpr = String::from_utf8_lossy(&run_properties(source, run)).into_owned();
    let make = |value: &str| {
        format!(
            "{open}{rpr}<{}{}>{}</{}></{}>",
            name("t"),
            if requires_space_preservation(value) {
                " xml:space=\"preserve\""
            } else {
                ""
            },
            escape(value),
            name("t"),
            name("r")
        )
    };
    Ok((
        if start > 0 {
            make(&text[..start])
        } else {
            String::new()
        },
        make(&text[start..end]),
        if end < text.len() {
            make(&text[end..])
        } else {
            String::new()
        },
    ))
}

pub(super) fn exact_hyperlink(
    source: &SourceDocument,
    matched: &crate::text_search::ResolvedTextMatch,
    link: NodeId,
) -> Result<(), OperationResult> {
    if matched.segments.iter().any(|s| {
        source
            .node(s.id)
            .and_then(|n| n.parent())
            .and_then(|r| source.node(r))
            .and_then(|n| n.parent())
            != Some(link)
    }) || (crate::references::Hyperlink {
        source,
        source_id: link,
    })
    .text()
    .map_err(document_invalid)?
        != matched.text
    {
        return Err(unsupported(
            "set_hyperlink requires an exact ordinary hyperlink",
        ));
    }
    Ok(())
}

pub(super) fn clear_supported_hyperlink(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    matched: &crate::text_search::ResolvedTextMatch,
    link: NodeId,
) -> Result<Vec<u8>, OperationResult> {
    exact_hyperlink(source, matched, link)?;
    let SourceNodeKind::Element {
        start_tag,
        end_tag: Some(end_tag),
        ..
    } = source.node(link).unwrap().kind()
    else {
        return Err(unsupported("hyperlink has no source boundaries"));
    };
    let span = source.node(link).unwrap().span();
    write_patches_to_vec(
        package,
        main,
        source,
        vec![Patch {
            span,
            replacement: source.original_bytes()[start_tag.end..end_tag.start].to_vec(),
        }],
    )
}

pub(super) fn replace_supported_hyperlink(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    matched: &crate::text_search::ResolvedTextMatch,
    link: NodeId,
    url: &str,
) -> Result<Vec<u8>, OperationResult> {
    if !valid_hyperlink_url(url) {
        return Err(OperationResult::failed(
            "INVALID_HYPERLINK_URL",
            "URL must be an absolute http:// or https:// URL",
        ));
    }
    exact_hyperlink(source, matched, link)?;
    let (rels_name, rels_exist, rels, id) = hyperlink_relationship(package, main, url)?;
    let span = source.node(link).unwrap().span();
    let SourceNodeKind::Element {
        start_tag,
        end_tag: Some(end_tag),
        ..
    } = source.node(link).unwrap().kind()
    else {
        return Err(unsupported("hyperlink has no source boundaries"));
    };
    let children = &source.original_bytes()[start_tag.end..end_tag.start];
    let prefix = word_prefix(source, link)?;
    let replacement = format!("<{} xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:id=\"{id}\">", qualify(prefix,"hyperlink")).into_bytes().into_iter().chain(children.iter().copied()).chain(format!("</{}>",qualify(prefix,"hyperlink")).bytes()).collect();
    write_hyperlink_changes(
        package,
        main,
        source,
        vec![Patch { span, replacement }],
        rels_name,
        rels_exist,
        rels,
    )
}

pub(super) fn hyperlink_relationship(
    package: &Package,
    main: &Part,
    url: &str,
) -> Result<(PartName, bool, Vec<u8>, String), OperationResult> {
    let name = relationship_part_name(main)?;
    let (exists, mut xml) = match package.read_part_by_name(&name) { Ok(v)=>(true,v), Err(PackageError::MissingTargetPart(_)) => (false, br#"<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"></Relationships>"#.to_vec()), Err(e)=>return Err(OperationResult::failed(e.code(),e.to_string())) };
    let relationships = package.part_relationships(main).unwrap_or_default();
    if let Some(r) = relationships.iter().find(|r| {
        r.relationship_type.as_str() == HYPERLINK_RELATIONSHIP_TYPE
            && matches!(&r.target, RelationshipTarget::External { original } if original == url)
    }) {
        return Ok((name, exists, xml, r.id.as_str().to_owned()));
    }
    let n = relationships
        .iter()
        .filter_map(|r| r.id.as_str().strip_prefix("rId")?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| {
            OperationResult::failed("PACKAGE_CONFLICT", "cannot allocate hyperlink relationship")
        })?;
    xml = append_xml_element(
        &xml,
        &format!(
            r#"<Relationship Id="rId{n}" Type="{HYPERLINK_RELATIONSHIP_TYPE}" Target="{}" TargetMode="External"/>"#,
            escape(url)
        ),
    )?;
    Ok((name, exists, xml, format!("rId{n}")))
}

pub(super) fn write_hyperlink_changes(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    patches: Vec<Patch>,
    rels_name: PartName,
    rels_exist: bool,
    rels: Vec<u8>,
) -> Result<Vec<u8>, OperationResult> {
    let document = apply_patches(source, patches)?;
    let replaced = if rels_exist {
        vec![
            (main.name.clone(), document.as_slice()),
            (rels_name.clone(), rels.as_slice()),
        ]
    } else {
        vec![(main.name.clone(), document.as_slice())]
    };
    let added = if rels_exist {
        vec![]
    } else {
        vec![(rels_name, rels.as_slice())]
    };
    let output = package
        .write_package_with_named_changes_to_vec(&replaced, &added)
        .map_err(|e| OperationResult::failed(e.code(), e.to_string()))?;
    let verified = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    verified.verify().map_err(document_invalid)?;
    Ok(output)
}
