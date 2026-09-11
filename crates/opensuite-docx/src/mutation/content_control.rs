use super::*;

/// Sets visible text in one simple semantic Word content control and returns verified DOCX bytes.
pub fn set_content_control_text_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetContentControlText,
) -> Result<Vec<u8>, OperationResult> {
    let target = resolve_content_control(source, &operation.target)?;
    if target.text != operation.expected_current_text {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "resolved content control text does not match expected current text",
        ));
    }
    let patches = if target.text.is_empty() {
        if operation.replacement.is_empty() {
            Vec::new()
        } else {
            vec![empty_cell_patch(
                source,
                target.paragraph,
                &operation.replacement,
            )?]
        }
    } else {
        let matches = crate::text_search::resolve_text(source, &target.text)
            .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
        let Some(matched) = matches.into_iter().find(|matched| {
            matched.start == 0
                && matched.end == target.text.len()
                && matched
                    .segments
                    .iter()
                    .all(|segment| is_descendant(source, segment.id, target.control))
        }) else {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "resolved content control has no compatible text source range",
            ));
        };
        patches_for_match(source, &matched, &operation.replacement)?
    };
    let patched = apply_patches(source, patches)?;
    let before = all_content_control_texts(source)?;
    let Some(index) = before.iter().position(|item| item.0 == target.control) else {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "resolved content control is unavailable",
        ));
    };
    let expected = before
        .into_iter()
        .enumerate()
        .map(|(item_index, (_, text))| {
            if item_index == index {
                operation.replacement.clone()
            } else {
                text
            }
        })
        .collect::<Vec<_>>();
    let output = package
        .write_replaced_part_to_vec(main, &patched)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    verify_content_control_output_bytes(
        &output,
        &operation.target,
        &operation.replacement,
        &expected,
    )?;
    Ok(output)
}

/// Sets visible text in one simple semantic Word content control.
pub fn set_content_control_text(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetContentControlText,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let bytes = match set_content_control_text_to_vec(package, main, source, operation) {
        Ok(bytes) => bytes,
        Err(error) => return error,
    };
    if let Err(error) = std::fs::write(output, bytes) {
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::content_control_text_set(
        operation.expected_current_text.clone(),
        operation.replacement.clone(),
    )
}

pub(super) struct ResolvedContentControl {
    control: NodeId,
    paragraph: NodeId,
    text: String,
}

pub(super) fn resolve_content_control(
    source: &SourceDocument,
    target: &ContentControlTarget,
) -> Result<ResolvedContentControl, OperationResult> {
    if target.tag.is_none() && target.alias.is_none() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "content control target requires tag or alias",
        ));
    }
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    let mut controls = document
        .content_controls()
        .filter(|control| {
            let properties = control.properties();
            target
                .tag
                .as_ref()
                .is_none_or(|tag| properties.tag.as_ref() == Some(tag))
                && target
                    .alias
                    .as_ref()
                    .is_none_or(|alias| properties.alias.as_ref() == Some(alias))
        })
        .collect::<Vec<_>>();
    if controls.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "content control target was not found",
        ));
    }
    let control = if let Some(occurrence) = target.occurrence {
        if occurrence >= controls.len() {
            return Err(OperationResult::failed(
                "TARGET_NOT_FOUND",
                "content control target occurrence was not found",
            ));
        }
        controls.remove(occurrence)
    } else if controls.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "content control target matches more than one control",
        ));
    } else {
        controls.pop().expect("one control")
    };
    let properties = control.properties();
    if !matches!(
        properties.kind,
        crate::ContentControlKind::Text | crate::ContentControlKind::RichText
    ) || properties.data_binding.is_some()
        || properties.lock.is_some()
        || control.properties_id().is_some_and(|id| {
            source
                .children(id)
                .any(|child| word(source, child, "showingPlcHdr"))
        })
    {
        return Err(unsupported(
            "set_content_control_text supports only unlocked, unbound text controls without placeholder state",
        ));
    }
    let Some(content) = control.content_id() else {
        return Err(unsupported("content control has no editable content"));
    };
    let paragraphs = source
        .children(content)
        .filter(|id| word(source, *id, "p"))
        .collect::<Vec<_>>();
    if paragraphs.len() != 1
        || source.children(content).any(|id| {
            matches!(
                source.node(id).map(|node| node.kind()),
                Some(SourceNodeKind::Element { .. })
            ) && !word(source, id, "p")
        })
        || !safe_table_paragraph(source, paragraphs[0])
    {
        return Err(unsupported(
            "set_content_control_text requires one ordinary direct paragraph",
        ));
    }
    let text = crate::tracked_change::text_for_view(source, paragraphs[0], RevisionView::Current)
        .map_err(document_invalid)?;
    Ok(ResolvedContentControl {
        control: control.source_id(),
        paragraph: paragraphs[0],
        text,
    })
}

pub(super) fn all_content_control_texts(
    source: &SourceDocument,
) -> Result<Vec<(NodeId, String)>, OperationResult> {
    crate::DocxDocument::new(source)
        .map_err(document_invalid)?
        .content_controls()
        .map(|control| {
            control
                .visible_text()
                .map(|text| (control.source_id(), text))
                .map_err(document_invalid)
        })
        .collect()
}

pub(super) fn verify_content_control_output_bytes(
    output: &[u8],
    target: &ContentControlTarget,
    replacement: &str,
    expected_controls: &[String],
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    if resolve_content_control(&source, target)?.text != replacement {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output content control text does not match replacement",
        ));
    }
    let actual = all_content_control_texts(&source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    (actual == expected_controls).then_some(()).ok_or_else(|| {
        OperationResult::failed(
            "DOCUMENT_INVALID",
            "output content controls do not preserve expected semantic structure",
        )
    })
}
