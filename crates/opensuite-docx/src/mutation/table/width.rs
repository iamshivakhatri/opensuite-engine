use super::*;

pub fn set_table_column_widths_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetTableColumnWidths,
) -> Result<Vec<u8>, OperationResult> {
    let (_, table, rows, _) = resolve_table(source, &operation.table)?;
    if let Some(reason) = table_mutation_reason(source, table) {
        return Err(
            unsupported("set_table_column_widths supports only simple rectangular tables")
                .with_reason_code(reason.as_str()),
        );
    }
    let columns = rows.first().map(|row| row.cells().count()).unwrap_or(0);
    if operation.widths_twips.len() != columns
        || operation
            .widths_twips
            .iter()
            .any(|width| *width == 0 || *width > 31_680)
    {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "table widths must be positive sane twips, one per column",
        ));
    }
    let prefix = word_element_prefix(source, table, "tbl")?;
    let name = |local: &str| qualify(prefix, local);
    let attr = attr_prefix(prefix);
    let columns = operation
        .widths_twips
        .iter()
        .fold(String::new(), |mut columns, width| {
            use std::fmt::Write;
            write!(columns, r#"<{} {}w="{width}"/>"#, name("gridCol"), attr)
                .expect("write to string");
            columns
        });
    let grid = format!("<{}>{}</{}>", name("tblGrid"), columns, name("tblGrid"));
    let grid_node = source
        .children(table)
        .find(|id| word(source, *id, "tblGrid"))
        .ok_or_else(|| unsupported("table has no grid"))?;
    let mut patches = vec![Patch {
        span: source.node(grid_node).expect("grid exists").span(),
        replacement: grid.into_bytes(),
    }];
    for row in rows {
        for (index, cell) in row.cells().enumerate() {
            patches.push(cell_width_patch(
                source,
                cell.source_id(),
                operation.widths_twips[index],
                prefix,
            )?);
        }
    }
    write_patches_to_vec(package, main, source, patches)
}

fn cell_width_patch(
    source: &SourceDocument,
    cell: NodeId,
    width: u16,
    prefix: &str,
) -> Result<Patch, OperationResult> {
    let name = |local: &str| qualify(prefix, local);
    let value = format!(
        r#"<{} {}w="{width}" {}type="dxa"/>"#,
        name("tcW"),
        attr_prefix(prefix),
        attr_prefix(prefix)
    );
    if let Some(properties) = source.children(cell).find(|id| word(source, *id, "tcPr")) {
        if let Some(existing) = source
            .children(properties)
            .find(|id| word(source, *id, "tcW"))
        {
            return Ok(Patch {
                span: source.node(existing).expect("width exists").span(),
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
