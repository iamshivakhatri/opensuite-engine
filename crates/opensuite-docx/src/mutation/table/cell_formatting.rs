use super::*;
use opensuite_protocol::{CellVerticalAlignment, ParagraphAlignment, TableCellFormattingUpdate};

/// Formats several cells using targets from the same input table.
pub fn set_table_cells_formatting_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetTableCellsFormatting,
) -> Result<Vec<u8>, OperationResult> {
    if operation.updates.is_empty() || operation.updates.len() > MAX_TABLE_CELL_UPDATES {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "table cell formatting updates must contain between 1 and 100 updates",
        ));
    }
    let (table_index, table, rows, headers) = resolve_table(source, &operation.table)?;
    if let Some(reason) = table_mutation_reason(source, table) {
        return Err(
            unsupported("table cell formatting requires a simple rectangular table")
                .with_reason_code(reason.as_str()),
        );
    }
    let has_alignment = operation
        .updates
        .iter()
        .any(|u| u.alignment.is_some() || u.vertical_alignment.is_some());
    if has_alignment {
        ensure_document_unprotected(package, main)?;
        super::layout::ensure_simple_layout_table(source, table)?;
    }
    let mut cells = HashSet::new();
    let mut patches = Vec::new();
    for (index, update) in operation.updates.iter().enumerate() {
        let update_error = |error| cell_update_failure(error, index, &update.target);
        let align_cell = update.alignment.is_some() || update.vertical_alignment.is_some();
        if update.alignment.is_none()
            && update.vertical_alignment.is_none()
            && update.fill.is_none()
            && update
                .text_formatting
                .as_ref()
                .is_none_or(|formatting| *formatting == Default::default())
        {
            return Err(update_error(OperationResult::failed(
                "PRECONDITION_FAILED",
                "each cell update needs fill, text formatting, or alignment",
            )));
        }
        super::shading::validate_shading_fill(update.fill.as_deref()).map_err(update_error)?;
        let target = super::cell::resolve_cell_in_table(
            source,
            table_index,
            &rows,
            &headers,
            &update.target,
            align_cell,
        )
        .map_err(update_error)?;
        if !cells.insert(target.cell) {
            return Err(update_error(OperationResult::failed(
                "PRECONDITION_FAILED",
                "the same table cell was requested more than once",
            )));
        }
        let paragraphs = direct_cell_paragraphs(source, target.cell);
        if align_cell
            && (paragraphs.is_empty()
                || paragraphs.iter().any(|p| !safe_table_paragraph(source, *p))
                || source.children(target.cell).any(|id| {
                    !word(source, id, "tcPr")
                        && !word(source, id, "p")
                        && !matches!(source.node(id).unwrap().kind(), SourceNodeKind::Text)
                }))
        {
            return Err(update_error(unsupported(
                "cell alignment requires ordinary direct paragraphs",
            )));
        }
        patches.extend(cell_property_patches(source, target.cell, update).map_err(update_error)?);
        for paragraph in paragraphs {
            if let Some(alignment) = &update.alignment {
                let pprs = source
                    .children(paragraph)
                    .filter(|id| word(source, *id, "pPr"))
                    .collect::<Vec<_>>();
                if pprs.len() > 1 {
                    return Err(update_error(unsupported(
                        "paragraph has duplicate properties",
                    )));
                }
                if let Some(ppr) = pprs.first() {
                    ensure_simple_property(source, *ppr, "jc").map_err(update_error)?;
                }
                let alignment = match alignment {
                    PropertyPatch::Clear => PropertyPatch::Clear,
                    PropertyPatch::Set(value) => PropertyPatch::Set(match value {
                        TableAlignment::Left => ParagraphAlignment::Left,
                        TableAlignment::Center => ParagraphAlignment::Center,
                        TableAlignment::Right => ParagraphAlignment::Right,
                    }),
                };
                patches.extend(
                    formatting_patches(
                        source,
                        paragraph,
                        &ParagraphFormattingPatch {
                            alignment: Some(alignment),
                            ..Default::default()
                        },
                    )
                    .map_err(update_error)?,
                );
            }
            if let Some(formatting) = &update.text_formatting {
                patches.extend(
                    table_cell_text_formatting_patches(source, paragraph, formatting)
                        .map_err(update_error)?,
                );
            }
        }
    }
    let expected = all_table_rows(source)?;
    let output = write_patches_to_vec(package, main, source, patches)?;
    verify_table_row_output_bytes(&output, &expected)?;
    if has_alignment {
        verify_cell_alignments(&output, operation)?;
    }
    Ok(output)
}

