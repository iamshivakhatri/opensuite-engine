use super::*;

/// Sets selected direct paragraph-formatting properties without touching paragraph content.
pub fn set_paragraph_formatting(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetParagraphFormatting,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let (text, paragraph) = match resolve_paragraph_anchor(source, &operation.target) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let patches = match formatting_patches(source, paragraph, &operation.formatting) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let patched = match apply_patches(source, patches) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let temporary = temporary_path(output);
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) =
        verify_formatting_output(&temporary, &operation.target, &text, &operation.formatting)
    {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::paragraph_formatting_set(text)
}

pub fn set_paragraph_formatting_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetParagraphFormatting,
) -> Result<Vec<u8>, OperationResult> {
    let (_, paragraph) = resolve_paragraph_anchor(source, &operation.target)?;
    write_patches_to_vec(
        package,
        main,
        source,
        formatting_patches(source, paragraph, &operation.formatting)?,
    )
}

/// Sets or clears only the direct paragraph style reference.
pub fn set_paragraph_style(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetParagraphStyle,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let (text, paragraph) = match resolve_paragraph_anchor(source, &operation.target) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let styles = match crate::load_styles(package, main) {
        Ok(styles) => styles,
        Err(error) => return document_invalid(error),
    };
    let resolved_style = match resolve_paragraph_style(styles.as_ref(), &operation.style) {
        Ok(style) => style,
        Err(result) => return result,
    };
    let before = direct_paragraph_style_name(source, paragraph, styles.as_ref());
    let direct_formatting = match source
        .children(paragraph)
        .find(|id| word(source, *id, "pPr"))
        .map(|ppr| crate::styles::paragraph_formatting(source, ppr))
        .transpose()
    {
        Ok(value) => value.unwrap_or_default(),
        Err(error) => return document_invalid(error),
    };
    let patches = match paragraph_style_patches(
        source,
        paragraph,
        resolved_style.as_ref().map(|style| style.0.as_str()),
    ) {
        Ok(patches) => patches,
        Err(result) => return result,
    };
    let patched = match apply_patches(source, patches) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let temporary = temporary_path(output);
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) = verify_paragraph_style_output(
        &temporary,
        &operation.target,
        &text,
        resolved_style.as_ref(),
        &direct_formatting,
    ) {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    let after = resolved_style.map_or_else(|| "direct style cleared".to_owned(), |style| style.1);
    OperationResult::paragraph_style_set(text, before, after)
}

pub fn set_paragraph_style_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetParagraphStyle,
) -> Result<Vec<u8>, OperationResult> {
    let (_, paragraph) = resolve_paragraph_anchor(source, &operation.target)?;
    let styles = crate::load_styles(package, main).map_err(document_invalid)?;
    let style = resolve_paragraph_style(styles.as_ref(), &operation.style)?;
    write_patches_to_vec(
        package,
        main,
        source,
        paragraph_style_patches(
            source,
            paragraph,
            style.as_ref().map(|style| style.0.as_str()),
        )?,
    )
}

pub(super) fn resolve_paragraph_style(
    styles: Option<&crate::StyleSheet>,
    style: &PropertyPatch<String>,
) -> Result<Option<(String, String)>, OperationResult> {
    let PropertyPatch::Set(name) = style else {
        return Ok(None);
    };
    if name.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "paragraph style name must not be empty",
        ));
    }
    let Some(styles) = styles else {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "document has no stylesheet",
        ));
    };
    let matches = styles
        .styles()
        .filter(|style| style.name() == Some(name.as_str()))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "paragraph style name was not found",
        ));
    }
    if matches.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "paragraph style name matches more than one style",
        ));
    }
    (matches[0].style_type() == crate::StyleType::Paragraph)
        .then_some(Some((matches[0].id().as_str().to_owned(), name.clone())))
        .ok_or_else(|| unsupported("requested style is not a paragraph style"))
}

pub(super) fn direct_paragraph_style_name(
    source: &SourceDocument,
    paragraph: NodeId,
    styles: Option<&crate::StyleSheet>,
) -> String {
    let id = source
        .children(paragraph)
        .find(|id| word(source, *id, "pPr"))
        .and_then(|ppr| source.children(ppr).find(|id| word(source, *id, "pStyle")))
        .and_then(|id| source.node(id))
        .and_then(|node| node.attribute("val"));
    id.and_then(|id| {
        styles
            .and_then(|styles| styles.styles().find(|style| style.id().as_str() == id))
            .and_then(crate::Style::name)
    })
    .map(str::to_owned)
    .unwrap_or_else(|| "direct style cleared".to_owned())
}

