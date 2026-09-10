use super::*;

/// Inserts one complete row after a semantic row anchor in a simple main-body table.
pub fn insert_table_row_after_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertTableRowAfter,
) -> Result<Vec<u8>, OperationResult> {
    insert_table_rows_after(
        source,
        package,
        main,
        &operation.table,
        &operation.after,
        std::slice::from_ref(&operation.cells),
    )
}

/// Inserts several complete rows with one contiguous source insertion.
pub fn insert_table_rows_after_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertTableRowsAfter,
) -> Result<Vec<u8>, OperationResult> {
    insert_table_rows_after(
        source,
        package,
        main,
        &operation.table,
        &operation.after,
        &operation.rows,
    )
}

fn insert_table_rows_after(
    source: &SourceDocument,
    package: &Package,
    main: &Part,
    table: &TableTarget,
    after: &TableRowTarget,
    rows: &[Vec<String>],
) -> Result<Vec<u8>, OperationResult> {
    if rows.is_empty() || rows.len() > MAX_TABLE_ROWS_PER_OPERATION {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "inserted rows must contain between 1 and 100 rows",
        ));
    }
    let target = resolve_table_row(source, table, after, rows)?;
    let mut fragment = Vec::new();
    for cells in rows {
        fragment.extend(match target.template {
            Some(template) => table_row_fragment(source, template, cells)?,
            None => minimal_table_row_fragment(source, target.row, cells)?,
        });
    }
    let insertion = source.node(target.row).expect("row exists").span().end;
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
    let mut expected = all_table_rows(source)?;
    expected[target.table_index].splice(
        target.row_index + 1..target.row_index + 1,
        rows.iter().cloned(),
    );
    let output = package
        .write_replaced_part_to_vec(main, &patched)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    verify_table_row_output_bytes(&output, &expected)?;
    Ok(output)
}

pub fn delete_table_row_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &DeleteTableRow,
) -> Result<Vec<u8>, OperationResult> {
    let (table_index, table, rows, _) = resolve_table(source, &operation.table)?;
    if let Some(reason) = table_mutation_reason(source, table) {
        return Err(
            unsupported("delete_table_row supports only simple rectangular tables")
                .with_reason_code(reason.as_str()),
        );
    }
    if rows.len() == 1 {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "deleting the final table row requires delete_table",
        )
        .with_reason_code("LAST_TABLE_ROW"));
    }
    let (handle_table, row_index) = operation
        .row
        .handle
        .as_deref()
        .map(parse_row_handle)
        .transpose()?
        .ok_or_else(|| {
            OperationResult::failed(
                "PRECONDITION_FAILED",
                "delete_table_row requires a row handle",
            )
        })?;
    if handle_table != table_index || row_index >= rows.len() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "table row handle was not found",
        ));
    }
    let mut expected = all_table_rows(source)?;
    expected[table_index].remove(row_index);
    write_patches_to_vec(
        package,
        main,
        source,
        vec![Patch {
            span: source
                .node(rows[row_index].source_id())
                .expect("row exists")
                .span(),
            replacement: Vec::new(),
        }],
    )
    .and_then(|output| {
        verify_table_row_output_bytes(&output, &expected)?;
        Ok(output)
    })
}

struct ResolvedTableRow {
    row: NodeId,
    template: Option<NodeId>,
    table_index: usize,
    row_index: usize,
}

fn resolve_table_row(
    source: &SourceDocument,
    table_target: &TableTarget,
    after: &TableRowTarget,
    inserted_rows: &[Vec<String>],
) -> Result<ResolvedTableRow, OperationResult> {
    let (table_index, table, rows, _headers) = resolve_table(source, table_target)?;
    if let Some(reason) = table_mutation_reason(source, table) {
        return Err(unsupported(
            "insert_table_row supports only simple rectangular tables without merges, nesting, or revisions",
        )
        .with_reason_code(reason.as_str()));
    }
    let width = rows[0].cells().count();
    if inserted_rows.iter().any(|cells| cells.len() != width) {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "inserted row cell count must match the table column count",
        ));
    }
    if let Some(handle) = &after.handle {
        let (handle_table, row_index) = parse_row_handle(handle)?;
        if handle_table != table_index {
            return Err(OperationResult::failed(
                "PRECONDITION_FAILED",
                "row handle does not belong to the selected table",
            ));
        }
        let row = rows.get(row_index).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "table row handle was not found")
        })?;
        let template = rows
            .iter()
            .filter(|candidate| candidate.source_id() != row.source_id())
            .find(|candidate| safe_table_row(source, candidate.source_id()))
            .map(crate::Row::source_id);
        return Ok(ResolvedTableRow {
            row: row.source_id(),
            template,
            table_index,
            row_index,
        });
    }
    let mut matches = Vec::new();
    for (row_index, row) in rows.iter().enumerate().skip(1) {
        let cells = row.cells().collect::<Vec<_>>();
        if cells.first().is_some_and(|cell| {
            cell.text_for_view(RevisionView::Current)
                .ok()
                .as_deref()
                .is_some_and(|text| label_matches(text, &after.first_cell_text))
        }) {
            matches.push((row_index, row.source_id()));
        }
    }
    let (row_index, row) = select_row(matches, after)?;
    let template = rows
        .iter()
        .filter(|candidate| candidate.source_id() != row)
        .find(|candidate| safe_table_row(source, candidate.source_id()))
        .map(crate::Row::source_id);
    Ok(ResolvedTableRow {
        row,
        template,
        table_index,
        row_index,
    })
}

