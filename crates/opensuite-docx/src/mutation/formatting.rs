use super::*;

mod text;

pub use text::*;

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
