use super::*;

/// Applies one preservation-safe replacement across compatible `w:t` source regions.
pub fn replace_text(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &ReplaceText,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let target = match resolve_target(source, operation) {
        Ok(target) => target,
        Err(result) => return result,
    };
    if target.value != operation.expected_current_text {
        return OperationResult::failed(
            "PRECONDITION_FAILED",
            "resolved text does not match expected current text",
        )
        .with_reason_code("EXPECTED_TEXT_MISMATCH");
    }
    let patched = match apply_patches(source, target.patches) {
        Ok(patched) => patched,
        Err(result) => return result,
    };
    let temporary = output.with_file_name(format!(
        ".opensuite-{}-{}.docx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) = verify_output(&temporary, &operation.replacement) {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::applied(
        operation.expected_current_text.clone(),
        operation.replacement.clone(),
    )
}

/// Applies one preservation-safe text replacement and returns the updated DOCX bytes.
pub fn replace_text_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &ReplaceText,
) -> Result<Vec<u8>, OperationResult> {
    let target = resolve_target(source, operation)?;
    if target.value != operation.expected_current_text {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "resolved text does not match expected current text",
        )
        .with_reason_code("EXPECTED_TEXT_MISMATCH"));
    }
    let patched = apply_patches(source, target.patches)?;
    let output = package
        .write_replaced_part_to_vec(main, &patched)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    verify_output_bytes(&output, &operation.replacement)?;
    Ok(output)
}

pub(super) struct ResolvedText {
    value: String,
    patches: Vec<Patch>,
}

pub(super) fn resolve_target(
    source: &SourceDocument,
    operation: &ReplaceText,
) -> Result<ResolvedText, OperationResult> {
    let matches = crate::text_search::resolve_text(source, &operation.target.text)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    if matches.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "text target was not found",
        ));
    }
    let matched = if let Some(occurrence) = operation.target.occurrence {
        matches.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "text target occurrence was not found")
        })?
    } else if matches.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "text target matches more than one current semantic range",
        )
        .with_candidates(
            "text",
            (0..matches.len()).map(|index| format!("text occurrence {index}")),
            matches.len(),
            "occurrence",
        ));
    } else {
        matches.into_iter().next().expect("one match")
    };
    let patches = patches_for_match(source, &matched, &operation.replacement)?;
    Ok(ResolvedText {
        value: matched.text,
        patches,
    })
}

pub(super) fn patches_for_match(
    source: &SourceDocument,
    matched: &crate::text_search::ResolvedTextMatch,
    replacement: &str,
) -> Result<Vec<Patch>, OperationResult> {
    let mut runs = Vec::new();
    for segment in &matched.segments {
        if segment.inside_tracked_change {
            return Err(unsupported(
                "replace_text does not edit tracked revision text",
            ));
        }
        let run = ordinary_run(source, segment.id)
            .ok_or_else(|| unsupported("replace_text does not cross inline wrapper boundaries"))?;
        let child = text_child(source, segment.id)
            .filter(|child| !is_cdata(source, *child))
            .ok_or_else(|| unsupported("replace_text requires replaceable text source regions"))?;
        if source.children(segment.id).nth(1).is_some() {
            return Err(unsupported(
                "replace_text requires one source region per text node",
            ));
        }
        runs.push((segment, run, child));
    }
    let formatting = run_properties(source, runs[0].1);
    if runs
        .iter()
        .any(|(_, run, _)| run_properties(source, *run) != formatting)
    {
        return Err(unsupported(
            "replace_text spans incompatible run formatting regions",
        ));
    }

    let mut remaining = replacement.chars();
    let mut patches = Vec::with_capacity(runs.len() * 2);
    for (index, (segment, _, child)) in runs.into_iter().enumerate() {
        let start = matched.start.max(segment.start) - segment.start;
        let end = matched.end.min(segment.end) - segment.start;
        let prefix = &segment.source_text[..start];
        let suffix = &segment.source_text[end..];
        let matched_chars = segment.source_text[start..end].chars().count();
        let distributed: String = if index + 1 == matched.segments.len() {
            remaining.by_ref().collect()
        } else {
            remaining.by_ref().take(matched_chars).collect()
        };
        let text = format!("{prefix}{distributed}{suffix}");
        let span = source.node(child).expect("source child exists").span();
        patches.push(Patch {
            span,
            replacement: escape(&text).into_owned().into_bytes(),
        });
        if requires_space_preservation(&text) && !has_xml_space(source, segment.id) {
            patches.push(Patch {
                span: start_tag_end(source, segment.id)?,
                replacement: b" xml:space=\"preserve\"".to_vec(),
            });
        }
    }
    Ok(patches)
}