pub(super) fn paragraph_style_patches(
    source: &SourceDocument,
    paragraph: NodeId,
    style_id: Option<&str>,
) -> Result<Vec<Patch>, OperationResult> {
    let prefix = word_prefix(source, paragraph)?;
    let name = |local: &str| qualify(prefix, local);
    let ppr = source
        .children(paragraph)
        .find(|id| word(source, *id, "pPr"));
    match (ppr, style_id) {
        (Some(ppr), Some(style)) => {
            let replacement = format!(
                "<{} {}val=\"{}\"/>",
                name("pStyle"),
                attr_prefix(prefix),
                escape(style)
            );
            if let Some(existing) = source.children(ppr).find(|id| word(source, *id, "pStyle")) {
                Ok(vec![Patch {
                    span: source.node(existing).expect("node").span(),
                    replacement: replacement.into_bytes(),
                }])
            } else {
                Ok(vec![ppr_insertion_at_start(
                    source,
                    ppr,
                    replacement.into_bytes(),
                )?])
            }
        }
        (Some(ppr), None) => Ok(source
            .children(ppr)
            .find(|id| word(source, *id, "pStyle"))
            .map(|id| Patch {
                span: source.node(id).expect("node").span(),
                replacement: Vec::new(),
            })
            .into_iter()
            .collect()),
        (None, Some(style)) => {
            let at = source
                .children(paragraph)
                .find_map(|id| {
                    source
                        .node(id)
                        .filter(|node| matches!(node.kind(), SourceNodeKind::Element { .. }))
                        .map(|node| node.span().start)
                })
                .ok_or_else(|| unsupported("paragraph has no insertion boundary"))?;
            Ok(vec![Patch {
                span: SourceSpan { start: at, end: at },
                replacement: format!(
                    "<{}><{} {}val=\"{}\"/></{}>",
                    name("pPr"),
                    name("pStyle"),
                    attr_prefix(prefix),
                    escape(style),
                    name("pPr")
                )
                .into_bytes(),
            }])
        }
        (None, None) => Ok(Vec::new()),
    }
}

pub(super) fn ppr_insertion_at_start(
    source: &SourceDocument,
    ppr: NodeId,
    replacement: Vec<u8>,
) -> Result<Patch, OperationResult> {
    if let Some(first) = source.children(ppr).next() {
        let at = source.node(first).expect("node").span().start;
        return Ok(Patch {
            span: SourceSpan { start: at, end: at },
            replacement,
        });
    }
    ppr_insertion(source, ppr, replacement)
}

/// Sets direct formatting on one complete, ordinary visible text run.
pub fn set_text_formatting(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetTextFormatting,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let (text, runs) = match resolve_formatting_runs(source, &operation.target) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let patches = match range_text_formatting_patches(source, &runs, &operation.formatting) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let patched = match apply_patches(source, patches) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let temporary = temporary_path(output);
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) =
        verify_text_formatting_output(&temporary, &operation.target, &text, &operation.formatting)
    {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::text_formatting_set(text)
}

pub fn set_text_formatting_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetTextFormatting,
) -> Result<Vec<u8>, OperationResult> {
    let (_, runs) = resolve_formatting_runs(source, &operation.target)?;
    write_patches_to_vec(
        package,
        main,
        source,
        range_text_formatting_patches(source, &runs, &operation.formatting)?,
    )
}

pub(super) type FormattingRangeRun = (NodeId, usize, usize, String);

pub(super) fn resolve_formatting_runs(
    source: &SourceDocument,
    target: &TextTarget,
) -> Result<(String, Vec<FormattingRangeRun>), OperationResult> {
    let matches = crate::text_search::resolve_text(source, &target.text)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    let matched = if matches.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "text target was not found",
        ));
    } else if let Some(occurrence) = target.occurrence {
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
    let mut runs = Vec::new();
    let mut paragraph = None;
    for segment in &matched.segments {
        if segment.inside_tracked_change
            || source.children(segment.id).nth(1).is_some()
            || is_cdata(source, segment.id)
        {
            return Err(unsupported(
                "set_text_formatting requires ordinary visible text source regions",
            ));
        }
        let run = ordinary_run(source, segment.id)
            .ok_or_else(|| unsupported("set_text_formatting does not edit inline wrappers"))?;
        let parent = source
            .node(run)
            .and_then(|node| node.parent())
            .ok_or_else(|| unsupported("run has no paragraph"))?;
        if paragraph
            .replace(parent)
            .is_some_and(|value| value != parent)
            || source
                .children(run)
                .any(|child| !word(source, child, "rPr") && !word(source, child, "t"))
        {
            return Err(unsupported(
                "set_text_formatting requires simple direct runs in one paragraph",
            ));
        }
        let start = matched.start.max(segment.start) - segment.start;
        let end = matched.end.min(segment.end) - segment.start;
        runs.push((run, start, end, segment.source_text.clone()));
    }
    let paragraph = paragraph.ok_or_else(|| unsupported("text range has no paragraph"))?;
    if !safe_body_paragraph(source, paragraph)
        || source.children(paragraph).any(|child| {
            word(source, child, "bookmarkStart")
                || word(source, child, "bookmarkEnd")
                || word(source, child, "commentRangeStart")
                || word(source, child, "commentRangeEnd")
                || word(source, child, "commentReference")
        })
    {
        return Err(unsupported(
            "set_text_formatting supports only an ordinary direct body run without ranges or wrappers",
        ));
    }
    Ok((matched.text, runs))
}

