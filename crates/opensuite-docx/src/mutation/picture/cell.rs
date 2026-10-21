//! Source-backed cell placement and conservative declared cell bounds.
use super::*;
use opensuite_protocol::CellPicturePosition;

pub(super) struct CellInsertion {
    pub cell: NodeId,
    pub insertion: usize,
    table: NodeId,
    row: NodeId,
    column: usize,
    columns: usize,
}

pub(super) fn resolve_insertion(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    handle: &str,
    position: CellPicturePosition,
) -> Result<CellInsertion, OperationResult> {
    let (cell_handle, stamp) = handle.rsplit_once(':').ok_or_else(stale_target)?;
    if stamp
        != format!(
            "{:016x}",
            sections::section_fingerprint(package, main, source)?
        )
    {
        return Err(stale_target());
    }
    let (table_index, row_index, column) = table::parse_cell_handle(cell_handle)?;
    let (_, table, rows, _) = table::resolve_table(
        source,
        &TableTarget {
            handle: Some(format!("t{table_index}")),
            header_cells: Vec::new(),
            occurrence: None,
        },
    )?;
    if let Some(reason) = table::table_mutation_reason(source, table) {
        return Err(
            unsupported("cell picture insertion requires a simple rectangular body table")
                .with_reason_code(reason.as_str()),
        );
    }
    // Older horizontal merges and omitted grid cells are ambiguous even when cell counts match.
    if source.node_ids().any(|id| {
        table::is_descendant(source, id, table)
            && ["hMerge", "gridBefore", "gridAfter", "wBefore", "wAfter"]
                .iter()
                .any(|name| word(source, id, name))
    }) || !ordinary_children(source, table, &["tblPr", "tblGrid", "tr"])
        || rows
            .iter()
            .any(|row| !ordinary_children(source, row.source_id(), &["trPr", "tc"]))
    {
        return Err(unsupported(
            "cell picture insertion cannot use merged, omitted, or wrapped table cells",
        ));
    }
    let row = rows.get(row_index).ok_or_else(|| {
        OperationResult::failed("TARGET_NOT_FOUND", "picture cell row was not found")
    })?;
    let cell = row
        .cells()
        .nth(column)
        .ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "picture cell column was not found")
        })?
        .source_id();
    let paragraphs = table::direct_cell_paragraphs(source, cell);
    if paragraphs.is_empty() || !ordinary_children(source, cell, &["tcPr", "p"]) {
        return Err(unsupported(
            "picture cell must contain ordinary direct paragraphs",
        ));
    }
    sections::section_child(source, cell, "tcPr")?;
    let insertion = match position {
        CellPicturePosition::Start => source.node(paragraphs[0]).expect("paragraph").span().start,
        CellPicturePosition::End => {
            source
                .node(*paragraphs.last().expect("paragraph"))
                .expect("paragraph")
                .span()
                .end
        }
    };
    protected_range::ensure_safe_source_ranges(
        source,
        &[SourceSpan {
            start: insertion,
            end: insertion,
        }],
        "UNSAFE_INSERTION_BOUNDARY",
        false,
    )?;
    // Property history cannot be checked by a point inside the cell content.
    if source.node_ids().any(|id| {
        table::is_descendant(source, id, table)
            && source.node(id).is_some_and(|node| match node.kind() {
                SourceNodeKind::Element { name, .. } => {
                    name.local_name().ends_with("PrChange")
                        || ["cellIns", "cellDel", "cellMerge"]
                            .iter()
                            .any(|name| word(source, id, name))
                }
                _ => false,
            })
    }) {
        return Err(unsupported(
            "cell picture insertion cannot use tracked table property changes",
        ));
    }
    Ok(CellInsertion {
        cell,
        insertion,
        table,
        row: row.source_id(),
        column,
        columns: rows[0].cells().count(),
    })
}

fn stale_target() -> OperationResult {
    OperationResult::failed(
        "PRECONDITION_FAILED",
        "picture cell handle is stale or malformed; inspect tables again",
    )
}

fn ordinary_children(source: &SourceDocument, parent: NodeId, names: &[&str]) -> bool {
    source.children(parent).all(|id| {
        names.iter().any(|name| word(source, id, name))
            || source.node(id).is_some_and(|node| {
                matches!(node.kind(), SourceNodeKind::Text)
                    && source.original_bytes()[node.span().start..node.span().end]
                        .iter()
                        .all(u8::is_ascii_whitespace)
            })
    })
}

fn geometry_error() -> OperationResult {
    OperationResult::failed(
        "INVALID_CELL_GEOMETRY",
        "automatic cell fit requires positive, unambiguous grid widths, absolute cell/table widths, and cell margins",
    )
}

fn child(
    source: &SourceDocument,
    parent: NodeId,
    name: &str,
) -> Result<Option<NodeId>, OperationResult> {
    sections::section_child(source, parent, name).map_err(|_| geometry_error())
}

fn width(source: &SourceDocument, id: NodeId, positive: bool) -> Result<i64, OperationResult> {
    let node = source.node(id).expect("width property");
    if node.attribute("type") != Some("dxa") {
        return Err(geometry_error());
    }
    number(source, id, "w", positive)
}

fn number(
    source: &SourceDocument,
    id: NodeId,
    name: &str,
    positive: bool,
) -> Result<i64, OperationResult> {
    source
        .node(id)
        .and_then(|node| node.attribute(name))
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| !positive || *v > 0)
        .map(i64::from)
        .ok_or_else(geometry_error)
}

