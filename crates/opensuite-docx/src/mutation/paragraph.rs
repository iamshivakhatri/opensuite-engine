use super::*;

/// Inserts one plain paragraph after a safe, direct main-body paragraph anchor.
pub fn insert_paragraph_after(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertParagraphAfter,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let (anchor, paragraph) = match resolve_paragraph_anchor(source, &operation.anchor) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let output_bytes = match insert_paragraph_bytes(
        package,
        main,
        source,
        std::slice::from_ref(&operation.text),
        ResolvedParagraphPlacement::After(paragraph),
    ) {
        Ok(bytes) => bytes,
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
    if let Err(error) = std::fs::write(&temporary, output_bytes) {
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    if let Err(result) =
        verify_inserted_output(&temporary, &operation.anchor, &anchor, &operation.text)
    {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::paragraph_inserted(anchor, operation.text.clone())
}

/// Inserts one plain paragraph at a direct-body placement and returns verified DOCX bytes.
pub fn insert_paragraph_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertParagraph,
) -> Result<Vec<u8>, OperationResult> {
    let placement = resolve_paragraph_placement(source, &operation.placement)?;
    insert_paragraph_bytes(
        package,
        main,
        source,
        std::slice::from_ref(&operation.text),
        placement,
    )
}

/// Inserts several plain paragraphs as one verified source patch.
pub fn insert_paragraphs_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertParagraphs,
) -> Result<Vec<u8>, OperationResult> {
    if operation.texts.is_empty() || operation.texts.len() > MAX_PARAGRAPHS_PER_OPERATION {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            format!(
                "insert_paragraphs requires between 1 and {MAX_PARAGRAPHS_PER_OPERATION} paragraphs"
            ),
        ));
    }
    let placement = resolve_paragraph_placement(source, &operation.placement)?;
    insert_paragraph_bytes(package, main, source, &operation.texts, placement)
}

pub fn delete_paragraph(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &DeleteParagraph,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let (_target, paragraph) = match resolve_paragraph_anchor(source, &operation.target) {
        Ok(value) => value,
        Err(result) => return result,
    };
    if !safe_to_delete(source, paragraph) {
        return unsupported(
            "delete_paragraph rejects paragraphs with ranges, fields, controls, revisions, or unsupported wrappers",
        );
    }
    let body = match body_texts(source) {
        Ok(body) => body,
        Err(result) => return result,
    };
    let paragraph_text = match crate::DocxDocument::new(source)
        .map_err(document_invalid)
        .and_then(|document| {
            document
                .paragraphs()
                .find(|item| item.source_id() == paragraph)
                .ok_or_else(|| {
                    OperationResult::failed(
                        "DOCUMENT_INVALID",
                        "anchor paragraph is not in the body",
                    )
                })
                .and_then(|item| {
                    item.text_for_view(RevisionView::Current)
                        .map_err(document_invalid)
                })
        }) {
        Ok(text) => text,
        Err(result) => return result,
    };
    let body_index = match body.iter().position(|item| item.0 == paragraph) {
        Some(index) => index,
        None => {
            return OperationResult::failed(
                "DOCUMENT_INVALID",
                "anchor paragraph is not a body block",
            );
        }
    };
    let expected_body = body.into_iter().map(|(_, text)| text).collect::<Vec<_>>();
    let mut expected_after = expected_body.clone();
    expected_after.remove(body_index);
    let span = source
        .node(paragraph)
        .expect("anchor paragraph exists")
        .span();
    let patched = match apply_patches(
        source,
        vec![Patch {
            span,
            replacement: Vec::new(),
        }],
    ) {
        Ok(patched) => patched,
        Err(result) => return result,
    };
    let temporary = temporary_path(output);
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) = verify_deleted_output(&temporary, &expected_after) {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::paragraph_deleted(paragraph_text)
}

pub fn delete_paragraph_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &DeleteParagraph,
) -> Result<Vec<u8>, OperationResult> {
    let (_, paragraph) = resolve_paragraph_anchor(source, &operation.target)?;
    if !safe_to_delete(source, paragraph) {
        return Err(unsupported(
            "delete_paragraph rejects paragraphs with ranges, fields, controls, revisions, or unsupported wrappers",
        ));
    }
    write_patches_to_vec(
        package,
        main,
        source,
        vec![Patch {
            span: source
                .node(paragraph)
                .expect("anchor paragraph exists")
                .span(),
            replacement: Vec::new(),
        }],
    )
}