pub(super) fn range_text_formatting_patches(
    source: &SourceDocument,
    runs: &[FormattingRangeRun],
    patch: &TextFormattingPatch,
) -> Result<Vec<Patch>, OperationResult> {
    let mut patches = Vec::new();
    for (run, start, end, text) in runs {
        if *start == 0 && *end == text.len() {
            patches.extend(text_formatting_patches(source, *run, patch)?);
        } else {
            patches.push(Patch {
                span: source.node(*run).expect("run").span(),
                replacement: split_formatted_run(source, *run, text, *start, *end, patch)?,
            });
        }
    }
    Ok(patches)
}

pub(super) fn split_formatted_run(
    source: &SourceDocument,
    run: NodeId,
    text: &str,
    start: usize,
    end: usize,
    patch: &TextFormattingPatch,
) -> Result<Vec<u8>, OperationResult> {
    let paragraph = source
        .node(run)
        .and_then(|node| node.parent())
        .expect("run parent");
    let prefix = word_prefix(source, paragraph)?;
    let name = |local: &str| qualify(prefix, local);
    let start_tag = match source.node(run).expect("run").kind() {
        SourceNodeKind::Element { start_tag, .. } => *start_tag,
        _ => return Err(unsupported("run has no source tag")),
    };
    let open = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
        .map_err(|_| unsupported("run tag is not UTF-8"))?;
    let rpr = source
        .children(run)
        .find(|id| word(source, *id, "rPr"))
        .map(|id| {
            let span = source.node(id).expect("rpr").span();
            String::from_utf8_lossy(&source.original_bytes()[span.start..span.end]).into_owned()
        })
        .unwrap_or_default();
    let fragment = |value: &str, formatted: bool| -> Result<String, OperationResult> {
        let space = if requires_space_preservation(value) {
            " xml:space=\"preserve\""
        } else {
            ""
        };
        let raw = format!(
            "{open}{rpr}<{}{}>{}</{}></{}>",
            name("t"),
            space,
            escape(value),
            name("t"),
            name("r")
        );
        if !formatted {
            return Ok(raw);
        }
        let xml = format!(
            "<{} xmlns:{}=\"{}\"><{}><{}>{}</{}></{}></{}>",
            name("document"),
            prefix,
            NS[0],
            name("body"),
            name("p"),
            raw,
            name("p"),
            name("body"),
            name("document")
        );
        let doc = SourceDocument::parse(xml.into_bytes()).map_err(document_invalid)?;
        let p = doc
            .children(doc.root())
            .find(|id| word(&doc, *id, "body"))
            .and_then(|body| doc.children(body).find(|id| word(&doc, *id, "p")))
            .and_then(|paragraph| doc.children(paragraph).find(|id| word(&doc, *id, "r")))
            .ok_or_else(|| unsupported("formatted fragment is invalid"))?;
        let data = apply_patches(&doc, text_formatting_patches(&doc, p, patch)?)?;
        let updated = SourceDocument::parse(data).map_err(document_invalid)?;
        let run = updated
            .children(updated.root())
            .find(|id| word(&updated, *id, "body"))
            .and_then(|body| updated.children(body).find(|id| word(&updated, *id, "p")))
            .and_then(|paragraph| {
                updated
                    .children(paragraph)
                    .find(|id| word(&updated, *id, "r"))
            })
            .ok_or_else(|| unsupported("formatted fragment is invalid"))?;
        let span = updated.node(run).expect("run").span();
        Ok(
            String::from_utf8(updated.original_bytes()[span.start..span.end].to_vec())
                .expect("utf8"),
        )
    };
    let mut result = Vec::new();
    if start > 0 {
        result.extend(fragment(&text[..start], false)?.bytes());
    }
    result.extend(fragment(&text[start..end], true)?.bytes());
    if end < text.len() {
        result.extend(fragment(&text[end..], false)?.bytes());
    }
    Ok(result)
}