fn select_row(
    mut rows: Vec<(usize, NodeId)>,
    target: &TableRowTarget,
) -> Result<(usize, NodeId), OperationResult> {
    if rows.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "table row was not found",
        ));
    }
    if let Some(occurrence) = target.occurrence {
        return rows.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed(
                "TARGET_NOT_FOUND",
                "table row target occurrence was not found",
            )
        });
    }
    if rows.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "table row first-cell text matches more than one row",
        ));
    }
    Ok(rows.pop().expect("one row"))
}

fn safe_table_row(source: &SourceDocument, row: NodeId) -> bool {
    source
        .children(row)
        .all(|child| word(source, child, "trPr") || word(source, child, "tc"))
        && source
            .children(row)
            .filter(|child| word(source, *child, "tc"))
            .all(|cell| safe_template_cell(source, cell))
        && !source.node_ids().any(|id| {
            is_descendant(source, id, row)
                && (word(source, id, "fldChar")
                    || word(source, id, "fldSimple")
                    || word(source, id, "instrText")
                    || word(source, id, "sdt")
                    || word(source, id, "hyperlink")
                    || word(source, id, "drawing")
                    || word(source, id, "pict")
                    || word(source, id, "bookmarkStart")
                    || word(source, id, "bookmarkEnd")
                    || word(source, id, "commentRangeStart")
                    || word(source, id, "commentRangeEnd")
                    || word(source, id, "commentReference")
                    || word(source, id, "rPrChange"))
        })
}

fn table_row_fragment(
    source: &SourceDocument,
    row: NodeId,
    cells: &[String],
) -> Result<Vec<u8>, OperationResult> {
    let prefix = word_element_prefix(source, row, "tr")?;
    let name = |local: &str| qualify(prefix, local);
    let row_properties = source
        .children(row)
        .find(|child| word(source, *child, "trPr"))
        .filter(|properties| safe_row_properties(source, *properties))
        .map(|properties| source_bytes(source, properties))
        .transpose()?
        .unwrap_or_default();
    let templates = source
        .children(row)
        .filter(|child| word(source, *child, "tc"))
        .collect::<Vec<_>>();
    let mut fragment = format!(
        "<{}>{}",
        name("tr"),
        String::from_utf8_lossy(&row_properties)
    );
    for (template, value) in templates.into_iter().zip(cells) {
        fragment.push_str(&cell_fragment(source, template, value, prefix)?);
    }
    fragment.push_str(&format!("</{}>", name("tr")));
    Ok(fragment.into_bytes())
}

fn minimal_table_row_fragment(
    source: &SourceDocument,
    position_row: NodeId,
    cells: &[String],
) -> Result<Vec<u8>, OperationResult> {
    let prefix = word_element_prefix(source, position_row, "tr")?;
    let name = |local: &str| qualify(prefix, local);
    let mut fragment = format!("<{}>", name("tr"));
    for value in cells {
        let space = if requires_space_preservation(value) {
            " xml:space=\"preserve\""
        } else {
            ""
        };
        fragment.push_str(&format!(
            "<{}><{}><{}><{}{}>{}</{}></{}></{}></{}>",
            name("tc"),
            name("p"),
            name("r"),
            name("t"),
            space,
            escape(value),
            name("t"),
            name("r"),
            name("p"),
            name("tc")
        ));
    }
    fragment.push_str(&format!("</{}>", name("tr")));
    Ok(fragment.into_bytes())
}

fn safe_row_properties(source: &SourceDocument, properties: NodeId) -> bool {
    source.children(properties).all(|child| {
        word(source, child, "trHeight")
            || word(source, child, "cantSplit")
            || word(source, child, "jc")
            || word(source, child, "tblCellSpacing")
            || word(source, child, "cnfStyle")
    })
}
