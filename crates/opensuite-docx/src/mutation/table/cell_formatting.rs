use super::*;

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
    let mut cells = HashSet::new();
    let mut patches = Vec::new();
    for update in &operation.updates {
        if update.fill.is_none()
            && update
                .text_formatting
                .as_ref()
                .is_none_or(|formatting| *formatting == Default::default())
        {
            return Err(OperationResult::failed(
                "PRECONDITION_FAILED",
                "each cell update needs fill or text formatting",
            ));
        }
        super::shading::validate_shading_fill(update.fill.as_deref())?;
        let target =
            resolve_table_cell_in_table(source, table_index, &rows, &headers, &update.target)?;
        if !cells.insert(target.cell) {
            return Err(OperationResult::failed(
                "PRECONDITION_FAILED",
                "the same table cell was requested more than once",
            ));
        }
        if let Some(fill) = update.fill.as_deref() {
            patches.push(super::shading::shading_patch(
                source,
                target.cell,
                Some(fill),
            )?);
        }
        if let Some(formatting) = &update.text_formatting {
            patches.extend(table_cell_text_formatting_patches(
                source,
                target.paragraph,
                formatting,
            )?);
        }
    }
    let expected = all_table_rows(source)?;
    let output = write_patches_to_vec(package, main, source, patches)?;
    verify_table_row_output_bytes(&output, &expected)?;
    Ok(output)
}