pub(super) fn text_formatting_patches(
    source: &SourceDocument,
    run: NodeId,
    patch: &TextFormattingPatch,
) -> Result<Vec<Patch>, OperationResult> {
    let prefix = word_prefix(
        source,
        source
            .node(run)
            .and_then(|node| node.parent())
            .expect("run parent"),
    )?;
    let name = |local: &str| qualify(prefix, local);
    let rpr = source.children(run).find(|id| word(source, *id, "rPr"));
    if rpr.is_none() {
        let children = new_text_formatting_children(patch, &name, prefix);
        if children.is_empty() {
            return Ok(Vec::new());
        }
        let at = source
            .children(run)
            .find_map(|id| source.node(id).map(|node| node.span().start))
            .ok_or_else(|| unsupported("run has no formatting insertion boundary"))?;
        return Ok(vec![Patch {
            span: SourceSpan { start: at, end: at },
            replacement: format!("<{}>{}</{}>", name("rPr"), children, name("rPr")).into_bytes(),
        }]);
    }
    let rpr = rpr.expect("checked");
    let mut patches = Vec::new();
    text_boolean_property(
        source,
        rpr,
        "b",
        patch.bold.as_ref(),
        prefix,
        2,
        &mut patches,
    )?;
    text_boolean_property(
        source,
        rpr,
        "i",
        patch.italic.as_ref(),
        prefix,
        3,
        &mut patches,
    )?;
    text_simple_property(
        source,
        rpr,
        "sz",
        patch
            .font_size_half_points
            .as_ref()
            .map(|value| match value {
                PropertyPatch::Set(value) => {
                    format!("<{} {}val=\"{}\"/>", name("sz"), attr_prefix(prefix), value)
                }
                PropertyPatch::Clear => String::new(),
            }),
        4,
        &mut patches,
    )?;
    font_property(
        source,
        rpr,
        patch.font_family.as_ref(),
        &name,
        prefix,
        &mut patches,
    )?;
    Ok(patches)
}

pub(super) fn new_text_formatting_children(
    patch: &TextFormattingPatch,
    name: &impl Fn(&str) -> String,
    prefix: &str,
) -> String {
    let mut result = String::new();
    if let Some(PropertyPatch::Set(value)) = &patch.font_family {
        result.push_str(&font_xml(name, prefix, value));
    }
    if let Some(PropertyPatch::Set(value)) = &patch.bold {
        result.push_str(&text_boolean_xml(name, "b", *value, prefix));
    }
    if let Some(PropertyPatch::Set(value)) = &patch.italic {
        result.push_str(&text_boolean_xml(name, "i", *value, prefix));
    }
    if let Some(PropertyPatch::Set(value)) = &patch.font_size_half_points {
        result.push_str(&format!(
            "<{} {}val=\"{}\"/>",
            name("sz"),
            attr_prefix(prefix),
            value
        ));
    }
    result
}

pub(super) fn text_boolean_xml(
    name: &impl Fn(&str) -> String,
    local: &str,
    value: bool,
    prefix: &str,
) -> String {
    if value {
        format!("<{} />", name(local))
    } else {
        format!("<{} {}val=\"0\"/>", name(local), attr_prefix(prefix))
    }
}

pub(super) fn text_boolean_property(
    source: &SourceDocument,
    rpr: NodeId,
    local: &str,
    value: Option<&PropertyPatch<bool>>,
    prefix: &str,
    rank: usize,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    let name = |local: &str| qualify(prefix, local);
    text_simple_property(
        source,
        rpr,
        local,
        value.map(|value| match value {
            PropertyPatch::Set(value) => text_boolean_xml(&name, local, *value, prefix),
            PropertyPatch::Clear => String::new(),
        }),
        rank,
        patches,
    )
}

