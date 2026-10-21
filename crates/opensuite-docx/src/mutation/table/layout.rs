//! Exact table separators and direct row pagination properties.
use super::*;
use opensuite_protocol::{SetTableRowLayout, SetTableSpacing, TableLayoutTarget};

fn invalid(message: &str) -> OperationResult {
    OperationResult::failed("PRECONDITION_FAILED", message)
}

fn resolve_layout_table(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    target: &TableLayoutTarget,
) -> Result<(usize, NodeId), OperationResult> {
    let (handle, stamp) = target
        .handle
        .rsplit_once(':')
        .ok_or_else(|| invalid("table layout needs a fresh layoutHandle from table inspection"))?;
    if stamp
        != format!(
            "{:016x}",
            sections::section_fingerprint(package, main, source)?
        )
    {
        return Err(invalid(
            "table layout handle is stale; inspect tables again",
        ));
    }
    ensure_document_unprotected(package, main)
        .map_err(|error| error.with_reason_code("UNSUPPORTED_TABLE_LAYOUT"))?;
    let (index, table, _, _) = resolve_table(
        source,
        &TableTarget {
            handle: Some(handle.to_owned()),
            header_cells: Vec::new(),
            occurrence: None,
        },
    )?;
    ensure_simple_layout_table(source, table)?;
    Ok((index, table))
}

pub(in crate::mutation) fn ensure_simple_layout_table(
    source: &SourceDocument,
    table: NodeId,
) -> Result<(), OperationResult> {
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    let rows = document
        .blocks()
        .find_map(|block| match block {
            crate::BodyBlock::Table(value) if value.source_id() == table => {
                Some(value.rows().collect::<Vec<_>>())
            }
            _ => None,
        })
        .ok_or_else(|| unsupported("table must be a direct body table"))?;
    if let Some(reason) = table_mutation_reason(source, table) {
        return Err(
            unsupported("table layout requires a simple rectangular body table")
                .with_reason_code(reason.as_str()),
        );
    }
    let ordinary = |id, allowed: &[&str]| {
        source.children(id).all(|child| {
            matches!(source.node(child).unwrap().kind(), SourceNodeKind::Text)
                || allowed.iter().any(|name| word(source, child, name))
        })
    };
    if !ordinary(table, &["tblPr", "tblGrid", "tr"])
        || rows
            .iter()
            .any(|row| !ordinary(row.source_id(), &["trPr", "tc"]))
        || source.node_ids().any(|id| {
            is_descendant(source, id, table)
                && [
                    "hMerge",
                    "gridBefore",
                    "gridAfter",
                    "wBefore",
                    "wAfter",
                    "tblpPr",
                ]
                .iter()
                .any(|name| word(source, id, name))
        })
    {
        return Err(unsupported(
            "table layout cannot use merged, omitted, wrapped, or floating table structures",
        ));
    }
    // Inspect the full selected table so ranges and property history cannot be bypassed.
    protected_range::ensure_safe_source_ranges(
        source,
        &[source.node(table).unwrap().span()],
        "UNSUPPORTED_TABLE_LAYOUT",
        false,
    )?;
    Ok(())
}

/// Inserts only at a direct table boundary, or updates our own exact separator.
pub fn set_table_spacing_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetTableSpacing,
) -> Result<Vec<u8>, OperationResult> {
    let twips = operation.spacing_points * 20.0;
    if !twips.is_finite()
        || !(1.0..=31_680.0).contains(&twips)
        || (twips - twips.round()).abs() > 0.000_001
    {
        return Err(invalid(
            "spacingPoints must be 0.05 through 1584, in 0.05-point steps",
        ));
    }
    let (_, before) = resolve_layout_table(package, main, source, &operation.before)?;
    let (_, after) = resolve_layout_table(package, main, source, &operation.after)?;
    let body = source.node(before).unwrap().parent().unwrap();
    let elements = source
        .children(body)
        .filter(|id| {
            matches!(
                source.node(*id).unwrap().kind(),
                SourceNodeKind::Element { .. }
            )
        })
        .collect::<Vec<_>>();
    let start = elements.iter().position(|id| *id == before).unwrap();
    let end = elements.iter().position(|id| *id == after).unwrap();
    let prefix = word_prefix_for(source, body, "body")?;
    require_prefix(&prefix)?;
    let fragment = separator_xml(&prefix, twips.round() as u32);
    let span = if end == start + 1 {
        let at = source.node(before).unwrap().span().end;
        SourceSpan { start: at, end: at }
    } else if end == start + 2 && is_our_separator(source, elements[start + 1], &prefix) {
        source.node(elements[start + 1]).unwrap().span()
    } else {
        return Err(invalid(
            "tables must be directly adjacent, or separated only by an engine-authored table separator",
        ));
    };
    protected_range::ensure_safe_source_ranges(
        source,
        &[span],
        "UNSAFE_INSERTION_BOUNDARY",
        false,
    )?;
    verified_layout_output(
        package,
        main,
        source,
        vec![Patch {
            span,
            replacement: fragment.into_bytes(),
        }],
    )
}

