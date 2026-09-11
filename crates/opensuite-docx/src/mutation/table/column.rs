use super::*;

/// Inserts one grid-aware column into a simple semantic table.
pub fn insert_table_column_after_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertTableColumnAfter,
) -> Result<Vec<u8>, OperationResult> {
    let (table_index, table, rows, headers) = resolve_table(source, &operation.table)?;
    if let Some(reason) = table_mutation_reason(source, table) {
        return Err(unsupported(
            "insert_table_column supports only simple rectangular tables without merges, nesting, or revisions",
        )
        .with_reason_code(reason.as_str()));
    }
    if operation.cells.len() != rows.len() - 1 {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "column cell count must match the table data-row count",
        ));
    }
    let columns = headers
        .iter()
        .enumerate()
        .filter_map(|(index, header)| {
            label_matches(header, &operation.after_column_header).then_some(index)
        })
        .collect::<Vec<_>>();
    let column = if let Some(handle) = &operation.after_column_handle {
        let (handle_table, column) = parse_column_handle(handle)?;
        if handle_table != table_index {
            return Err(OperationResult::failed(
                "PRECONDITION_FAILED",
                "column handle does not belong to the selected table",
            ));
        }
        if column >= headers.len() {
            return Err(OperationResult::failed(
                "TARGET_NOT_FOUND",
                "table column handle was not found",
            ));
        }
        column
    } else if columns.len() != 1 {
        return Err(OperationResult::failed(
            if columns.is_empty() {
                "TARGET_NOT_FOUND"
            } else {
                "TARGET_AMBIGUOUS"
            },
            "column header does not resolve to one current table column",
        ));
    } else {
        columns[0]
    };
    let grid = explicit_table_grid(source, table, headers.len())?;
    let mut patches = vec![Patch {
        span: SourceSpan {
            start: source
                .node(grid.columns[column])
                .expect("grid column exists")
                .span()
                .end,
            end: source
                .node(grid.columns[column])
                .expect("grid column exists")
                .span()
                .end,
        },
        replacement: source_bytes(source, grid.columns[column])?,
    }];
    for (row_index, row) in rows.iter().enumerate() {
        let cells = row.cells().collect::<Vec<_>>();
        let value = if row_index == 0 {
            &operation.header
        } else {
            &operation.cells[row_index - 1]
        };
        let template = cells[column].source_id();
        let end = source.node(template).expect("cell exists").span().end;
        patches.push(Patch {
            span: SourceSpan { start: end, end },
            replacement: if safe_template_cell(source, template) {
                cell_fragment(
                    source,
                    template,
                    value,
                    word_element_prefix(source, row.source_id(), "tr")?,
                )?
                .into_bytes()
            } else {
                minimal_table_cell_fragment(source, row.source_id(), value)?.into_bytes()
            },
        });
    }
    let patched = apply_patches(source, patches)?;
    let mut expected = all_table_rows(source)?;
    for (row_index, row) in expected[table_index].iter_mut().enumerate() {
        row.insert(
            column + 1,
            if row_index == 0 {
                operation.header.clone()
            } else {
                operation.cells[row_index - 1].clone()
            },
        );
    }
    let output = package
        .write_replaced_part_to_vec(main, &patched)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    let mut output_target = operation.table.clone();
    if output_target.handle.is_none() {
        output_target
            .header_cells
            .insert(column + 1, operation.header.clone());
    }
    verify_table_column_output_bytes(&output, &output_target, &expected)?;
    Ok(output)
}

