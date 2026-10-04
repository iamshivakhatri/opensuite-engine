//! Real Word styles, using the existing stylesheet and formatting writers.
use super::*;
use crate::{StyleId, StyleSheet, StyleType};
use opensuite_protocol::{CreateStyle, UpdateStyle, WordStylePatch, WordStyleType};

pub fn create_style_to_vec(
    package: &Package,
    main: &Part,
    _source: &SourceDocument,
    operation: &CreateStyle,
) -> Result<Vec<u8>, OperationResult> {
    change_style(
        package,
        main,
        &operation.style_id,
        operation.style_type,
        &operation.properties,
        true,
    )
}

pub fn update_style_to_vec(
    package: &Package,
    main: &Part,
    _source: &SourceDocument,
    operation: &UpdateStyle,
) -> Result<Vec<u8>, OperationResult> {
    change_style(
        package,
        main,
        &operation.style_id,
        operation.style_type,
        &operation.properties,
        false,
    )
}

fn style_error(error: crate::StyleError) -> OperationResult {
    OperationResult::failed(error.code(), error.to_string()).with_reason_code(error.code())
}

fn change_style(
    package: &Package,
    main: &Part,
    id: &str,
    kind: WordStyleType,
    properties: &WordStylePatch,
    create: bool,
) -> Result<Vec<u8>, OperationResult> {
    if id.is_empty() || id.len() > 253 || id.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(OperationResult::failed(
            "INVALID_STYLE_ID",
            "style ID must be nonempty, at most 253 bytes, and contain no whitespace or control characters",
        ));
    }
    if let Some(name) = &properties.name {
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            return Err(OperationResult::failed(
                "INVALID_OPERATION",
                "style name must be nonempty and contain no control characters",
            ));
        }
    } else if create {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "creating a style requires an explicit display name",
        ));
    }
    validate_text_formatting(&properties.run)?;
    if matches!(
        properties.run.font_size_half_points,
        Some(PropertyPatch::Set(0))
    ) || matches!(&properties.run.font_family, Some(PropertyPatch::Set(v)) if v.trim().is_empty() || v.chars().any(char::is_control))
    {
        return Err(OperationResult::failed(
            "INVALID_FORMATTING_VALUE",
            "font name must be nonempty and font size must be positive",
        ));
    }
    if kind == WordStyleType::Character
        && (properties.next.is_some()
            || properties.paragraph != ParagraphFormattingPatch::default())
    {
        return Err(OperationResult::failed(
            "STYLE_TYPE_MISMATCH",
            "character styles support run formatting and basedOn only",
        ));
    }
    for value in [
        &properties.paragraph.spacing_before_twips,
        &properties.paragraph.spacing_after_twips,
        &properties.paragraph.first_line_indent_twips,
        &properties.paragraph.hanging_indent_twips,
    ] {
        if matches!(value, Some(PropertyPatch::Set(value)) if *value < 0) {
            return Err(OperationResult::failed(
                "INVALID_FORMATTING_VALUE",
                "spacing, first-line and hanging indentation must be nonnegative",
            ));
        }
    }
    if matches!(&properties.paragraph.line_spacing, Some(PropertyPatch::Set(value)) if value.value == 0)
    {
        return Err(OperationResult::failed(
            "INVALID_FORMATTING_VALUE",
            "line spacing must be positive",
        ));
    }
    let part = crate::styles::styles_part(package, main)
        .map_err(style_error)?
        .ok_or_else(|| {
            unsupported("document has no styles part; style part creation is not supported")
        })?;
    let styles = StyleSheet::parse(package.read_part(&part).map_err(document_invalid)?)
        .map_err(style_error)?;
    let source = styles.source();
    let style_id = StyleId::new(id.to_owned());
    let expected_type = match kind {
        WordStyleType::Paragraph => StyleType::Paragraph,
        WordStyleType::Character => StyleType::Character,
    };
    // Include unsupported table/numbering styles when checking identity collisions.
    let existing = source.children(source.root()).find(|node| {
        word(source, *node, "style")
            && source.node(*node).and_then(|n| n.attribute("styleId")) == Some(id)
    });
    if create && existing.is_some() {
        return Err(OperationResult::failed(
            "DUPLICATE_STYLE_ID",
            format!("style ID already exists: {id}"),
        ));
    }
    if !create {
        let style = styles.style(&style_id).ok_or_else(|| {
            OperationResult::failed("MISSING_STYLE", format!("missing style: {id}"))
        })?;
        if style.style_type() != expected_type {
            return Err(OperationResult::failed(
                "STYLE_TYPE_MISMATCH",
                "style type cannot be changed",
            ));
        }
    }
    if let Some(name) = &properties.name {
        for node in source
            .children(source.root())
            .filter(|node| word(source, *node, "style"))
        {
            if Some(node) == existing {
                continue;
            }
            if source
                .children(node)
                .filter(|child| word(source, *child, "name"))
                .any(|child| {
                    source
                        .node(child)
                        .and_then(|n| n.attribute("val"))
                        .is_some_and(|value| value.eq_ignore_ascii_case(name))
                })
            {
                return Err(OperationResult::failed(
                    "DUPLICATE_STYLE_NAME",
                    format!("style display name already exists: {name}"),
                ));
            }
        }
    }
    let prefix = word_prefix_for(
        source,
        existing.unwrap_or(source.root()),
        if create { "styles" } else { "style" },
    )?;
    let prefix = prefix.as_str();
    let name = |local: &str| qualify(prefix, local);
    let mut patches = Vec::new();
    if create {
        let xml = format!(
            "<{} {}type=\"{}\" {}customStyle=\"1\" {}styleId=\"{}\">{}</{}>",
            name("style"),
            attr_prefix(prefix),
            if kind == WordStyleType::Paragraph {
                "paragraph"
            } else {
                "character"
            },
            attr_prefix(prefix),
            attr_prefix(prefix),
            escape(id),
            new_style_children(properties, prefix),
            name("style")
        );
        patches.push(ppr_insertion(source, source.root(), xml.into_bytes())?);
    } else {
        let node = existing.expect("checked");
        for (local, value) in [
            (
                "name",
                properties
                    .name
                    .as_ref()
                    .map(|v| PropertyPatch::Set(v.clone())),
            ),
            ("basedOn", properties.based_on.clone()),
            ("next", properties.next.clone()),
        ] {
            if let Some(value) = value {
                let child = source
                    .children(node)
                    .find(|child| word(source, *child, local));
                match (child, value) {
                    (Some(child), PropertyPatch::Clear) => patches.push(Patch {
                        span: source.node(child).expect("child").span(),
                        replacement: vec![],
                    }),
                    (Some(child), PropertyPatch::Set(value)) => {
                        let SourceNodeKind::Element { start_tag, .. } =
                            source.node(child).expect("child").kind()
                        else {
                            unreachable!()
                        };
                        let tag = String::from_utf8_lossy(
                            &source.original_bytes()[start_tag.start..start_tag.end],
                        )
                        .into_owned();
                        patches.push(Patch {
                            span: *start_tag,
                            replacement: edit_attribute(
                                tag,
                                &format!("{}val", attr_prefix(prefix)),
                                Some(&escape(&value)),
                            )
                            .into_bytes(),
                        });
                    }
                    (None, PropertyPatch::Set(value)) => patches.push(style_insertion(
                        source,
                        node,
                        local,
                        value_xml(prefix, local, &value).into_bytes(),
                    )?),
                    _ => {}
                }
            }
        }
        for local in ["pPr", "rPr"] {
            let child = source
                .children(node)
                .find(|child| word(source, *child, local));
            let children = if local == "pPr" {
                new_formatting_children(&properties.paragraph, &name, prefix)
            } else {
                new_text_formatting_children(&properties.run, &name, prefix)
            };
            if let Some(child) = child {
                patches.extend(if local == "pPr" {
                    paragraph_property_patches(source, child, &properties.paragraph)?
                } else {
                    run_property_patches(source, child, &properties.run)?
                });
            } else if !children.is_empty() {
                patches.push(style_insertion(
                    source,
                    node,
                    local,
                    format!("<{}>{children}</{}>", name(local), name(local)).into_bytes(),
                )?);
            }
        }
    }
    merge_empty_insertions(&mut patches)?;
    let patched = apply_patches(source, patches)?;
    // The same authoritative parser and inheritance traversal validate the candidate BEFORE saving.
    let candidate = StyleSheet::parse(patched.clone()).map_err(style_error)?;
    candidate
        .style_run_formatting(&style_id, expected_type)
        .map_err(style_error)?;
    if let Some(next) = candidate.style(&style_id).and_then(|style| style.next()) {
        if candidate
            .style(next)
            .is_none_or(|style| style.style_type() != StyleType::Paragraph)
        {
            return Err(OperationResult::failed(
                "INVALID_STYLE_NEXT",
                "next style must name an existing paragraph style",
            ));
        }
    }
    let output = package
        .write_replaced_part_to_vec(&part, &patched)
        .map_err(document_invalid)?;
    let reopened = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    reopened.verify().map_err(document_invalid)?;
    // Check the saved stylesheet bytes, as well as main-document readability.
    let saved = reopened.part(&part.name).map_err(document_invalid)?;
    if reopened.read_part(&saved).map_err(document_invalid)? != patched {
        return Err(document_invalid(
            "saved styles differ from the verified candidate",
        ));
    }
    crate::open_main_source(&reopened).map_err(document_invalid)?;
    Ok(output)
}