/// Bounds are in twips. Paragraph defaults are deducted by the existing inline fitter.
pub(super) fn fit_bounds(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    cell: &CellInsertion,
    section: (i64, i64),
) -> Result<(i64, i64), OperationResult> {
    let grid = child(source, cell.table, "tblGrid")?.ok_or_else(geometry_error)?;
    let columns = source
        .children(grid)
        .filter(|id| word(source, *id, "gridCol"))
        .collect::<Vec<_>>();
    if columns.len() != cell.columns || !ordinary_children(source, grid, &["gridCol"]) {
        return Err(geometry_error());
    }
    let widths = columns
        .iter()
        .map(|id| number(source, *id, "w", true))
        .collect::<Result<Vec<_>, _>>()?;
    let total: i64 = widths.iter().sum();
    if total > section.0 {
        return Err(geometry_error());
    }
    // Conflicting preferred widths can redistribute a column in Word. Do not guess.
    for row in source
        .children(cell.table)
        .filter(|id| word(source, *id, "tr"))
    {
        for (index, item) in source
            .children(row)
            .filter(|id| word(source, *id, "tc"))
            .enumerate()
        {
            if let Some(properties) = child(source, item, "tcPr")? {
                if let Some(id) = child(source, properties, "tcW")? {
                    if width(source, id, true)? != widths[index] {
                        return Err(OperationResult::failed(
                            "INVALID_CELL_GEOMETRY",
                            "declared cell width conflicts with the table grid; automatic fit cannot determine the cell width",
                        ));
                    }
                }
            }
        }
    }
    let available = widths[cell.column];
    let table_properties = child(source, cell.table, "tblPr")?;
    let cell_properties = child(source, cell.cell, "tcPr")?;
    let mut table_margins = None;
    if let Some(properties) = table_properties {
        for name in ["tblStyle", "tblpPr", "bidiVisual", "tblPrEx"] {
            if child(source, properties, name)?.is_some() {
                return Err(OperationResult::failed(
                    "INVALID_CELL_GEOMETRY",
                    format!("automatic cell fit cannot resolve table property {name}"),
                ));
            }
        }
        if let Some(id) = child(source, properties, "tblW")? {
            let node = source.node(id).expect("table width");
            if !(node.attribute("type") == Some("auto") && node.attribute("w") == Some("0"))
                && width(source, id, true)? != total
            {
                return Err(geometry_error());
            }
        }
        if let Some(id) = child(source, properties, "tblInd")? {
            if total + width(source, id, false)? > section.0 {
                return Err(geometry_error());
            }
        }
        if let Some(id) = child(source, properties, "tblCellSpacing")? {
            if width(source, id, false)? != 0 {
                return Err(geometry_error());
            }
        }
        table_margins = child(source, properties, "tblCellMar")?;
    }
    // Table-style inheritance is not resolved by the paragraph style resolver.
    if let Some(styles) = crate::load_styles(package, main).map_err(document_invalid)? {
        let styles = styles.source();
        if styles.node_ids().any(|id| {
            word(styles, id, "style")
                && styles.node(id).is_some_and(|node| {
                    node.attribute("type") == Some("table")
                        && matches!(node.attribute("default"), Some("1" | "true" | "on"))
                })
        }) {
            return Err(OperationResult::failed(
                "INVALID_CELL_GEOMETRY",
                "automatic cell fit cannot resolve a default table style",
            ));
        }
    }
    let mut cell_margins = None;
    if let Some(properties) = cell_properties {
        for name in ["textDirection", "tcFitText", "noWrap"] {
            if child(source, properties, name)?.is_some() {
                return Err(geometry_error());
            }
        }
        cell_margins = child(source, properties, "tcMar")?;
    }
    let row_properties = child(source, cell.row, "trPr")?;
    if child(source, cell.row, "tblPrEx")?.is_some() {
        return Err(geometry_error());
    }
    let mut height = section.1;
    if let Some(properties) = row_properties {
        if let Some(id) = child(source, properties, "trHeight")? {
            let node = source.node(id).expect("row height");
            match node.attribute("hRule") {
                // Existing cell text also consumes a fixed row height; remaining height needs rendering.
                Some("exact") => {
                    return Err(OperationResult::failed(
                        "INVALID_CELL_GEOMETRY",
                        "automatic cell fit cannot determine remaining height in an exact-height row",
                    ));
                }
                None | Some("auto" | "atLeast") => {}
                _ => return Err(geometry_error()),
            }
        }
        if child(source, properties, "tblCellSpacing")?.is_some() {
            return Err(geometry_error());
        }
    }
    let margin = |name, default| -> Result<i64, OperationResult> {
        // Logical start/end margins require direction resolution; refuse instead of guessing.
        for margins in [cell_margins, table_margins].into_iter().flatten() {
            if child(source, margins, "start")?.is_some()
                || child(source, margins, "end")?.is_some()
            {
                return Err(geometry_error());
            }
        }
        for margins in [cell_margins, table_margins].into_iter().flatten() {
            if let Some(id) = child(source, margins, name)? {
                return width(source, id, false);
            }
        }
        Ok(default)
    };
    // OOXML defaults: 108 twips left/right and zero top/bottom without table styles.
    let available = available - margin("left", 108)? - margin("right", 108)?;
    height -= margin("top", 0)? + margin("bottom", 0)?;
    if available <= 0 || height <= 0 {
        return Err(geometry_error());
    }
    Ok((available, height))
}

#[cfg(test)]
mod tests;