fn cell_property_patches(
    source: &SourceDocument,
    cell: NodeId,
    update: &TableCellFormattingUpdate,
) -> Result<Vec<Patch>, OperationResult> {
    if update.vertical_alignment.is_none() && update.alignment.is_none() {
        return update
            .fill
            .as_deref()
            .map(|fill| super::shading::shading_patch(source, cell, Some(fill)))
            .transpose()
            .map(|patch| patch.into_iter().collect());
    }
    let prefix = word_prefix_for(source, cell, "tc")?;
    let n = |local: &str| qualify(&prefix, local);
    let a = attr_prefix(&prefix);
    let mut values = Vec::new();
    if let Some(fill) = &update.fill {
        values.push((
            "shd",
            format!(
                "<{} {a}val=\"clear\" {a}color=\"auto\" {a}fill=\"{}\"/>",
                n("shd"),
                fill.to_ascii_uppercase()
            ),
        ));
    }
    if let Some(value) = &update.vertical_alignment {
        values.push((
            "vAlign",
            match value {
                PropertyPatch::Clear => String::new(),
                PropertyPatch::Set(value) => format!(
                    "<{} {a}val=\"{}\"/>",
                    n("vAlign"),
                    match value {
                        CellVerticalAlignment::Top => "top",
                        CellVerticalAlignment::Center => "center",
                        CellVerticalAlignment::Bottom => "bottom",
                    }
                ),
            },
        ));
    }
    let properties = source
        .children(cell)
        .filter(|id| word(source, *id, "tcPr"))
        .collect::<Vec<_>>();
    if properties.len() > 1 {
        return Err(unsupported("cell has duplicate properties"));
    }
    let Some(properties) = properties.first().copied() else {
        let children = values
            .into_iter()
            .map(|(_, value)| value)
            .collect::<String>();
        if children.is_empty() {
            return Ok(Vec::new());
        }
        let at = source
            .children(cell)
            .next()
            .ok_or_else(|| unsupported("cell has no paragraph"))?;
        let at = source.node(at).unwrap().span().start;
        return Ok(vec![Patch {
            span: SourceSpan { start: at, end: at },
            replacement: format!("<{}>{children}</{}>", n("tcPr"), n("tcPr")).into_bytes(),
        }]);
    };
    if update.vertical_alignment.is_some() {
        ensure_simple_property(source, properties, "vAlign")?;
    }
    let mut patches = Vec::new();
    let mut empty_children = String::new();
    let empty = matches!(
        source.node(properties).unwrap().kind(),
        SourceNodeKind::Element { end_tag: None, .. }
    );
    for (local, value) in values {
        let existing = source
            .children(properties)
            .filter(|id| word(source, *id, local))
            .collect::<Vec<_>>();
        if existing.len() > 1 {
            return Err(unsupported("cell has duplicate formatting properties"));
        }
        if let Some(id) = existing.first() {
            patches.push(Patch {
                span: source.node(*id).unwrap().span(),
                replacement: value.into_bytes(),
            });
        } else if !value.is_empty() {
            if empty {
                empty_children.push_str(&value);
                continue;
            }
            let following: &[&str] = if local == "shd" {
                &[
                    "noWrap",
                    "tcMar",
                    "textDirection",
                    "tcFitText",
                    "vAlign",
                    "hideMark",
                    "headers",
                    "cellIns",
                    "cellDel",
                    "cellMerge",
                    "tcPrChange",
                ]
            } else {
                &[
                    "hideMark",
                    "headers",
                    "cellIns",
                    "cellDel",
                    "cellMerge",
                    "tcPrChange",
                ]
            };
            if let Some(next) = source
                .children(properties)
                .find(|id| following.iter().any(|name| word(source, *id, name)))
            {
                let at = source.node(next).unwrap().span().start;
                patches.push(Patch {
                    span: SourceSpan { start: at, end: at },
                    replacement: value.into_bytes(),
                });
            } else {
                patches.push(common::element_insertion(
                    source,
                    properties,
                    value.into_bytes(),
                )?);
            }
        }
    }
    if !empty_children.is_empty() {
        patches.push(common::element_insertion(
            source,
            properties,
            empty_children.into_bytes(),
        )?);
    }
    Ok(patches)
}

fn verify_cell_alignments(
    output: &[u8],
    operation: &SetTableCellsFormatting,
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (index, _, rows, headers) = resolve_table(&source, &operation.table)?;
    for update in &operation.updates {
        let target = super::cell::resolve_cell_in_table(
            &source,
            index,
            &rows,
            &headers,
            &update.target,
            true,
        )?;
        let value = |parent, properties, local| {
            source
                .children(parent)
                .find(|id| word(&source, *id, properties))
                .and_then(|id| source.children(id).find(|id| word(&source, *id, local)))
                .and_then(|id| source.node(id).unwrap().attribute("val"))
        };
        let matches = update.vertical_alignment.as_ref().is_none_or(|patch| {
            value(target.cell, "tcPr", "vAlign")
                == match patch {
                    PropertyPatch::Clear => None,
                    PropertyPatch::Set(value) => Some(match value {
                        CellVerticalAlignment::Top => "top",
                        CellVerticalAlignment::Center => "center",
                        CellVerticalAlignment::Bottom => "bottom",
                    }),
                }
        }) && update.alignment.as_ref().is_none_or(|patch| {
            direct_cell_paragraphs(&source, target.cell)
                .iter()
                .all(|p| {
                    value(*p, "pPr", "jc")
                        == match patch {
                            PropertyPatch::Clear => None,
                            PropertyPatch::Set(value) => Some(match value {
                                TableAlignment::Left => "left",
                                TableAlignment::Center => "center",
                                TableAlignment::Right => "right",
                            }),
                        }
                })
        });
        if !matches {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "output cell alignment does not match request",
            ));
        }
    }
    Ok(())
}