pub(super) fn text_simple_property(
    source: &SourceDocument,
    rpr: NodeId,
    local: &str,
    replacement: Option<String>,
    rank: usize,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    match (
        source.children(rpr).find(|id| word(source, *id, local)),
        replacement,
    ) {
        (Some(id), Some(value)) => patches.push(Patch {
            span: source.node(id).expect("node").span(),
            replacement: value.into_bytes(),
        }),
        (None, Some(value)) if !value.is_empty() => {
            patches.push(rpr_insertion(source, rpr, value.into_bytes(), rank)?)
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn font_xml(name: &impl Fn(&str) -> String, prefix: &str, value: &str) -> String {
    format!(
        "<{} {}ascii=\"{}\" {}hAnsi=\"{}\"/>",
        name("rFonts"),
        attr_prefix(prefix),
        escape(value),
        attr_prefix(prefix),
        escape(value)
    )
}

pub(super) fn font_property(
    source: &SourceDocument,
    rpr: NodeId,
    value: Option<&PropertyPatch<String>>,
    name: &impl Fn(&str) -> String,
    prefix: &str,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    let Some(value) = value else { return Ok(()) };
    if let Some(id) = source.children(rpr).find(|id| word(source, *id, "rFonts")) {
        let SourceNodeKind::Element { start_tag, .. } = source.node(id).expect("node").kind()
        else {
            return Err(unsupported("font properties have no source tag"));
        };
        let mut tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
            .map_err(|_| unsupported("font properties tag is not UTF-8"))?
            .to_owned();
        let prefix = attr_prefix(prefix);
        match value {
            PropertyPatch::Set(value) => {
                let value = escape(value).into_owned();
                tag = edit_attribute(tag, &format!("{prefix}ascii"), Some(&value));
                tag = edit_attribute(tag, &format!("{prefix}hAnsi"), Some(&value));
            }
            PropertyPatch::Clear => {
                tag = edit_attribute(tag, &format!("{prefix}ascii"), None);
                tag = edit_attribute(tag, &format!("{prefix}hAnsi"), None);
            }
        }
        patches.push(Patch {
            span: *start_tag,
            replacement: tag.into_bytes(),
        });
    } else if let PropertyPatch::Set(value) = value {
        patches.push(rpr_insertion(
            source,
            rpr,
            font_xml(name, prefix, value).into_bytes(),
            1,
        )?);
    }
    Ok(())
}

pub(super) fn rpr_rank(source: &SourceDocument, id: NodeId) -> usize {
    if word(source, id, "rStyle") {
        0
    } else if word(source, id, "rFonts") {
        1
    } else if word(source, id, "b") {
        2
    } else if word(source, id, "i") {
        3
    } else if word(source, id, "sz") {
        4
    } else {
        100
    }
}

pub(super) fn rpr_insertion(
    source: &SourceDocument,
    rpr: NodeId,
    replacement: Vec<u8>,
    rank: usize,
) -> Result<Patch, OperationResult> {
    let SourceNodeKind::Element {
        start_tag, end_tag, ..
    } = source.node(rpr).expect("node").kind()
    else {
        return Err(unsupported("run properties have no source tag"));
    };
    if let Some(next) = source.children(rpr).find(|id| rpr_rank(source, *id) > rank) {
        return Ok(Patch {
            span: SourceSpan {
                start: source.node(next).expect("node").span().start,
                end: source.node(next).expect("node").span().start,
            },
            replacement,
        });
    }
    if let Some(end) = end_tag {
        return Ok(Patch {
            span: SourceSpan {
                start: end.start,
                end: end.start,
            },
            replacement,
        });
    }
    let tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
        .map_err(|_| unsupported("run properties tag is not UTF-8"))?;
    let name = tag
        .trim_start_matches('<')
        .split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        .next()
        .ok_or_else(|| unsupported("run properties tag is invalid"))?;
    let offset = tag
        .rfind("/>")
        .ok_or_else(|| unsupported("run properties cannot receive insertion"))?;
    let mut value = b">".to_vec();
    value.extend(replacement);
    value.extend(format!("</{name}>").bytes());
    Ok(Patch {
        span: SourceSpan {
            start: start_tag.start + offset,
            end: start_tag.end,
        },
        replacement: value,
    })
}

pub(super) fn formatting_patches(
    source: &SourceDocument,
    paragraph: NodeId,
    patch: &ParagraphFormattingPatch,
) -> Result<Vec<Patch>, OperationResult> {
    let prefix = word_prefix(source, paragraph)?;
    let name = |local: &str| qualify(prefix, local);
    let Some(ppr) = source
        .children(paragraph)
        .find(|id| word(source, *id, "pPr"))
    else {
        let children = new_formatting_children(patch, &name, prefix);
        if children.is_empty() {
            return Ok(Vec::new());
        }
        let at = source
            .children(paragraph)
            .find_map(|id| {
                source
                    .node(id)
                    .filter(|node| matches!(node.kind(), SourceNodeKind::Element { .. }))
                    .map(|node| node.span().start)
            })
            .ok_or_else(|| unsupported("paragraph has no insertion boundary"))?;
        return Ok(vec![Patch {
            span: SourceSpan { start: at, end: at },
            replacement: format!("<{}>{}</{}>", name("pPr"), children, name("pPr")).into_bytes(),
        }]);
    };
    let mut patches = Vec::new();
    simple_property(
        source,
        ppr,
        "jc",
        patch.alignment.as_ref().map(|value| match value {
            PropertyPatch::Set(value) => format!(
                "<{} {}val=\"{}\"/>",
                name("jc"),
                attr_prefix(prefix),
                alignment_value(*value)
            ),
            PropertyPatch::Clear => String::new(),
        }),
        &mut patches,
    )?;
    boolean_property(
        source,
        ppr,
        "keepNext",
        patch.keep_with_next.as_ref(),
        &name,
        prefix,
        &mut patches,
    )?;
    boolean_property(
        source,
        ppr,
        "keepLines",
        patch.keep_lines.as_ref(),
        &name,
        prefix,
        &mut patches,
    )?;
    compound_property(
        source,
        ppr,
        "spacing",
        spacing_changes(patch),
        &name,
        prefix,
        &mut patches,
    )?;
    compound_property(
        source,
        ppr,
        "ind",
        indent_changes(patch),
        &name,
        prefix,
        &mut patches,
    )?;
    Ok(patches)
}

pub(super) type AttrChange = (&'static str, Option<String>);

pub(super) fn alignment_value(value: opensuite_protocol::ParagraphAlignment) -> &'static str {
    match value {
        opensuite_protocol::ParagraphAlignment::Left => "left",
        opensuite_protocol::ParagraphAlignment::Center => "center",
        opensuite_protocol::ParagraphAlignment::Right => "right",
        opensuite_protocol::ParagraphAlignment::Both => "both",
        opensuite_protocol::ParagraphAlignment::Distribute => "distribute",
    }
}

pub(super) fn scalar_change(value: Option<&PropertyPatch<i32>>) -> Option<Option<String>> {
    value.map(|value| match value {
        PropertyPatch::Set(value) => Some(value.to_string()),
        PropertyPatch::Clear => None,
    })
}

pub(super) fn spacing_changes(patch: &ParagraphFormattingPatch) -> Vec<AttrChange> {
    let mut out = vec![];
    if let Some(value) = scalar_change(patch.spacing_before_twips.as_ref()) {
        out.push(("before", value));
    }
    if let Some(value) = scalar_change(patch.spacing_after_twips.as_ref()) {
        out.push(("after", value));
    }
    if let Some(value) = &patch.line_spacing {
        match value {
            PropertyPatch::Set(value) => {
                out.push(("line", Some(value.value.to_string())));
                out.push((
                    "lineRule",
                    value.rule.map(|rule| {
                        match rule {
                            opensuite_protocol::LineSpacingRule::Auto => "auto",
                            opensuite_protocol::LineSpacingRule::Exact => "exact",
                            opensuite_protocol::LineSpacingRule::AtLeast => "atLeast",
                        }
                        .to_owned()
                    }),
                ));
            }
            PropertyPatch::Clear => {
                out.push(("line", None));
                out.push(("lineRule", None));
            }
        }
    }
    out
}

pub(super) fn indent_changes(patch: &ParagraphFormattingPatch) -> Vec<AttrChange> {
    [
        ("left", scalar_change(patch.left_indent_twips.as_ref())),
        ("right", scalar_change(patch.right_indent_twips.as_ref())),
        (
            "firstLine",
            scalar_change(patch.first_line_indent_twips.as_ref()),
        ),
        (
            "hanging",
            scalar_change(patch.hanging_indent_twips.as_ref()),
        ),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| (key, value)))
    .collect()
}

pub(super) fn new_formatting_children(
    patch: &ParagraphFormattingPatch,
    name: &impl Fn(&str) -> String,
    prefix: &str,
) -> String {
    let mut out = String::new();
    if let Some(PropertyPatch::Set(value)) = &patch.keep_with_next {
        out.push_str(&boolean_xml(name, "keepNext", *value, prefix));
    }
    if let Some(PropertyPatch::Set(value)) = &patch.keep_lines {
        out.push_str(&boolean_xml(name, "keepLines", *value, prefix));
    }
    add_compound(&mut out, name, "spacing", spacing_changes(patch), prefix);
    add_compound(&mut out, name, "ind", indent_changes(patch), prefix);
    if let Some(PropertyPatch::Set(value)) = &patch.alignment {
        out.push_str(&format!(
            "<{} {}val=\"{}\"/>",
            name("jc"),
            attr_prefix(prefix),
            alignment_value(*value)
        ));
    }
    out
}

pub(super) fn add_compound(
    out: &mut String,
    name: &impl Fn(&str) -> String,
    local: &str,
    changes: Vec<AttrChange>,
    prefix: &str,
) {
    let values = changes
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
        .collect::<Vec<_>>();
    if !values.is_empty() {
        out.push_str(&format!("<{}", name(local)));
        for (key, value) in values {
            out.push_str(&format!(" {}{}=\"{}\"", attr_prefix(prefix), key, value));
        }
        out.push_str("/>");
    }
}

pub(super) fn simple_property(
    source: &SourceDocument,
    ppr: NodeId,
    local: &str,
    replacement: Option<String>,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    let existing = source.children(ppr).find(|id| word(source, *id, local));
    match (existing, replacement) {
        (Some(id), Some(value)) => patches.push(Patch {
            span: source.node(id).expect("node").span(),
            replacement: value.into_bytes(),
        }),
        (None, Some(value)) if !value.is_empty() => {
            patches.push(ppr_insertion(source, ppr, value.into_bytes())?)
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn boolean_xml(
    name: &impl Fn(&str) -> String,
    local: &str,
    value: bool,
    prefix: &str,
) -> String {
    if value {
        format!("<{} />", name(local))
    } else {
        format!("<{} {}val=\"0\"/>", name(local), attr_prefix(prefix))
    }
}

pub(super) fn boolean_property(
    source: &SourceDocument,
    ppr: NodeId,
    local: &str,
    value: Option<&PropertyPatch<bool>>,
    name: &impl Fn(&str) -> String,
    prefix: &str,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    let replacement = value.map(|value| match value {
        PropertyPatch::Set(value) => boolean_xml(name, local, *value, prefix),
        PropertyPatch::Clear => String::new(),
    });
    simple_property(source, ppr, local, replacement, patches)
}

pub(super) fn compound_property(
    source: &SourceDocument,
    ppr: NodeId,
    local: &str,
    changes: Vec<AttrChange>,
    name: &impl Fn(&str) -> String,
    prefix: &str,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    if changes.is_empty() {
        return Ok(());
    }
    if let Some(id) = source.children(ppr).find(|id| word(source, *id, local)) {
        let SourceNodeKind::Element { start_tag, .. } = source.node(id).expect("node").kind()
        else {
            return Err(unsupported("formatting property has no source tag"));
        };
        let mut tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
            .map_err(|_| unsupported("formatting property tag is not UTF-8"))?
            .to_owned();
        for (key, value) in changes {
            tag = edit_attribute(
                tag,
                &format!("{}{}", attr_prefix(prefix), key),
                value.as_deref(),
            );
        }
        patches.push(Patch {
            span: *start_tag,
            replacement: tag.into_bytes(),
        });
    } else {
        let mut value = String::new();
        add_compound(&mut value, name, local, changes, prefix);
        if !value.is_empty() {
            patches.push(ppr_insertion(source, ppr, value.into_bytes())?);
        }
    }
    Ok(())
}

pub(super) fn edit_attribute(mut tag: String, key: &str, value: Option<&str>) -> String {
    let needle = format!("{key}=\"");
    if let Some(start) = tag.find(&needle) {
        let end = start + needle.len() + tag[start + needle.len()..].find('"').unwrap_or(0) + 1;
        if let Some(value) = value {
            tag.replace_range(start..end, &format!("{key}=\"{value}\""));
        } else {
            let begin = tag[..start].rfind(char::is_whitespace).unwrap_or(start);
            tag.replace_range(begin..end, "");
        }
    } else if let Some(value) = value {
        let at = tag
            .rfind("/>")
            .or_else(|| tag.rfind('>'))
            .unwrap_or(tag.len());
        tag.insert_str(at, &format!(" {key}=\"{value}\""));
    }
    tag
}

pub(super) fn ppr_insertion(
    source: &SourceDocument,
    ppr: NodeId,
    replacement: Vec<u8>,
) -> Result<Patch, OperationResult> {
    let SourceNodeKind::Element {
        start_tag, end_tag, ..
    } = source.node(ppr).expect("node").kind()
    else {
        return Err(unsupported("paragraph properties have no source tag"));
    };
    if let Some(end) = end_tag {
        return Ok(Patch {
            span: SourceSpan {
                start: end.start,
                end: end.start,
            },
            replacement,
        });
    }
    let tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
        .map_err(|_| unsupported("paragraph properties tag is not UTF-8"))?;
    let name = tag
        .trim_start_matches('<')
        .split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        .next()
        .ok_or_else(|| unsupported("paragraph properties tag is invalid"))?;
    let offset = tag
        .rfind("/>")
        .ok_or_else(|| unsupported("paragraph properties cannot receive insertion"))?;
    let mut value = b">".to_vec();
    value.extend(replacement);
    value.extend(format!("</{name}>").bytes());
    Ok(Patch {
        span: SourceSpan {
            start: start_tag.start + offset,
            end: start_tag.end,
        },
        replacement: value,
    })
}

pub(super) fn verify_formatting_output(
    output: &Path,
    target: &TextTarget,
    text: &str,
    patch: &ParagraphFormattingPatch,
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (resolved, paragraph) = resolve_paragraph_anchor(&source, target)?;
    if resolved != text {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output target paragraph text changed",
        ));
    }
    let direct = source
        .children(paragraph)
        .find(|id| word(&source, *id, "pPr"))
        .map(|ppr| crate::styles::paragraph_formatting(&source, ppr))
        .transpose()
        .map_err(document_invalid)?
        .unwrap_or_default();
    formatting_matches(&direct, patch)
        .then_some(())
        .ok_or_else(|| {
            OperationResult::failed(
                "DOCUMENT_INVALID",
                "output paragraph formatting does not match request",
            )
        })
}

pub(super) fn verify_paragraph_style_output(
    output: &Path,
    target: &TextTarget,
    text: &str,
    style: Option<&(String, String)>,
    expected_formatting: &crate::styles::ParagraphFormatting,
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (resolved, paragraph) = resolve_paragraph_anchor(&source, target)?;
    if resolved != text {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output target paragraph text changed",
        ));
    }
    let ppr = source
        .children(paragraph)
        .find(|id| word(&source, *id, "pPr"));
    let direct = ppr
        .map(|id| crate::styles::paragraph_formatting(&source, id))
        .transpose()
        .map_err(document_invalid)?
        .unwrap_or_default();
    if &direct != expected_formatting {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output direct paragraph formatting changed",
        ));
    }
    let direct_id = ppr
        .and_then(|ppr| source.children(ppr).find(|id| word(&source, *id, "pStyle")))
        .and_then(|id| source.node(id))
        .and_then(|node| node.attribute("val"));
    match style {
        Some((id, name)) => {
            if direct_id != Some(id.as_str()) {
                return Err(OperationResult::failed(
                    "DOCUMENT_INVALID",
                    "output direct paragraph style does not match request",
                ));
            }
            let styles = crate::load_styles(&package, &main)
                .map_err(document_invalid)?
                .ok_or_else(|| {
                    OperationResult::failed("DOCUMENT_INVALID", "output stylesheet is unavailable")
                })?;
            if styles
                .styles()
                .find(|style| style.id().as_str() == id)
                .and_then(crate::Style::name)
                != Some(name.as_str())
            {
                return Err(OperationResult::failed(
                    "DOCUMENT_INVALID",
                    "output paragraph style name does not match request",
                ));
            }
        }
        None if direct_id.is_some() => {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "output direct paragraph style was not cleared",
            ));
        }
        None => {}
    }
    Ok(())
}

