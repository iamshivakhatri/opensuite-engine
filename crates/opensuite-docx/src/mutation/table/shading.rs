use super::*;

pub fn set_table_cell_shading_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetTableCellShading,
) -> Result<Vec<u8>, OperationResult> {
    if operation.updates.is_empty() || operation.updates.len() > MAX_TABLE_CELL_UPDATES {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "table cell shading updates must contain between 1 and 100 updates",
        ));
    }
    let (table_index, table, rows, headers) = resolve_table(source, &operation.table)?;
    if let Some(reason) = table_mutation_reason(source, table) {
        return Err(
            unsupported("set_table_cell_shading supports only simple rectangular tables")
                .with_reason_code(reason.as_str()),
        );
    }
    let mut cells = HashSet::new();
    let mut targets = Vec::new();
    for update in &operation.updates {
        if update.fill.as_deref().is_some_and(|fill| {
            fill.len() != 6 || !fill.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(OperationResult::failed(
                "INVALID_OPERATION",
                "cell shading fill must be a 6-digit RGB hex value",
            ));
        }
        let target =
            resolve_table_cell_in_table(source, table_index, &rows, &headers, &update.target)?;
        if !cells.insert(target.cell) {
            return Err(OperationResult::failed(
                "PRECONDITION_FAILED",
                "the same table cell was requested more than once",
            ));
        }
        targets.push(target);
    }
    let mut patches = Vec::new();
    for (target, update) in targets.iter().zip(&operation.updates) {
        patches.push(shading_patch(source, target.cell, update.fill.as_deref())?);
    }
    write_patches_to_vec(package, main, source, patches)
}

fn shading_patch(
    source: &SourceDocument,
    cell: NodeId,
    fill: Option<&str>,
) -> Result<Patch, OperationResult> {
    let prefix = word_element_prefix(source, cell, "tc")?;
    let name = |local: &str| qualify(prefix, local);
    let value = fill
        .map(|fill| {
            format!(
                r#"<{} {}val="clear" {}color="auto" {}fill="{}"/>"#,
                name("shd"),
                attr_prefix(prefix),
                attr_prefix(prefix),
                attr_prefix(prefix),
                fill.to_ascii_uppercase()
            )
        })
        .unwrap_or_default();
    if let Some(properties) = source.children(cell).find(|id| word(source, *id, "tcPr")) {
        if let Some(existing) = source
            .children(properties)
            .find(|id| word(source, *id, "shd"))
        {
            return Ok(Patch {
                span: source.node(existing).expect("shading exists").span(),
                replacement: value.into_bytes(),
            });
        }
        let end = source
            .node(properties)
            .expect("properties exist")
            .span()
            .end
            - format!("</{}>", name("tcPr")).len();
        return Ok(Patch {
            span: SourceSpan { start: end, end },
            replacement: value.into_bytes(),
        });
    }
    if value.is_empty() {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "cell has no shading to clear",
        ));
    }
    let at = source
        .children(cell)
        .next()
        .and_then(|id| source.node(id))
        .ok_or_else(|| unsupported("cell has no insertion boundary"))?
        .span()
        .start;
    Ok(Patch {
        span: SourceSpan { start: at, end: at },
        replacement: format!("<{}>{}</{}>", name("tcPr"), value, name("tcPr")).into_bytes(),
    })
}