fn separator_xml(prefix: &str, twips: u32) -> String {
    let n = |local: &str| qualify(prefix, local);
    let a = attr_prefix(prefix);
    format!(
        "<{p}><{pr}><{keep_next} {a}val=\"0\"/><{keep_lines} {a}val=\"0\"/><{break_before} {a}val=\"0\"/><{widow} {a}val=\"0\"/><{num}><{num_id} {a}val=\"0\"/></{num}><{grid} {a}val=\"0\"/><{spacing} {a}before=\"0\" {a}after=\"0\" {a}beforeLines=\"0\" {a}afterLines=\"0\" {a}beforeAutospacing=\"0\" {a}afterAutospacing=\"0\" {a}line=\"{twips}\" {a}lineRule=\"exact\"/><{context} {a}val=\"0\"/></{pr}></{p}>",
        p = n("p"),
        pr = n("pPr"),
        keep_next = n("keepNext"),
        keep_lines = n("keepLines"),
        break_before = n("pageBreakBefore"),
        widow = n("widowControl"),
        num = n("numPr"),
        num_id = n("numId"),
        grid = n("snapToGrid"),
        spacing = n("spacing"),
        context = n("contextualSpacing")
    )
}

fn is_our_separator(source: &SourceDocument, id: NodeId, prefix: &str) -> bool {
    if !word(source, id, "p") {
        return false;
    }
    let line = source
        .children(id)
        .find(|id| word(source, *id, "pPr"))
        .and_then(|id| source.children(id).find(|id| word(source, *id, "spacing")))
        .and_then(|id| source.node(id)?.attribute("line")?.parse::<u32>().ok());
    line.is_some_and(|line| {
        source_bytes(source, id).ok().as_deref() == Some(separator_xml(prefix, line).as_bytes())
    })
}

/// Changes only requested row flags; the caller chooses the number of header rows.
pub fn set_table_row_layout_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetTableRowLayout,
) -> Result<Vec<u8>, OperationResult> {
    if operation.row.is_some() != operation.prevent_split.is_some()
        || (operation.repeating_header_rows.is_none() && operation.prevent_split.is_none())
    {
        return Err(invalid(
            "provide repeatingHeaderRows, or row and preventSplit together",
        ));
    }
    let (index, table) = resolve_layout_table(package, main, source, &operation.table)?;
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    let table_model = document
        .blocks()
        .find_map(|block| match block {
            crate::BodyBlock::Table(value) if value.source_id() == table => Some(value),
            _ => None,
        })
        .ok_or_else(|| invalid("table source is unavailable"))?;
    let rows = table_model.rows().collect::<Vec<_>>();
    if operation
        .repeating_header_rows
        .is_some_and(|count| count as usize > rows.len())
    {
        return Err(invalid(
            "repeatingHeaderRows must select consecutive leading rows within the table",
        ));
    }
    let selected = operation
        .row
        .as_ref()
        .map(|target| {
            if let Some(handle) = &target.handle {
                let (owner, row) = parse_row_handle(handle)?;
                if owner != index || row >= rows.len() {
                    return Err(invalid("row handle does not belong to the selected table"));
                }
                if !target.first_cell_text.is_empty()
                    && rows[row]
                        .cells()
                        .next()
                        .and_then(|cell| cell.text_for_view(RevisionView::Current).ok())
                        .as_deref()
                        != Some(target.first_cell_text.as_str())
                {
                    return Err(invalid(
                        "row handle first cell does not match expected text",
                    ));
                }
                Ok(row)
            } else {
                cell::resolve_semantic_row_index(
                    &rows,
                    &TableCellRow::Label {
                        text: target.first_cell_text.clone(),
                        occurrence: target.occurrence,
                    },
                )
            }
        })
        .transpose()?;
    let mut patches = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let id = row.source_id();
        let properties = sections::section_child(source, id, "trPr")?;
        let header = properties
            .map(|p| sections::section_child(source, p, "tblHeader"))
            .transpose()?
            .flatten();
        let split = properties
            .map(|p| sections::section_child(source, p, "cantSplit"))
            .transpose()?
            .flatten();
        let prefix = word_element_prefix(
            source,
            properties.unwrap_or(id),
            if properties.is_some() { "trPr" } else { "tr" },
        )?;
        let mut additions = String::new();
        if let Some(count) = operation.repeating_header_rows {
            row_flag_patch(
                source,
                prefix,
                header,
                "tblHeader",
                row_index < count as usize,
                &mut patches,
                &mut additions,
            )?;
        }
        if selected == Some(row_index) {
            row_flag_patch(
                source,
                prefix,
                split,
                "cantSplit",
                operation.prevent_split.unwrap(),
                &mut patches,
                &mut additions,
            )?;
        }
        if !additions.is_empty() {
            if let Some(properties) = properties {
                patches.push(element_insertion(
                    source,
                    properties,
                    additions.into_bytes(),
                )?);
            } else {
                let prefix = word_element_prefix(source, id, "tr")?;
                let at = match source.node(id).unwrap().kind() {
                    SourceNodeKind::Element { start_tag, .. } => start_tag.end,
                    _ => unreachable!(),
                };
                patches.push(Patch {
                    span: SourceSpan { start: at, end: at },
                    replacement: format!("<{pr}>{additions}</{pr}>", pr = qualify(prefix, "trPr"))
                        .into_bytes(),
                });
            }
        }
    }
    verified_layout_output(package, main, source, patches)
}