pub(super) fn verify_text_formatting_output(
    output: &Path,
    target: &TextTarget,
    text: &str,
    patch: &TextFormattingPatch,
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (resolved, runs) = resolve_formatting_runs(&source, target)?;
    if resolved != text {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output target text changed",
        ));
    }
    let direct = source
        .children(runs[0].0)
        .find(|id| word(&source, *id, "rPr"))
        .map(|rpr| crate::styles::run_formatting(&source, rpr))
        .transpose()
        .map_err(document_invalid)?
        .unwrap_or_default();
    text_formatting_matches(&direct, patch)
        .then_some(())
        .ok_or_else(|| {
            OperationResult::failed(
                "DOCUMENT_INVALID",
                "output direct run formatting does not match request",
            )
        })
}

pub(super) fn text_formatting_matches(
    actual: &crate::styles::RunFormatting,
    patch: &TextFormattingPatch,
) -> bool {
    matches_scalar(&actual.bold, patch.bold.as_ref(), |value| value)
        && matches_scalar(&actual.italic, patch.italic.as_ref(), |value| value)
        && matches_scalar(
            &actual.font_size_half_points,
            patch.font_size_half_points.as_ref(),
            |value| value,
        )
        && match patch.font_family.as_ref() {
            None => true,
            Some(PropertyPatch::Clear) => actual.font_family.is_none(),
            Some(PropertyPatch::Set(value)) => actual.font_family.as_ref() == Some(value),
        }
}

