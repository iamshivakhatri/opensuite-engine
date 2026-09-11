use super::*;

/// Patches basic direct table properties without touching rows, cells, or the grid.
pub fn set_table_formatting_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetTableFormatting,
) -> Result<Vec<u8>, OperationResult> {
    let (_, table, _, _) = resolve_table(source, &operation.table)?;
    if let Some(reason) = table_mutation_reason(source, table) {
        return Err(
            unsupported("set_table_formatting supports only simple rectangular tables")
                .with_reason_code(reason.as_str()),
        );
    }
    if operation.formatting == TableFormattingPatch::default() {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "table formatting patch is empty",
        ));
    }
    write_patches_to_vec(
        package,
        main,
        source,
        table_formatting_patches(source, table, &operation.formatting)?,
    )
}

fn table_formatting_patches(
    source: &SourceDocument,
    table: NodeId,
    patch: &TableFormattingPatch,
) -> Result<Vec<Patch>, OperationResult> {
    let prefix = word_element_prefix(source, table, "tbl")?;
    let name = |local: &str| qualify(prefix, local);
    let properties = source.children(table).find(|id| word(source, *id, "tblPr"));
    let mut changes = Vec::new();
    table_property_change(
        source,
        properties,
        "jc",
        patch.alignment.as_ref().map(|value| match value {
            PropertyPatch::Set(value) => format!(
                "<{} {}val=\"{}\"/>",
                name("jc"),
                attr_prefix(prefix),
                match value {
                    TableAlignment::Left => "left",
                    TableAlignment::Center => "center",
                    TableAlignment::Right => "right",
                }
            ),
            PropertyPatch::Clear => String::new(),
        }),
        &mut changes,
    )?;
    table_property_change(
        source,
        properties,
        "tblCellMar",
        patch.cell_margins.as_ref().map(|value| match value {
            PropertyPatch::Set(value) => table_cell_margins_xml(&name, prefix, value),
            PropertyPatch::Clear => String::new(),
        }),
        &mut changes,
    )?;
    table_property_change(
        source,
        properties,
        "tblBorders",
        patch.borders.as_ref().map(|value| match value {
            PropertyPatch::Set(TableBorders::Grid) => table_borders_xml(&name, prefix, "single"),
            PropertyPatch::Set(TableBorders::None) => table_borders_xml(&name, prefix, "nil"),
            PropertyPatch::Clear => String::new(),
        }),
        &mut changes,
    )?;
    if properties.is_some() {
        return Ok(changes);
    }
    if changes.is_empty() {
        return Ok(changes);
    }
    let at = source
        .children(table)
        .next()
        .and_then(|id| source.node(id))
        .map(|node| node.span().start)
        .ok_or_else(|| unsupported("table has no property insertion boundary"))?;
    Ok(vec![Patch {
        span: SourceSpan { start: at, end: at },
        replacement: format!(
            "<{}>{}</{}>",
            name("tblPr"),
            String::from_utf8(
                changes
                    .into_iter()
                    .flat_map(|patch| patch.replacement)
                    .collect()
            )
            .expect("generated XML is UTF-8"),
            name("tblPr")
        )
        .into_bytes(),
    }])
}

fn table_property_change(
    source: &SourceDocument,
    properties: Option<NodeId>,
    local: &str,
    replacement: Option<String>,
    changes: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    let Some(replacement) = replacement else {
        return Ok(());
    };
    let Some(properties) = properties else {
        if !replacement.is_empty() {
            changes.push(Patch {
                span: SourceSpan { start: 0, end: 0 },
                replacement: replacement.into_bytes(),
            });
        }
        return Ok(());
    };
    if let Some(existing) = source
        .children(properties)
        .find(|id| word(source, *id, local))
    {
        changes.push(Patch {
            span: source.node(existing).expect("property exists").span(),
            replacement: replacement.into_bytes(),
        });
    } else if !replacement.is_empty() {
        let at = source
            .node(properties)
            .expect("properties exist")
            .span()
            .end
            - format!(
                "</{}>",
                qualify(word_element_prefix(source, properties, "tblPr")?, "tblPr")
            )
            .len();
        changes.push(Patch {
            span: SourceSpan { start: at, end: at },
            replacement: replacement.into_bytes(),
        });
    }
    Ok(())
}

fn table_borders_xml(name: &impl Fn(&str) -> String, prefix: &str, value: &str) -> String {
    let attribute = attr_prefix(prefix);
    let edge = |edge: &str| {
        format!(
            "<{} {}val=\"{value}\" {}sz=\"4\" {}space=\"0\" {}color=\"auto\"/>",
            name(edge),
            attribute,
            attribute,
            attribute,
            attribute
        )
    };
    format!(
        "<{}>{}{}{}{}{}{}</{}>",
        name("tblBorders"),
        edge("top"),
        edge("bottom"),
        edge("left"),
        edge("right"),
        edge("insideH"),
        edge("insideV"),
        name("tblBorders")
    )
}

fn table_cell_margins_xml(
    name: &impl Fn(&str) -> String,
    prefix: &str,
    margins: &TableCellMargins,
) -> String {
    let attribute = attr_prefix(prefix);
    let edge = |edge: &str, width: u16| {
        format!(
            "<{} {}w=\"{width}\" {}type=\"dxa\"/>",
            name(edge),
            attribute,
            attribute
        )
    };
    format!(
        "<{}>{}{}{}{}</{}>",
        name("tblCellMar"),
        edge("top", margins.top_twips),
        edge("left", margins.left_twips),
        edge("bottom", margins.bottom_twips),
        edge("right", margins.right_twips),
        name("tblCellMar")
    )
}
