use super::*;

/// Creates a minimal, rectangular, immediately editable table.
pub fn create_table_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &CreateTable,
) -> Result<Vec<u8>, OperationResult> {
    let width = operation.rows.first().map_or(0, Vec::len);
    if operation.rows.is_empty()
        || width == 0
        || operation.rows.iter().any(|row| row.len() != width)
    {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "create_table requires a non-empty rectangular cell matrix",
        )
        .with_reason_code("INVALID_TABLE_DIMENSIONS"));
    }
    let placement = resolve_paragraph_placement(source, &operation.placement)?;
    let (body, blocks) = direct_body_blocks(source)?;
    let index = placement_index(&blocks, placement)?;
    let insertion = body_insertion(source, body, &blocks, index)?;
    let patched = apply_patches(
        source,
        vec![Patch {
            span: SourceSpan {
                start: insertion,
                end: insertion,
            },
            replacement: table_fragment_for_body(source, body, &operation.rows)?,
        }],
    )?;
    let output = package
        .write_replaced_part_to_vec(main, &patched)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    verify_table_row_output_bytes(
        &output,
        &expected_table_insert(source, &blocks, index, &operation.rows)?,
    )?;
    Ok(output)
}

pub fn delete_table_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &DeleteTable,
) -> Result<Vec<u8>, OperationResult> {
    let (table_index, table, _, _) = resolve_table(source, &operation.table)?;
    if let Some(reason) = table_mutation_reason(source, table) {
        return Err(
            unsupported("delete_table supports only simple direct body tables")
                .with_reason_code(reason.as_str()),
        );
    }
    let mut expected = all_table_rows(source)?;
    expected.remove(table_index);
    write_patches_to_vec(
        package,
        main,
        source,
        vec![Patch {
            span: source.node(table).expect("table exists").span(),
            replacement: Vec::new(),
        }],
    )
    .and_then(|output| {
        verify_table_row_output_bytes(&output, &expected)?;
        Ok(output)
    })
}
