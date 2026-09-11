use super::*;

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

type FormattingRangeRun = (NodeId, usize, usize, String);

fn resolve_formatting_runs(
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

fn range_text_formatting_patches(
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

fn split_formatted_run(
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

fn text_formatting_patches(
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

fn new_text_formatting_children(
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

fn text_boolean_xml(
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

fn text_boolean_property(
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

fn text_simple_property(
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

fn font_xml(name: &impl Fn(&str) -> String, prefix: &str, value: &str) -> String {
    format!(
        "<{} {}ascii=\"{}\" {}hAnsi=\"{}\"/>",
        name("rFonts"),
        attr_prefix(prefix),
        escape(value),
        attr_prefix(prefix),
        escape(value)
    )
}

fn font_property(
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

fn rpr_rank(source: &SourceDocument, id: NodeId) -> usize {
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

fn rpr_insertion(
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

fn verify_text_formatting_output(
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

fn text_formatting_matches(
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