pub(super) fn ordinary_run(source: &SourceDocument, text: NodeId) -> Option<NodeId> {
    let run = source.node(text)?.parent()?;
    (word(source, run, "r")
        && source
            .node(run)?
            .parent()
            .is_some_and(|parent| word(source, parent, "p")))
    .then_some(run)
}

pub(super) fn run_properties(source: &SourceDocument, run: NodeId) -> Vec<u8> {
    source
        .children(run)
        .find(|child| word(source, *child, "rPr"))
        .map_or_else(Vec::new, |properties| {
            let span = source.node(properties).expect("source node exists").span();
            source.original_bytes()[span.start..span.end].to_vec()
        })
}

pub(super) fn requires_space_preservation(text: &str) -> bool {
    text.starts_with(char::is_whitespace) || text.ends_with(char::is_whitespace)
}

pub(super) fn has_xml_space(source: &SourceDocument, text: NodeId) -> bool {
    let SourceNodeKind::Element { start_tag, .. } =
        source.node(text).expect("source node exists").kind()
    else {
        return false;
    };
    source.original_bytes()[start_tag.start..start_tag.end]
        .windows(b"xml:space".len())
        .any(|window| window == b"xml:space")
}

pub(super) fn start_tag_end(
    source: &SourceDocument,
    text: NodeId,
) -> Result<SourceSpan, OperationResult> {
    let SourceNodeKind::Element { start_tag, .. } =
        source.node(text).expect("source node exists").kind()
    else {
        return Err(unsupported("replace_text text node has no start tag"));
    };
    let end = start_tag
        .end
        .checked_sub(1)
        .ok_or_else(|| unsupported("replace_text text tag is invalid"))?;
    Ok(SourceSpan { start: end, end })
}

pub(super) fn verify_output_bytes(output: &[u8], replacement: &str) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let document = crate::DocxDocument::new(&source).map_err(document_invalid)?;
    let text = document
        .blocks()
        .map(|block| match block {
            crate::BodyBlock::Paragraph(paragraph) => {
                paragraph.text_for_view(RevisionView::Current)
            }
            crate::BodyBlock::Table(table) => table_current_text(table),
        })
        .collect::<Result<String, SemanticError>>()
        .map_err(document_invalid)?;
    text.contains(replacement).then_some(()).ok_or_else(|| {
        OperationResult::failed(
            "DOCUMENT_INVALID",
            format!(
                "output package {0} does not contain the replacement",
                main.name
            ),
        )
    })
}

pub(super) fn verify_output(output: &Path, replacement: &str) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let document = crate::DocxDocument::new(&source).map_err(document_invalid)?;
    if !document
        .blocks()
        .map(|block| match block {
            crate::BodyBlock::Paragraph(paragraph) => {
                paragraph.text_for_view(RevisionView::Current)
            }
            crate::BodyBlock::Table(table) => table_current_text(table),
        })
        .collect::<Result<String, SemanticError>>()
        .map_err(document_invalid)?
        .contains(replacement)
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            format!(
                "output package {0} does not contain the replacement",
                main.name
            ),
        ));
    }
    Ok(())
}

pub(super) fn text_child(source: &SourceDocument, id: NodeId) -> Option<NodeId> {
    source.children(id).next()
}

pub(super) fn is_cdata(source: &SourceDocument, id: NodeId) -> bool {
    let span = source.node(id).expect("source node exists").span();
    source.original_bytes()[..span.start].ends_with(b"<![CDATA[")
}