pub(super) fn formatting_matches(
    actual: &crate::styles::ParagraphFormatting,
    patch: &ParagraphFormattingPatch,
) -> bool {
    matches_scalar(
        &actual.alignment,
        patch.alignment.as_ref(),
        |value| match value {
            opensuite_protocol::ParagraphAlignment::Left => crate::styles::ParagraphAlignment::Left,
            opensuite_protocol::ParagraphAlignment::Center => {
                crate::styles::ParagraphAlignment::Center
            }
            opensuite_protocol::ParagraphAlignment::Right => {
                crate::styles::ParagraphAlignment::Right
            }
            opensuite_protocol::ParagraphAlignment::Both => crate::styles::ParagraphAlignment::Both,
            opensuite_protocol::ParagraphAlignment::Distribute => {
                crate::styles::ParagraphAlignment::Distribute
            }
        },
    ) && matches_scalar(
        &actual.spacing_before_twips,
        patch.spacing_before_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.spacing_after_twips,
        patch.spacing_after_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.left_indent_twips,
        patch.left_indent_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.right_indent_twips,
        patch.right_indent_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.first_line_indent_twips,
        patch.first_line_indent_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.hanging_indent_twips,
        patch.hanging_indent_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.keep_with_next,
        patch.keep_with_next.as_ref(),
        |value| value,
    ) && matches_scalar(&actual.keep_lines, patch.keep_lines.as_ref(), |value| value)
        && matches_line(&actual.line_spacing, patch.line_spacing.as_ref())
}