fn value_xml(prefix: &str, local: &str, value: &str) -> String {
    format!(
        "<{} {}val=\"{}\"/>",
        qualify(prefix, local),
        attr_prefix(prefix),
        escape(value)
    )
}

fn new_style_children(properties: &WordStylePatch, prefix: &str) -> String {
    let name = |local: &str| qualify(prefix, local);
    let mut xml = value_xml(
        prefix,
        "name",
        properties.name.as_deref().expect("validated"),
    );
    for (local, value) in [
        ("basedOn", &properties.based_on),
        ("next", &properties.next),
    ] {
        if let Some(PropertyPatch::Set(value)) = value {
            xml.push_str(&value_xml(prefix, local, value));
        }
    }
    for (local, children) in [
        (
            "pPr",
            new_formatting_children(&properties.paragraph, &name, prefix),
        ),
        (
            "rPr",
            new_text_formatting_children(&properties.run, &name, prefix),
        ),
    ] {
        if !children.is_empty() {
            xml.push_str(&format!("<{}>{children}</{}>", name(local), name(local)));
        }
    }
    xml
}

fn style_rank(local: &str) -> usize {
    match local {
        "name" => 0,
        "aliases" => 1,
        "basedOn" => 2,
        "next" => 3,
        "link" => 4,
        "autoRedefine" | "hidden" | "uiPriority" | "semiHidden" | "unhideWhenUsed" | "qFormat"
        | "locked" | "personal" | "personalCompose" | "personalReply" | "rsid" => 5,
        "pPr" => 6,
        "rPr" => 7,
        _ => 8,
    }
}