pub(super) fn resolve_paragraph_anchor(
    source: &SourceDocument,
    target: &TextTarget,
) -> Result<(String, NodeId), OperationResult> {
    let matches = crate::text_search::resolve_text(source, &target.text)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    if matches.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "text target was not found",
        ));
    }
    let matched = if let Some(occurrence) = target.occurrence {
        matches.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "text target occurrence was not found")
        })?
    } else if matches.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "text target matches more than one current semantic range",
        ));
    } else {
        matches.into_iter().next().expect("one match")
    };
    let paragraph = matched
        .segments
        .first()
        .and_then(|segment| paragraph_ancestor(source, segment.id))
        .ok_or_else(|| unsupported("insert_paragraph_after requires a paragraph anchor"))?;
    if matched
        .segments
        .iter()
        .any(|segment| paragraph_ancestor(source, segment.id) != Some(paragraph))
        || !safe_body_paragraph(source, paragraph)
    {
        return Err(unsupported(
            "insert_paragraph_after supports only ordinary direct body paragraphs",
        ));
    }
    Ok((matched.text, paragraph))
}

pub(super) fn paragraph_ancestor(source: &SourceDocument, mut id: NodeId) -> Option<NodeId> {
    loop {
        if word(source, id, "p") {
            return Some(id);
        }
        id = source.node(id)?.parent()?;
    }
}

pub(super) fn safe_body_paragraph(source: &SourceDocument, paragraph: NodeId) -> bool {
    let Some(body) = source.node(paragraph).and_then(|node| node.parent()) else {
        return false;
    };
    word(source, body, "body")
        && source
            .node(body)
            .and_then(|node| node.parent())
            .is_some_and(|document| word(source, document, "document"))
        && !source.children(paragraph).any(|id| {
            word(source, id, "pPr")
                && source
                    .children(id)
                    .any(|child| word(source, child, "sectPr"))
        })
        && !has_revision_wrapper(source, paragraph)
}

pub(super) fn safe_to_delete(source: &SourceDocument, paragraph: NodeId) -> bool {
    source
        .children(paragraph)
        .all(|child| safe_paragraph_child(source, child))
}

pub(super) fn safe_paragraph_child(source: &SourceDocument, id: NodeId) -> bool {
    if word(source, id, "pPr") {
        return !source
            .children(id)
            .any(|child| word(source, child, "pPrChange"));
    }
    if word(source, id, "r") {
        return source
            .children(id)
            .all(|child| safe_run_child(source, child));
    }
    word(source, id, "hyperlink")
        && source
            .children(id)
            .all(|child| word(source, child, "r") && safe_paragraph_child(source, child))
}

pub(super) fn safe_run_child(source: &SourceDocument, id: NodeId) -> bool {
    if word(source, id, "rPr") {
        return true;
    }
    word(source, id, "t")
        || word(source, id, "tab")
        || word(source, id, "br")
        || word(source, id, "cr")
        || word(source, id, "noBreakHyphen")
        || word(source, id, "softHyphen")
        || word(source, id, "lastRenderedPageBreak")
}

pub(super) fn has_revision_wrapper(source: &SourceDocument, id: NodeId) -> bool {
    word(source, id, "ins")
        || word(source, id, "del")
        || word(source, id, "moveFrom")
        || word(source, id, "moveTo")
        || source
            .children(id)
            .any(|child| has_revision_wrapper(source, child))
}

pub(super) fn body_texts(
    source: &SourceDocument,
) -> Result<Vec<(NodeId, String)>, OperationResult> {
    crate::DocxDocument::new(source)
        .map_err(document_invalid)?
        .blocks()
        .map(|block| match block {
            crate::BodyBlock::Paragraph(paragraph) => paragraph
                .text_for_view(RevisionView::Current)
                .map(|text| (paragraph.source_id(), text))
                .map_err(document_invalid),
            crate::BodyBlock::Table(table) => {
                let source_id = table.source_id();
                table_current_text(table)
                    .map(|text| (source_id, text))
                    .map_err(document_invalid)
            }
        })
        .collect()
}

pub(super) fn insert_paragraph_bytes(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    texts: &[String],
    placement: ResolvedParagraphPlacement,
) -> Result<Vec<u8>, OperationResult> {
    let (body, blocks) = direct_body_blocks(source)?;
    let index = match placement {
        ResolvedParagraphPlacement::Start => 0,
        ResolvedParagraphPlacement::End => blocks.len(),
        ResolvedParagraphPlacement::Before(anchor) => {
            blocks.iter().position(|id| *id == anchor).ok_or_else(|| {
                OperationResult::failed("TARGET_NOT_FOUND", "body block handle was not found")
            })?
        }
        ResolvedParagraphPlacement::After(anchor) => blocks
            .iter()
            .position(|id| *id == anchor)
            .map(|index| index + 1)
            .ok_or_else(|| {
                OperationResult::failed("TARGET_NOT_FOUND", "body block handle was not found")
            })?,
    };
    let section = terminal_section_properties(source, body)?;
    let insertion = if index < blocks.len() {
        source
            .node(blocks[index])
            .expect("body block exists")
            .span()
            .start
    } else if let Some(section) = section {
        source
            .node(section)
            .expect("section properties exist")
            .span()
            .start
    } else {
        body_closing_start(source, body)?
    };
    let fragment = texts.iter().try_fold(Vec::new(), |mut fragment, text| {
        fragment.extend(paragraph_fragment_for_body(source, body, text)?);
        Ok::<_, OperationResult>(fragment)
    })?;
    let patched = apply_patches(
        source,
        vec![Patch {
            span: SourceSpan {
                start: insertion,
                end: insertion,
            },
            replacement: fragment,
        }],
    )?;
    let output = package
        .write_replaced_part_to_vec(main, &patched)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    verify_inserted_body_bytes(&output, index, texts)?;
    Ok(output)
}