pub(super) fn matches_scalar<T: PartialEq, U>(
    actual: &Option<T>,
    patch: Option<&PropertyPatch<U>>,
    map: impl Fn(U) -> T,
) -> bool
where
    U: Copy,
{
    match patch {
        None => true,
        Some(PropertyPatch::Clear) => actual.is_none(),
        Some(PropertyPatch::Set(value)) => actual.as_ref() == Some(&map(*value)),
    }
}

pub(super) fn matches_line(
    actual: &Option<crate::styles::LineSpacing>,
    patch: Option<&PropertyPatch<opensuite_protocol::LineSpacing>>,
) -> bool {
    match patch {
        None => true,
        Some(PropertyPatch::Clear) => actual.is_none(),
        Some(PropertyPatch::Set(value)) => actual.as_ref().is_some_and(|actual| {
            actual.value == value.value
                && actual.rule
                    == value.rule.map(|rule| match rule {
                        opensuite_protocol::LineSpacingRule::Auto => {
                            crate::styles::LineSpacingRule::Auto
                        }
                        opensuite_protocol::LineSpacingRule::Exact => {
                            crate::styles::LineSpacingRule::Exact
                        }
                        opensuite_protocol::LineSpacingRule::AtLeast => {
                            crate::styles::LineSpacingRule::AtLeast
                        }
                    })
        }),
    }
}