fn row_flag_patch(
    source: &SourceDocument,
    new_property_prefix: &str,
    existing: Option<NodeId>,
    local: &str,
    enabled: bool,
    patches: &mut Vec<Patch>,
    additions: &mut String,
) -> Result<(), OperationResult> {
    if let Some(existing) = existing {
        let node = source.node(existing).unwrap();
        let value = match node.attribute("val") {
            None | Some("true" | "1" | "on") => true,
            Some("false" | "0" | "off") => false,
            _ => {
                return Err(unsupported(
                    "row layout property has an invalid boolean value",
                ));
            }
        };
        if value == enabled {
            return Ok(());
        }
        // Unknown attributes/content on a changed flag must not silently disappear.
        let prefix = word_element_prefix(source, existing, local)?;
        let bytes = source_bytes(source, existing)?;
        let mut reader = quick_xml::Reader::from_reader(bytes.as_slice());
        let event = reader.read_event().map_err(document_invalid)?;
        let tag = match event {
            quick_xml::events::Event::Start(tag) | quick_xml::events::Event::Empty(tag) => tag,
            _ => return Err(unsupported("row layout property has no source tag")),
        };
        let value_name = format!("{prefix}:val");
        if source.children(existing).next().is_some()
            || tag.attributes().any(|attribute| {
                attribute.map_or(true, |attribute| {
                    attribute.key.as_ref() != value_name.as_bytes()
                })
            })
        {
            return Err(unsupported(
                "row layout property contains unsupported attributes or content",
            ));
        }
    }
    // Use the property's own namespace scope when replacing an imported flag.
    let prefix = if let Some(id) = existing {
        word_element_prefix(source, id, local)?
    } else {
        new_property_prefix
    };
    require_prefix(prefix)?;
    let xml = format!(
        "<{} {}val=\"{}\"/>",
        qualify(prefix, local),
        attr_prefix(prefix),
        u8::from(enabled)
    );
    if let Some(id) = existing {
        patches.push(Patch {
            span: source.node(id).unwrap().span(),
            replacement: xml.into_bytes(),
        });
    } else {
        additions.push_str(&xml);
    }
    Ok(())
}

fn require_prefix(prefix: &str) -> Result<(), OperationResult> {
    if prefix.is_empty() {
        return Err(unsupported(
            "table layout authoring requires explicitly prefixed Word elements",
        ));
    }
    Ok(())
}

fn verified_layout_output(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    patches: Vec<Patch>,
) -> Result<Vec<u8>, OperationResult> {
    let expected_rows = all_table_rows(source)?;
    let expected_xml = apply_patches(source, patches)?;
    let output = package
        .write_replaced_part_to_vec(main, &expected_xml)
        .map_err(document_invalid)?;
    verify_table_row_output_bytes(&output, &expected_rows)?;
    let reopened = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    let (_, reopened_source) = crate::open_main_source(&reopened).map_err(document_invalid)?;
    if reopened_source.original_bytes() != expected_xml {
        return Err(document_invalid(
            "table layout output differs from the requested source patches",
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