pub(super) fn resolve_paragraph_placement(
    source: &SourceDocument,
    placement: &ParagraphPlacement,
) -> Result<ResolvedParagraphPlacement, OperationResult> {
    match placement {
        ParagraphPlacement::Start => Ok(ResolvedParagraphPlacement::Start),
        ParagraphPlacement::End => Ok(ResolvedParagraphPlacement::End),
        ParagraphPlacement::Before { handle } => Ok(ResolvedParagraphPlacement::Before(
            resolve_body_block_handle(source, handle)?,
        )),
        ParagraphPlacement::After { handle } => Ok(ResolvedParagraphPlacement::After(
            resolve_body_block_handle(source, handle)?,
        )),
    }
}

pub(super) fn paragraph_fragment_for_body(
    source: &SourceDocument,
    body: NodeId,
    text: &str,
) -> Result<Vec<u8>, OperationResult> {
    let prefix = word_prefix_for(source, body, "body")?;
    paragraph_fragment_with_prefix(&prefix, text)
}

#[cfg(test)]
pub(super) fn paragraph_fragment(
    source: &SourceDocument,
    paragraph: NodeId,
    text: &str,
) -> Result<Vec<u8>, OperationResult> {
    let prefix = word_prefix(source, paragraph)?;
    paragraph_fragment_with_prefix(prefix, text)
}

pub(super) fn paragraph_fragment_with_prefix(
    prefix: &str,
    text: &str,
) -> Result<Vec<u8>, OperationResult> {
    let name = |local: &str| {
        if prefix.is_empty() {
            local.to_owned()
        } else {
            format!("{prefix}:{local}")
        }
    };
    if text.is_empty() {
        return Ok(format!("<{}></{}>", name("p"), name("p")).into_bytes());
    }
    let space = if requires_space_preservation(text) {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    Ok(format!(
        "<{}><{}><{}{}>{}</{}></{}></{}>",
        name("p"),
        name("r"),
        name("t"),
        space,
        escape(text),
        name("t"),
        name("r"),
        name("p")
    )
    .into_bytes())
}

pub(super) fn verify_inserted_body_bytes(
    output: &[u8],
    expected_index: usize,
    inserted_texts: &[String],
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (_, blocks) = direct_body_blocks(&source)?;
    for (offset, expected) in inserted_texts.iter().enumerate() {
        let paragraph = blocks.get(expected_index + offset).ok_or_else(|| {
            OperationResult::failed("DOCUMENT_INVALID", "inserted paragraph was not found")
        })?;
        if !word(&source, *paragraph, "p") {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "inserted paragraph is not at the requested body position",
            ));
        }
        let text = crate::tracked_change::text_for_view(&source, *paragraph, RevisionView::Current)
            .map_err(document_invalid)?;
        if text != *expected {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "inserted paragraph text does not match request",
            ));
        }
    }
    Ok(())
}

pub(super) fn verify_inserted_output(
    output: &Path,
    target: &TextTarget,
    anchor_text: &str,
    inserted_text: &str,
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (resolved_anchor, anchor) = resolve_paragraph_anchor(&source, target)?;
    if resolved_anchor != anchor_text {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output anchor text changed",
        ));
    }
    let document = crate::DocxDocument::new(&source).map_err(document_invalid)?;
    let mut blocks = document.blocks();
    while let Some(block) = blocks.next() {
        if matches!(block, crate::BodyBlock::Paragraph(ref paragraph) if paragraph.source_id() == anchor)
        {
            let Some(crate::BodyBlock::Paragraph(inserted)) = blocks.next() else {
                return Err(OperationResult::failed(
                    "DOCUMENT_INVALID",
                    "inserted paragraph is not immediately after anchor",
                ));
            };
            return (inserted
                .text_for_view(RevisionView::Current)
                .map_err(document_invalid)?
                == inserted_text)
                .then_some(())
                .ok_or_else(|| {
                    OperationResult::failed(
                        "DOCUMENT_INVALID",
                        "inserted paragraph text does not match request",
                    )
                });
        }
    }
    Err(OperationResult::failed(
        "DOCUMENT_INVALID",
        "output anchor paragraph was not found",
    ))
}

pub(super) fn verify_deleted_output(
    output: &Path,
    expected_body: &[String],
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let actual = body_texts(&source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    (actual == expected_body).then_some(()).ok_or_else(|| {
        OperationResult::failed(
            "DOCUMENT_INVALID",
            "output body does not preserve expected neighboring structure",
        )
    })
}