/// Exact selection shared by comment anchors and tracked text authoring.
pub(super) fn simple_body_text_range(
    source: &SourceDocument,
    target: &TextTarget,
    range_reason: &'static str,
) -> Result<(crate::text_search::ResolvedTextMatch, NodeId), OperationResult> {
    let matched = hyperlink::resolve_single_hyperlink_match(source, target)?;
    let paragraph = matched
        .segments
        .first()
        .and_then(|s| paragraph_ancestor(source, s.id))
        .ok_or_else(|| unsupported("selection has no paragraph"))?;
    if !safe_body_paragraph(source, paragraph)
        || source.node_ids().any(|id| {
            let span = source.node(id).unwrap().span();
            let p = source.node(paragraph).unwrap().span();
            span.start >= p.start
                && span.end <= p.end
                && ["fldChar", "instrText", "fldSimple"]
                    .iter()
                    .any(|local| word(source, id, local))
        })
        || matched
            .segments
            .iter()
            .any(|s| s.inside_tracked_change || paragraph_ancestor(source, s.id) != Some(paragraph))
    {
        return Err(
            unsupported("selection requires ordinary text in one direct body paragraph")
                .with_reason_code(range_reason),
        );
    }
    let mut runs = Vec::new();
    for segment in &matched.segments {
        let run = ordinary_run(source, segment.id).ok_or_else(|| {
            unsupported("selection crosses an inline wrapper")
                .with_reason_code("UNSUPPORTED_WRAPPER")
        })?;
        if source
            .children(run)
            .filter(|id| word(source, *id, "rPr"))
            .count()
            > 1
            || source
                .children(run)
                .filter(|id| word(source, *id, "t"))
                .count()
                != 1
            || source
                .children(run)
                .any(|id| !word(source, id, "rPr") && !word(source, id, "t"))
            || source.children(segment.id).count() != 1
            || is_cdata(source, segment.id)
            || source.children(run).any(|id| {
                word(source, id, "rPr") && source.children(id).any(|c| word(source, c, "rPrChange"))
            })
        {
            return Err(unsupported("selection boundary requires a simple text run")
                .with_reason_code("UNSAFE_RUN_STRUCTURE"));
        }
        runs.push((
            run,
            matched.start.max(segment.start) - segment.start,
            matched.end.min(segment.end) - segment.start,
            segment.source_text.as_str(),
        ));
    }
    // Reject hidden field/wrapper content between selected runs, rather than skipping it.
    let first = source.node(runs[0].0).unwrap().span();
    let last = source.node(runs.last().unwrap().0).unwrap().span();
    if source.children(paragraph).any(|id| {
        let span = source.node(id).unwrap().span();
        span.start >= first.start
            && span.end <= last.end
            && !runs.iter().any(|r| r.0 == id)
            && !matches!(source.node(id).unwrap().kind(), SourceNodeKind::Text)
            && ![
                "bookmarkStart",
                "bookmarkEnd",
                "commentRangeStart",
                "commentRangeEnd",
            ]
            .iter()
            .any(|name| word(source, id, name))
    }) {
        return Err(
            unsupported("selection contains unsupported field or wrapper boundaries")
                .with_reason_code(range_reason),
        );
    }
    Ok((matched, paragraph))
}

pub(super) fn text_run_pieces(
    source: &SourceDocument,
    run: NodeId,
    text: &str,
    start: usize,
    end: usize,
    deleted: bool,
) -> Result<(String, String, String), OperationResult> {
    let node = source.node(run).unwrap();
    let SourceNodeKind::Element {
        start_tag,
        end_tag: Some(end_tag),
        ..
    } = node.kind()
    else {
        return Err(unsupported("run has no source boundaries"));
    };
    let text_node = source
        .children(run)
        .find(|id| word(source, *id, "t"))
        .ok_or_else(|| unsupported("run has no text"))?;
    let SourceNodeKind::Element {
        start_tag: text_start,
        end_tag: Some(text_end),
        ..
    } = source.node(text_node).unwrap().kind()
    else {
        return Err(unsupported("text has no source boundaries"));
    };
    let bytes = source.original_bytes();
    let xml =
        |span: &SourceSpan| String::from_utf8_lossy(&bytes[span.start..span.end]).into_owned();
    let open = xml(start_tag);
    let close = xml(end_tag);
    let text_open = xml(text_start);
    let text_close = xml(text_end);
    let rpr = String::from_utf8_lossy(&run_properties(source, run)).into_owned();
    let make = |value: &str, deleted: bool| {
        let mut text_open = text_open.clone();
        let mut text_close = text_close.clone();
        if deleted {
            text_open = rename_text_tag(&text_open, "delText");
            text_close = rename_text_tag(&text_close, "delText");
        }
        if requires_space_preservation(value) {
            text_open = edit_attribute(text_open, "xml:space", Some("preserve"));
        }
        format!("{open}{rpr}{text_open}{}{text_close}{close}", escape(value))
    };
    Ok((
        if start > 0 {
            make(&text[..start], false)
        } else {
            String::new()
        },
        make(&text[start..end], deleted),
        if end < text.len() {
            make(&text[end..], false)
        } else {
            String::new()
        },
    ))
}

// Rename only a parsed text tag's local name, preserving its prefix and attributes.
pub(super) fn rename_text_tag(tag: &str, local: &str) -> String {
    let name_start = if tag.starts_with("</") { 2 } else { 1 };
    let name_end = name_start
        + tag[name_start..]
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .unwrap();
    let local_start = tag[name_start..name_end]
        .rfind(':')
        .map_or(name_start, |n| name_start + n + 1);
    let mut renamed = tag.to_owned();
    renamed.replace_range(local_start..name_end, local);
    renamed
}
