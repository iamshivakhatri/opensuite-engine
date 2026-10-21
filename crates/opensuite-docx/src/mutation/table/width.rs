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
    // An absolute preferred width must agree with the new grid and cell widths.
    for properties in source
        .children(table)
        .filter(|id| word(source, *id, "tblPr"))
    {
        for id in source
            .children(properties)
            .filter(|id| word(source, *id, "tblW"))
        {
            let node = source.node(id).expect("table width exists");
            if node.attribute("type") != Some("dxa") {
                continue;
            }
            let SourceNodeKind::Element { start_tag, .. } = node.kind() else {
                unreachable!("table width is an element");
            };
            let tag = &source.original_bytes()[start_tag.start..start_tag.end];
            let mut reader = quick_xml::Reader::from_reader(tag);
            let event = match reader.read_event().map_err(document_invalid)? {
                quick_xml::events::Event::Start(event) | quick_xml::events::Event::Empty(event) => {
                    event
                }
                _ => return Err(unsupported("table width has no start tag")),
            };
            let key = qualify(word_element_prefix(source, id, "tblW")?, "w");
            let width = event
                .attributes()
                .find_map(|attribute| match attribute {
                    Ok(attribute) if attribute.key.as_ref() == key.as_bytes() => {
                        Some(Ok(attribute))
                    }
                    Err(error) => Some(Err(error)),
                    _ => None,
                })
                .transpose()
                .map_err(document_invalid)?
                .ok_or_else(|| unsupported("absolute table width has no width value"))?;
            let start = start_tag.start + width.value.as_ptr() as usize - tag.as_ptr() as usize;
            patches.push(Patch {
                span: SourceSpan {
                    start,
                    end: start + width.value.len(),
                },
                replacement: operation
                    .widths_twips
                    .iter()
                    .map(|width| u64::from(*width))
                    .sum::<u64>()
                    .to_string()
                    .into_bytes(),
            });
        }
    }
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
