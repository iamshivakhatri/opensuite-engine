use super::*;

mod cell;
mod column;
mod lifecycle;
mod row;

pub(crate) use cell::table_cell_text_reason;
pub use cell::*;
pub(crate) use column::table_grid_reason;
pub use column::*;
pub use lifecycle::*;
pub use row::*;

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

pub(super) fn resolve_table<'a>(
    source: &'a SourceDocument,
    target: &TableTarget,
) -> Result<(usize, NodeId, Vec<crate::Row<'a>>, Vec<String>), OperationResult> {
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    let requested_handle = target
        .handle
        .as_deref()
        .map(parse_table_handle)
        .transpose()?;
    let mut tables = Vec::new();
    let mut table_index = 0;
    for block in document.blocks() {
        let crate::BodyBlock::Table(table) = block else {
            continue;
        };
        if !is_direct_body_table(source, table.source_id()) {
            continue;
        }
        let current_table_index = table_index;
        table_index += 1;
        let rows = table.rows().collect::<Vec<_>>();
        let Some(header) = rows.first() else { continue };
        let headers = header
            .cells()
            .map(|cell| {
                cell.text_for_view(RevisionView::Current)
                    .map_err(document_invalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if requested_handle.is_some_and(|handle| handle == current_table_index)
            || (requested_handle.is_none() && labels_match(&headers, &target.header_cells))
        {
            tables.push((current_table_index, table.source_id(), rows, headers));
        }
    }
    select_table(tables, target)
}

pub(super) fn parse_table_handle(handle: &str) -> Result<usize, OperationResult> {
    handle
        .strip_prefix('t')
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| OperationResult::failed("PRECONDITION_FAILED", "table handle is malformed"))
}

pub(super) fn parse_row_handle(handle: &str) -> Result<(usize, usize), OperationResult> {
    let Some((table, row)) = handle.split_once(":r") else {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "row handle is malformed",
        ));
    };
    Ok((
        parse_table_handle(table)?,
        row.parse().map_err(|_| {
            OperationResult::failed("PRECONDITION_FAILED", "row handle is malformed")
        })?,
    ))
}

pub(super) fn parse_cell_handle(handle: &str) -> Result<(usize, usize, usize), OperationResult> {
    let Some((row, column)) = handle.split_once(":c") else {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "cell handle is malformed",
        ));
    };
    let (table, row) = parse_row_handle(row)?;
    Ok((
        table,
        row,
        column.parse().map_err(|_| {
            OperationResult::failed("PRECONDITION_FAILED", "cell handle is malformed")
        })?,
    ))
}

pub(super) fn select_table<'a>(
    mut tables: Vec<(usize, NodeId, Vec<crate::Row<'a>>, Vec<String>)>,
    target: &TableTarget,
) -> Result<(usize, NodeId, Vec<crate::Row<'a>>, Vec<String>), OperationResult> {
    if tables.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "table header row was not found",
        ));
    }
    if let Some(occurrence) = target.occurrence {
        return tables.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "table target occurrence was not found")
        });
    }
    if tables.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "table header row matches more than one table",
        ));
    }
    Ok(tables.pop().expect("one table"))
}

pub(super) fn safe_template_cell(source: &SourceDocument, cell: NodeId) -> bool {
    let paragraphs = direct_cell_paragraphs(source, cell);
    paragraphs.len() == 1
        && source
            .children(cell)
            .all(|child| word(source, child, "tcPr") || word(source, child, "p"))
        && safe_table_paragraph(source, paragraphs[0])
        && source.children(paragraphs[0]).all(|child| {
            word(source, child, "pPr")
                || word(source, child, "r")
                || matches!(
                    source.node(child).map(|node| node.kind()),
                    Some(SourceNodeKind::Text)
                )
        })
}

pub(super) fn label_matches(actual: &str, requested: &str) -> bool {
    actual.trim() == requested.trim()
}

pub(super) fn labels_match(actual: &[String], requested: &[String]) -> bool {
    actual.len() == requested.len()
        && actual
            .iter()
            .zip(requested)
            .all(|(actual, requested)| label_matches(actual, requested))
}

