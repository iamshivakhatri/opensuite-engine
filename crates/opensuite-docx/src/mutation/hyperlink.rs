use super::*;

/// Applies or clears one external or internal hyperlink without changing visible text or run properties.
pub fn set_hyperlink_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetHyperlink,
) -> Result<Vec<u8>, OperationResult> {
    ensure_document_unprotected(package, main)?;
    if operation.url.is_some() && operation.bookmark_name.is_some() {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "supply url or bookmarkName, not both",
        ));
    }
    if let Some(name) = &operation.bookmark_name {
        validate_bookmark_destination(source, name)?;
    }
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
    let before = body_texts(source)?;
    let output = match (
        operation.url.as_deref(),
        operation.bookmark_name.as_deref(),
        link,
    ) {
        (None, None, Some(link)) => {
            clear_supported_hyperlink(package, main, source, &matched, link)
        }
        (None, None, None) => Err(unsupported("target is not a supported hyperlink")),
        (url, bookmark, Some(link)) => {
            replace_supported_hyperlink(package, main, source, &matched, link, url, bookmark)
        }
        (url, bookmark, None) => {
            text::simple_body_text_range(
                source,
                &operation.target,
                "UNSAFE_HYPERLINK_RANGE",
                false,
            )?;
            apply_supported_hyperlink(package, main, source, &matched, paragraph, url, bookmark)
        }
    }?;
    let reopened = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    let (new_main, new_source) = crate::open_main_source(&reopened).map_err(document_invalid)?;
    if body_texts(&new_source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        != before.into_iter().map(|(_, text)| text).collect::<Vec<_>>()
    {
        return Err(document_invalid("hyperlink edit changed document text"));
    }
    if operation.url.is_some() || operation.bookmark_name.is_some() {
        let selected = resolve_single_hyperlink_match(&new_source, &operation.target)?;
        let new_link = selected
            .segments
            .first()
            .and_then(|s| new_source.node(s.id))
            .and_then(|n| n.parent())
            .and_then(|id| new_source.node(id))
            .and_then(|n| n.parent())
            .filter(|id| word(&new_source, *id, "hyperlink"))
            .ok_or_else(|| document_invalid("hyperlink missing after reopen"))?;
        let target = crate::references::Hyperlink {
            source: &new_source,
            source_id: new_link,
        }
        .target(&reopened, &new_main)
        .map_err(document_invalid)?;
        let expected = operation
            .bookmark_name
            .as_ref()
            .map(|name| crate::HyperlinkTarget::InternalAnchor(name.clone()))
            .or_else(|| {
                operation
                    .url
                    .as_ref()
                    .map(|url| crate::HyperlinkTarget::External(url.clone()))
            });
        if target != expected {
            return Err(document_invalid(
                "hyperlink destination changed after reopen",
            ));
        }
    }
    Ok(output)
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
    url: Option<&str>,
    bookmark: Option<&str>,
) -> Result<Vec<u8>, OperationResult> {
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
        let (a, b, c) = text::text_run_pieces(source, *run, text, *start, *end, false)?;
        if index == 0 {
            before = a;
        }
        if index + 1 == runs.len() {
            after = c;
        }
        selected.push_str(&b);
    }
    let (attributes, relationships) =
        destination_attributes(package, main, source, prefix, url, bookmark)?;
    let first = source.node(runs[0].0).unwrap().span();
    let last = source.node(runs.last().unwrap().0).unwrap().span();
    let patch = Patch {
        span: SourceSpan {
            start: first.start,
            end: last.end,
        },
        replacement: format!(
            "{before}<{}{attributes}>{selected}</{}>{after}",
            name("hyperlink"),
            name("hyperlink")
        )
        .into_bytes(),
    };
    write_destination_changes(package, main, source, patch, relationships)
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
    if !source
        .children(link)
        .all(|id| word(source, id, "r") && safe_paragraph_child(source, id))
    {
        return Err(unsupported("hyperlink contains unsupported inline content"));
    }
    protected_range::ensure_safe_hyperlink_edit(source, source.node(link).unwrap().span(), link)?;
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
    url: Option<&str>,
    bookmark: Option<&str>,
) -> Result<Vec<u8>, OperationResult> {
    exact_hyperlink(source, matched, link)?;
    let node = source.node(link).unwrap();
    let SourceNodeKind::Element {
        start_tag,
        end_tag: Some(_),
        ..
    } = node.kind()
    else {
        return Err(unsupported("hyperlink has no source boundaries"));
    };
    let prefix = word_prefix_for(source, link, "hyperlink")?;
    let (attributes, relationships) =
        destination_attributes(package, main, source, &prefix, url, bookmark)?;
    let mut tag =
        String::from_utf8(source.original_bytes()[start_tag.start..start_tag.end].to_vec())
            .map_err(document_invalid)?;
    let mut reader = quick_xml::reader::NsReader::from_reader(source.original_bytes());
    let keys = loop {
        let event = reader.read_event().map_err(document_invalid)?;
        if reader.buffer_position() as usize == start_tag.end {
            let quick_xml::events::Event::Start(element) = event else {
                return Err(unsupported("hyperlink has no start tag"));
            };
            let mut keys = Vec::new();
            for attr in element.attributes() {
                let attr = attr.map_err(document_invalid)?;
                let (namespace, local) = reader.resolve_attribute(attr.key);
                let known = match namespace {
                    quick_xml::name::ResolveResult::Bound(uri) => {
                        let uri = std::str::from_utf8(uri.as_ref()).map_err(document_invalid)?;
                        (NS.contains(&uri) && matches!(local.as_ref(), b"anchor" | b"docLocation"))
                            || (["http://schemas.openxmlformats.org/officeDocument/2006/relationships", "http://purl.oclc.org/ooxml/officeDocument/relationships"].contains(&uri) && local.as_ref() == b"id")
                    }
                    _ => false,
                };
                if known {
                    keys.push(String::from_utf8_lossy(attr.key.as_ref()).into_owned());
                }
            }
            break keys;
        }
        if matches!(event, quick_xml::events::Event::Eof) {
            return Err(unsupported("hyperlink source tag was not found"));
        }
    };
    for key in keys {
        tag = edit_attribute(tag, &key, None);
    }
    tag.insert_str(tag.len() - 1, &attributes);
    write_destination_changes(
        package,
        main,
        source,
        Patch {
            span: *start_tag,
            replacement: tag.into_bytes(),
        },
        relationships,
    )
}

