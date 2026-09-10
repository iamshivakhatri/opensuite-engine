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