pub(super) fn minimal_table_cell_fragment(
    source: &SourceDocument,
    row: NodeId,
    value: &str,
) -> Result<String, OperationResult> {
    let prefix = word_element_prefix(source, row, "tr")?;
    let name = |local: &str| qualify(prefix, local);
    let space = if requires_space_preservation(value) {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    Ok(format!(
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
    ))
}

pub(super) fn cell_fragment(
    source: &SourceDocument,
    cell: NodeId,
    value: &str,
    prefix: &str,
) -> Result<String, OperationResult> {
    let name = |local: &str| qualify(prefix, local);
    let properties = source
        .children(cell)
        .find(|child| word(source, *child, "tcPr"))
        .map(|id| source_bytes(source, id))
        .transpose()?
        .unwrap_or_default();
    let paragraph = direct_cell_paragraphs(source, cell)[0];
    let paragraph_properties = source
        .children(paragraph)
        .find(|child| word(source, *child, "pPr"))
        .map(|id| source_bytes(source, id))
        .transpose()?
        .unwrap_or_default();
    let run_properties = source
        .children(paragraph)
        .find(|child| word(source, *child, "r"))
        .and_then(|run| {
            source
                .children(run)
                .find(|child| word(source, *child, "rPr"))
        })
        .map(|id| source_bytes(source, id))
        .transpose()?
        .unwrap_or_default();
    let space = if requires_space_preservation(value) {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    Ok(format!(
        "<{}>{}<{}>{}<{}>{}<{}{}>{}</{}></{}></{}></{}>",
        name("tc"),
        String::from_utf8_lossy(&properties),
        name("p"),
        String::from_utf8_lossy(&paragraph_properties),
        name("r"),
        String::from_utf8_lossy(&run_properties),
        name("t"),
        space,
        escape(value),
        name("t"),
        name("r"),
        name("p"),
        name("tc")
    ))
}

pub(super) fn source_bytes(
    source: &SourceDocument,
    id: NodeId,
) -> Result<Vec<u8>, OperationResult> {
    let span = source
        .node(id)
        .ok_or_else(|| unsupported("template source is unavailable"))?
        .span();
    Ok(source.original_bytes()[span.start..span.end].to_vec())
}

pub(super) fn word_element_prefix<'a>(
    source: &'a SourceDocument,
    id: NodeId,
    local: &str,
) -> Result<&'a str, OperationResult> {
    let SourceNodeKind::Element { start_tag, .. } = source
        .node(id)
        .ok_or_else(|| unsupported("template source is unavailable"))?
        .kind()
    else {
        return Err(unsupported("template source node is not an element"));
    };
    let tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
        .map_err(|_| unsupported("template tag is not UTF-8"))?;
    let name = tag
        .strip_prefix('<')
        .and_then(|value| {
            value
                .split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
                .next()
        })
        .ok_or_else(|| unsupported("template tag is invalid"))?;
    let expected = if name == local {
        local
    } else {
        name.rsplit(':').next().unwrap_or("")
    };
    (expected == local)
        .then_some(
            name.strip_suffix(local)
                .and_then(|value| value.strip_suffix(':'))
                .unwrap_or(""),
        )
        .ok_or_else(|| unsupported("template tag is not WordprocessingML"))
}