pub fn delete_table_column_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &DeleteTableColumn,
) -> Result<Vec<u8>, OperationResult> {
    let (table_index, table, rows, headers) = resolve_table(source, &operation.table)?;
    if let Some(reason) = table_mutation_reason(source, table) {
        return Err(
            unsupported("delete_table_column supports only simple rectangular tables")
                .with_reason_code(reason.as_str()),
        );
    }
    if headers.len() == 1 {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "deleting the final table column requires delete_table",
        )
        .with_reason_code("LAST_TABLE_COLUMN"));
    }
    let column = if let Some(handle) = &operation.column_handle {
        let (t, c) = parse_column_handle(handle)?;
        if t != table_index {
            return Err(OperationResult::failed(
                "PRECONDITION_FAILED",
                "column handle does not belong to the selected table",
            ));
        }
        c
    } else {
        let found = headers
            .iter()
            .enumerate()
            .filter_map(|(i, text)| label_matches(text, &operation.column_header).then_some(i))
            .collect::<Vec<_>>();
        if found.len() != 1 {
            return Err(OperationResult::failed(
                if found.is_empty() {
                    "TARGET_NOT_FOUND"
                } else {
                    "TARGET_AMBIGUOUS"
                },
                "column header does not resolve to one current table column",
            ));
        }
        found[0]
    };
    if column >= headers.len() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "table column handle was not found",
        ));
    }
    let grid = explicit_table_grid(source, table, headers.len())?;
    let mut patches = vec![Patch {
        span: source
            .node(grid.columns[column])
            .expect("grid exists")
            .span(),
        replacement: Vec::new(),
    }];
    for row in &rows {
        let cell = row
            .cells()
            .nth(column)
            .expect("rectangular table")
            .source_id();
        patches.push(Patch {
            span: source.node(cell).expect("cell exists").span(),
            replacement: Vec::new(),
        });
    }
    let mut expected = all_table_rows(source)?;
    for row in &mut expected[table_index] {
        row.remove(column);
    }
    let output = write_patches_to_vec(package, main, source, patches)?;
    verify_table_row_output_bytes(&output, &expected)?;
    let target = TableTarget {
        header_cells: expected[table_index][0].clone(),
        occurrence: None,
        handle: Some(format!("t{table_index}")),
    };
    let output_package = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    let (_, output_source) = crate::open_main_source(&output_package).map_err(document_invalid)?;
    let (_, output_table, _, output_headers) = resolve_table(&output_source, &target)?;
    explicit_table_grid(&output_source, output_table, output_headers.len())?;
    Ok(output)
}

fn parse_column_handle(handle: &str) -> Result<(usize, usize), OperationResult> {
    let Some((table, column)) = handle.split_once(":c") else {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "column handle is malformed",
        ));
    };
    Ok((
        parse_table_handle(table)?,
        column.parse().map_err(|_| {
            OperationResult::failed("PRECONDITION_FAILED", "column handle is malformed")
        })?,
    ))
}

struct ExplicitTableGrid {
    columns: Vec<NodeId>,
}

fn explicit_table_grid(
    source: &SourceDocument,
    table: NodeId,
    width: usize,
) -> Result<ExplicitTableGrid, OperationResult> {
    let grids = source
        .children(table)
        .filter(|id| word(source, *id, "tblGrid"))
        .collect::<Vec<_>>();
    if grids.len() != 1 {
        return Err(unsupported(
            "insert_table_column requires one explicit table grid",
        ));
    }
    let columns = source
        .children(grids[0])
        .filter(|column| word(source, *column, "gridCol"))
        .collect::<Vec<_>>();
    if columns.len() != width {
        return Err(unsupported(
            "insert_table_column requires one grid column per table column",
        ));
    }
    Ok(ExplicitTableGrid { columns })
}

pub(crate) fn table_grid_reason(
    source: &SourceDocument,
    table: NodeId,
    width: usize,
) -> Option<AffordanceReason> {
    explicit_table_grid(source, table, width)
        .err()
        .map(|_| AffordanceReason::InvalidTableGrid)
}

fn verify_table_column_output_bytes(
    output: &[u8],
    target: &TableTarget,
    expected: &[Vec<Vec<String>>],
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    if all_table_rows(&source)? != expected {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output table cells do not match the requested column insertion",
        ));
    }
    let (_, table, _rows, headers) = resolve_table(&source, target)?;
    explicit_table_grid(&source, table, headers.len())?;
    Ok(())
}