pub(crate) fn validate_bookmark_destination(
    source: &SourceDocument,
    name: &str,
) -> Result<(), OperationResult> {
    let bookmarks = crate::references::bookmarks(source)
        .map_err(|e| OperationResult::failed(e.code(), e.to_string()))?;
    validate_parsed_bookmark_destination(&bookmarks, name)
}

pub(crate) fn validate_parsed_bookmark_destination(
    bookmarks: &[crate::Bookmark],
    name: &str,
) -> Result<(), OperationResult> {
    let count = bookmarks.iter().filter(|b| b.name == name).count();
    if count != 1 {
        return Err(OperationResult::failed(
            if count == 0 {
                "BOOKMARK_NOT_FOUND"
            } else {
                "BOOKMARK_AMBIGUOUS"
            },
            "bookmark destination must have exactly one complete definition",
        ));
    }
    if name.chars().count() > 40
        || !name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        || !name.chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        return Err(unsupported(
            "bookmark name cannot be authored as a Word-compatible anchor",
        ));
    }
    Ok(())
}

type HyperlinkRelationships = (PartName, bool, Vec<u8>);
fn destination_attributes(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    prefix: &str,
    url: Option<&str>,
    bookmark: Option<&str>,
) -> Result<(String, Option<HyperlinkRelationships>), OperationResult> {
    if let Some(name) = bookmark {
        if prefix.is_empty() {
            return Err(unsupported(
                "internal hyperlink authoring requires an explicit Word namespace prefix",
            ));
        }
        return Ok((format!(" {prefix}:anchor=\"{}\"", escape(name)), None));
    }
    let url = url.ok_or_else(|| unsupported("hyperlink destination is missing"))?;
    if !valid_hyperlink_url(url) {
        return Err(OperationResult::failed(
            "INVALID_HYPERLINK_URL",
            "URL must be an absolute http:// or https:// URL",
        ));
    }
    let (name, exists, bytes, id) = hyperlink_relationship(package, main, url)?;
    let mut rel_prefix = "opensuiteLink".to_owned();
    while source
        .original_bytes()
        .windows(rel_prefix.len())
        .any(|bytes| bytes == rel_prefix.as_bytes())
    {
        rel_prefix.push('_');
    }
    Ok((
        format!(
            " xmlns:{rel_prefix}=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" {rel_prefix}:id=\"{id}\""
        ),
        Some((name, exists, bytes)),
    ))
}
fn write_destination_changes(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    patch: Patch,
    relationships: Option<HyperlinkRelationships>,
) -> Result<Vec<u8>, OperationResult> {
    match relationships {
        Some((name, exists, bytes)) => {
            write_hyperlink_changes(package, main, source, vec![patch], name, exists, bytes)
        }
        None => write_patches_to_vec(package, main, source, vec![patch]),
    }
}

pub(super) fn hyperlink_relationship(
    package: &Package,
    main: &Part,
    url: &str,
) -> Result<(PartName, bool, Vec<u8>, String), OperationResult> {
    let name = relationship_part_name(main)?;
    let (exists, mut xml) = match package.read_part_by_name(&name) { Ok(v)=>(true,v), Err(PackageError::MissingTargetPart(_)) => (false, br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_vec()), Err(e)=>return Err(OperationResult::failed(e.code(),e.to_string())) };
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