pub(super) fn all_table_rows(
    source: &SourceDocument,
) -> Result<Vec<Vec<Vec<String>>>, OperationResult> {
    crate::DocxDocument::new(source)
        .map_err(document_invalid)?
        .blocks()
        .filter_map(|block| match block {
            crate::BodyBlock::Table(table) if is_direct_body_table(source, table.source_id()) => {
                Some(table)
            }
            _ => None,
        })
        .map(|table| {
            table
                .rows()
                .map(|row| {
                    row.cells()
                        .map(|cell| {
                            cell.text_for_view(RevisionView::Current)
                                .map_err(document_invalid)
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}

pub(super) fn verify_table_row_output_bytes(
    output: &[u8],
    expected: &[Vec<Vec<String>>],
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    (all_table_rows(&source)? == expected)
        .then_some(())
        .ok_or_else(|| {
            OperationResult::failed(
                "DOCUMENT_INVALID",
                "output table rows do not match the requested insertion",
            )
        })
}

pub(super) fn is_direct_body_table(source: &SourceDocument, table: NodeId) -> bool {
    source
        .node(table)
        .and_then(|node| node.parent())
        .is_some_and(|body| word(source, body, "body"))
}

pub(super) fn simple_table(source: &SourceDocument, table: NodeId) -> bool {
    table_structure_reason(source, table).is_none()
}

pub(crate) fn table_mutation_reason(
    source: &SourceDocument,
    table: NodeId,
) -> Option<AffordanceReason> {
    table_mutation_reason_from_structure(source, table, table_structure_reason(source, table))
}

pub(crate) fn table_mutation_reason_from_structure(
    source: &SourceDocument,
    table: NodeId,
    structure_reason: Option<AffordanceReason>,
) -> Option<AffordanceReason> {
    structure_reason.or_else(|| {
        has_revision_wrapper(source, table).then_some(AffordanceReason::RevisionWrapper)
    })
}

pub(crate) fn table_structure_reason(
    source: &SourceDocument,
    table: NodeId,
) -> Option<AffordanceReason> {
    if source
        .node_ids()
        .any(|id| id != table && word(source, id, "tbl") && is_descendant(source, id, table))
    {
        return Some(AffordanceReason::NestedTableStructure);
    }
    if source.node_ids().any(|id| {
        is_descendant(source, id, table)
            && (word(source, id, "gridSpan") || word(source, id, "vMerge"))
    }) {
        return Some(AffordanceReason::MergedTableStructure);
    }
    let rows = source
        .children(table)
        .filter(|id| word(source, *id, "tr"))
        .collect::<Vec<_>>();
    let Some(first) = rows.first() else {
        return Some(AffordanceReason::NonRectangularTable);
    };
    let width = source
        .children(*first)
        .filter(|id| word(source, *id, "tc"))
        .count();
    (width > 0
        && !rows.is_empty()
        && rows.iter().all(|row| {
            source
                .children(*row)
                .filter(|id| word(source, *id, "tc"))
                .count()
                == width
        }))
    .then_some(())
    .is_none()
    .then_some(AffordanceReason::NonRectangularTable)
}

pub(super) fn direct_cell_paragraphs(source: &SourceDocument, cell: NodeId) -> Vec<NodeId> {
    source
        .children(cell)
        .filter(|id| word(source, *id, "p"))
        .collect()
}

pub(super) fn safe_table_paragraph(source: &SourceDocument, paragraph: NodeId) -> bool {
    source.children(paragraph).all(|id| {
        if matches!(
            source.node(id).map(|node| node.kind()),
            Some(SourceNodeKind::Text)
        ) {
            true
        } else if word(source, id, "pPr") {
            !source
                .children(id)
                .any(|child| word(source, child, "sectPr") || word(source, child, "pPrChange"))
        } else if word(source, id, "r") {
            source
                .children(id)
                .all(|child| safe_run_child(source, child))
        } else {
            false
        }
    })
}

pub(super) fn is_descendant(source: &SourceDocument, mut id: NodeId, ancestor: NodeId) -> bool {
    while let Some(parent) = source.node(id).and_then(|node| node.parent()) {
        if parent == ancestor {
            return true;
        }
        id = parent;
    }
    false
}

pub(super) fn empty_cell_patch(
    source: &SourceDocument,
    paragraph: NodeId,
    replacement: &str,
) -> Result<Patch, OperationResult> {
    let fragment = run_fragment(source, paragraph, replacement)?;
    let SourceNodeKind::Element {
        start_tag, end_tag, ..
    } = source.node(paragraph).expect("paragraph exists").kind()
    else {
        return Err(unsupported("empty cell paragraph has no source tag"));
    };
    if let Some(end_tag) = end_tag {
        return Ok(Patch {
            span: SourceSpan {
                start: end_tag.start,
                end: end_tag.start,
            },
            replacement: fragment,
        });
    }
    let tag = &source.original_bytes()[start_tag.start..start_tag.end];
    let Some(offset) = tag.windows(2).rposition(|window| window == b"/>") else {
        return Err(unsupported(
            "empty cell paragraph cannot receive a source insertion",
        ));
    };
    Ok(Patch {
        span: SourceSpan {
            start: start_tag.start + offset,
            end: start_tag.end,
        },
        replacement: format!(
            ">{}</{}>",
            String::from_utf8(fragment).expect("generated XML is UTF-8"),
            qualified_name(source, paragraph, "p")?
        )
        .into_bytes(),
    })
}

pub(super) fn run_fragment(
    source: &SourceDocument,
    paragraph: NodeId,
    text: &str,
) -> Result<Vec<u8>, OperationResult> {
    let run = qualified_name(source, paragraph, "r")?;
    let value = qualified_name(source, paragraph, "t")?;
    let space = if requires_space_preservation(text) {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    Ok(format!("<{run}><{value}{space}>{}</{value}></{run}>", escape(text)).into_bytes())
}

pub(super) fn table_fragment_for_body(
    source: &SourceDocument,
    body: NodeId,
    rows: &[Vec<String>],
) -> Result<Vec<u8>, OperationResult> {
    let prefix = word_prefix_for(source, body, "body")?;
    let name = |local: &str| qualify(&prefix, local);
    let width_attribute = attr_prefix(&prefix);
    let column_width = 9360 / rows[0].len();
    let remainder = 9360 % rows[0].len();
    let mut value = format!(
        "<{}><{}><{} {}w=\"9360\" {}type=\"dxa\"/><{}><{} {}val=\"single\" {}sz=\"4\" {}space=\"0\" {}color=\"auto\"/><{} {}val=\"single\" {}sz=\"4\" {}space=\"0\" {}color=\"auto\"/><{} {}val=\"single\" {}sz=\"4\" {}space=\"0\" {}color=\"auto\"/><{} {}val=\"single\" {}sz=\"4\" {}space=\"0\" {}color=\"auto\"/><{} {}val=\"single\" {}sz=\"4\" {}space=\"0\" {}color=\"auto\"/><{} {}val=\"single\" {}sz=\"4\" {}space=\"0\" {}color=\"auto\"/></{}><{}><{} {}w=\"100\" {}type=\"dxa\"/><{} {}w=\"140\" {}type=\"dxa\"/><{} {}w=\"100\" {}type=\"dxa\"/><{} {}w=\"140\" {}type=\"dxa\"/></{}></{}><{}>",
        name("tbl"),
        name("tblPr"),
        name("tblW"),
        width_attribute,
        width_attribute,
        name("tblBorders"),
        name("top"),
        width_attribute,
        width_attribute,
        width_attribute,
        width_attribute,
        name("bottom"),
        width_attribute,
        width_attribute,
        width_attribute,
        width_attribute,
        name("left"),
        width_attribute,
        width_attribute,
        width_attribute,
        width_attribute,
        name("right"),
        width_attribute,
        width_attribute,
        width_attribute,
        width_attribute,
        name("insideH"),
        width_attribute,
        width_attribute,
        width_attribute,
        width_attribute,
        name("insideV"),
        width_attribute,
        width_attribute,
        width_attribute,
        width_attribute,
        name("tblBorders"),
        name("tblCellMar"),
        name("top"),
        width_attribute,
        width_attribute,
        name("left"),
        width_attribute,
        width_attribute,
        name("bottom"),
        width_attribute,
        width_attribute,
        name("right"),
        width_attribute,
        width_attribute,
        name("tblCellMar"),
        name("tblPr"),
        name("tblGrid")
    );
    for index in 0..rows[0].len() {
        let width = column_width + usize::from(index < remainder);
        value.push_str(&format!(
            "<{} {}w=\"{width}\"/>",
            name("gridCol"),
            width_attribute
        ));
    }
    value.push_str(&format!("</{}>", name("tblGrid")));
    for row in rows {
        value.push_str(&format!("<{}>", name("tr")));
        for text in row {
            let space = if requires_space_preservation(text) {
                " xml:space=\"preserve\""
            } else {
                ""
            };
            value.push_str(&format!(
                "<{}><{}><{}><{} {}before=\"0\" {}after=\"0\"/></{}><{}><{}{}>{}</{}></{}></{}></{}>",
                name("tc"),
                name("p"),
                name("pPr"),
                name("spacing"),
                width_attribute,
                width_attribute,
                name("pPr"),
                name("r"),
                name("t"),
                space,
                escape(text),
                name("t"),
                name("r"),
                name("p"),
                name("tc")
            ));
        }
        value.push_str(&format!("</{}>", name("tr")));
    }
    value.push_str(&format!("</{}>", name("tbl")));
    Ok(value.into_bytes())
}

pub(super) fn table_formatting_patches(
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

pub(super) fn table_property_change(
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

pub(super) fn table_borders_xml(
    name: &impl Fn(&str) -> String,
    prefix: &str,
    value: &str,
) -> String {
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

pub(super) fn table_cell_margins_xml(
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

pub(super) fn expected_table_insert(
    source: &SourceDocument,
    blocks: &[NodeId],
    index: usize,
    rows: &[Vec<String>],
) -> Result<Vec<Vec<Vec<String>>>, OperationResult> {
    let mut expected = all_table_rows(source)?;
    let table_index = blocks[..index]
        .iter()
        .filter(|id| word(source, **id, "tbl"))
        .count();
    expected.insert(table_index, rows.to_vec());
    Ok(expected)
}

pub(super) fn table_current_text(table: crate::Table<'_>) -> Result<String, SemanticError> {
    let mut text = String::new();
    for row in table.rows() {
        for cell in row.cells() {
            text.push_str(&cell.text_for_view(RevisionView::Current)?);
        }
    }
    Ok(text)
}