fn style_insertion(
    source: &SourceDocument,
    node: NodeId,
    local: &str,
    replacement: Vec<u8>,
) -> Result<Patch, OperationResult> {
    if let Some(next) = source.children(node).find(|id| matches!(source.node(*id).map(|n| n.kind()), Some(SourceNodeKind::Element { name, .. }) if style_rank(name.local_name()) > style_rank(local))) {
        let at = source.node(next).expect("node").span().start;
        return Ok(Patch { span: SourceSpan { start: at, end: at }, replacement });
    }
    ppr_insertion(source, node, replacement)
}

// Multiple new children of one self-closing source element share its / > boundary.
fn merge_empty_insertions(patches: &mut Vec<Patch>) -> Result<(), OperationResult> {
    let mut merged: Vec<Patch> = Vec::new();
    for patch in patches.drain(..) {
        if patch.span.start != patch.span.end {
            if let Some(previous) = merged.iter_mut().find(|p| p.span == patch.span) {
                let close = previous
                    .replacement
                    .windows(2)
                    .rposition(|bytes| bytes == b"</")
                    .ok_or_else(|| unsupported("overlapping style property patches"))?;
                let next_close = patch
                    .replacement
                    .windows(2)
                    .rposition(|bytes| bytes == b"</")
                    .ok_or_else(|| unsupported("overlapping style property patches"))?;
                if previous.replacement[close..] != patch.replacement[next_close..]
                    || patch.replacement.first() != Some(&b'>')
                {
                    return Err(unsupported("overlapping style property patches"));
                }
                previous.replacement.splice(
                    close..close,
                    patch.replacement[1..next_close].iter().copied(),
                );
                continue;
            }
        }
        merged.push(patch);
    }
    *patches = merged;
    Ok(())
}

#[cfg(test)]
mod tests;
