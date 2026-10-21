//! Move one source-backed paragraph without rebuilding its content.
use super::*;
use opensuite_protocol::{MoveParagraph, ParagraphMovePosition, ParagraphMoveTarget};

fn refused(message: &str) -> OperationResult {
    unsupported(message).with_reason_code("UNSUPPORTED_PARAGRAPH_MOVE")
}

fn resolve_target(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    target: &ParagraphMoveTarget,
) -> Result<NodeId, OperationResult> {
    let (handle, stamp) = target.handle.split_once(':').ok_or_else(|| {
        OperationResult::failed(
            "PRECONDITION_FAILED",
            "use a fresh movementHandle from body-block inspection",
        )
    })?;
    if stamp != format!("{:016x}", section_fingerprint(package, main, source)?) {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "paragraph movement handle is stale; inspect body blocks again",
        ));
    }
    let paragraph = resolve_body_block_handle(source, handle)?;
    if !word(source, paragraph, "p")
        || !safe_body_paragraph(source, paragraph)
        || crate::inspection::page_break_paragraph(source, paragraph)
    {
        return Err(refused(
            "movement requires ordinary direct body paragraphs without section boundaries",
        ));
    }
    if !source.children(paragraph).all(|id| {
        matches!(source.node(id).unwrap().kind(), SourceNodeKind::Text)
            || ((word(source, id, "pPr") || word(source, id, "r"))
                && safe_paragraph_child(source, id))
    }) {
        return Err(refused("paragraph contains unsupported inline content"));
    }
    let span = source.node(paragraph).unwrap().span();
    // Properties stay byte-identical, but unfamiliar structures may carry dependencies.
    for id in source.node_ids() {
        let node = source.node(id).unwrap();
        if node.span().start < span.start || node.span().end > span.end {
            continue;
        }
        if let SourceNodeKind::Element { name, .. } = node.kind() {
            if !name.namespace_uri().is_some_and(|uri| NS.contains(&uri))
                || name.local_name() == "framePr"
            {
                return Err(refused(
                    "paragraph contains unfamiliar XML with unknown movement dependencies",
                ));
            }
        }
    }
    protected_range::ensure_safe_source_ranges(
        source,
        &[span],
        "UNSUPPORTED_PARAGRAPH_MOVE",
        false,
    )?;
    Ok(paragraph)
}

pub fn move_paragraph_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &MoveParagraph,
) -> Result<Vec<u8>, OperationResult> {
    ensure_document_unprotected(package, main)?;
    let target = resolve_target(package, main, source, &operation.target)?;
    let destination = resolve_target(package, main, source, &operation.destination)?;
    let (body, blocks) = direct_body_blocks(source)?;
    // Reuse section validation; missing/ambiguous boundaries cannot establish ownership.
    let sections = sections::safe_sections(source)?;
    let owner = |id| {
        let start = source.node(id).unwrap().span().start;
        sections
            .iter()
            .position(|section| source.node(*section).unwrap().span().start > start)
    };
    if owner(target).is_none() || owner(target) != owner(destination) {
        return Err(refused("paragraphs must belong to the same section"));
    }
    let from = blocks.iter().position(|id| *id == target).unwrap();
    let to = blocks.iter().position(|id| *id == destination).unwrap();
    let after = operation.position == ParagraphMovePosition::After;
    let index = to + usize::from(after);
    let target_span = source.node(target).unwrap().span();
    let destination_span = source.node(destination).unwrap().span();
    let range_start = target_span.start.min(destination_span.start);
    let range_end = target_span.end.max(destination_span.end);
    if source.children(body).any(|id| {
        let node = source.node(id).unwrap();
        node.span().start >= range_start
            && node.span().end <= range_end
            && matches!(node.kind(), SourceNodeKind::Element { .. })
            && !word(source, id, "p")
            && !word(source, id, "tbl")
    }) {
        return Err(refused("cannot move across unsupported body structures"));
    }
    let at = if after {
        destination_span.end
    } else {
        destination_span.start
    };
    protected_range::ensure_safe_source_ranges(
        source,
        &[SourceSpan { start: at, end: at }],
        "UNSUPPORTED_PARAGRAPH_MOVE",
        false,
    )?;
    // Keep inter-paragraph whitespace and XML comments unchanged for logical no-ops.
    if index == from || index == from + 1 {
        return write_patches_to_vec(package, main, source, Vec::new());
    }
    let fragment = source.original_bytes()[target_span.start..target_span.end].to_vec();
    let patched = apply_patches(
        source,
        vec![
            Patch {
                span: target_span,
                replacement: Vec::new(),
            },
            Patch {
                span: SourceSpan { start: at, end: at },
                replacement: fragment,
            },
        ],
    )?;
    // Semantic blocks also include paragraphs inside unrelated content controls.
    let mut expected = body_texts(source)?;
    let text_from = expected.iter().position(|(id, _)| *id == target).unwrap();
    let text_to = expected
        .iter()
        .position(|(id, _)| *id == destination)
        .unwrap();
    let text_index = text_to + usize::from(after);
    let text = expected.remove(text_from);
    expected.insert(text_index - usize::from(text_from < text_index), text);
    let expected = expected
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    let new_index = index - usize::from(from < index);
    let mut expected_xml = blocks
        .iter()
        .map(|id| {
            let span = source.node(*id).unwrap().span();
            source.original_bytes()[span.start..span.end].to_vec()
        })
        .collect::<Vec<_>>();
    let xml = expected_xml.remove(from);
    expected_xml.insert(new_index, xml);
    // Both parents are the same body, so moving the full subtree retains namespace context.
    debug_assert_eq!(source.node(target).unwrap().parent(), Some(body));
    let output = package
        .write_replaced_part_to_vec(main, &patched)
        .map_err(document_invalid)?;
    let reopened = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    reopened.verify().map_err(document_invalid)?;
    let (_, new_source) = crate::open_main_source(&reopened).map_err(document_invalid)?;
    let actual = body_texts(&new_source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    let (_, new_blocks) = direct_body_blocks(&new_source)?;
    let actual_xml = new_blocks
        .iter()
        .map(|id| {
            let span = new_source.node(*id).unwrap().span();
            new_source.original_bytes()[span.start..span.end].to_vec()
        })
        .collect::<Vec<_>>();
    if actual != expected || actual_xml != expected_xml || new_source.original_bytes() != patched {
        return Err(document_invalid(
            "paragraph movement changed content or unrelated source XML",
        ));
    }
    Ok(output)
}
