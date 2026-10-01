use super::*;

/// Replaces visible text in several cells of one semantic table atomically.
pub fn set_table_cells_text_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetTableCellsText,
) -> Result<Vec<u8>, OperationResult> {
    if operation.updates.is_empty() || operation.updates.len() > MAX_TABLE_CELL_UPDATES {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "table cell updates must contain between 1 and 100 updates",
        ));
    }
    let (table_index, table, rows, headers) = resolve_table(source, &operation.table)?;
    if let Some(reason) = table_mutation_reason(source, table) {
        return Err(unsupported(
            "set_table_cells_text supports only simple rectangular tables without merges, nesting, or revisions",
        )
        .with_reason_code(reason.as_str()));
    }
    let mut targets = Vec::with_capacity(operation.updates.len());
    let mut cells = HashSet::new();
    for update in &operation.updates {
        let target =
            resolve_table_cell_in_table(source, table_index, &rows, &headers, &update.target)?;
        if target.text != update.expected_current_text {
            return Err(OperationResult::failed(
                "PRECONDITION_FAILED",
                "resolved cell text does not match expected current text",
            )
            .with_reason_code("EXPECTED_TEXT_MISMATCH"));
        }
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
        patches.extend(table_cell_patches(source, target, &update.replacement)?);
    }
    let patched = apply_patches(source, patches)?;
    let mut expected = all_table_rows(source)?;
    for (target, update) in targets.iter().zip(&operation.updates) {
        expected[target.table_index][target.row_index][target.column_index] =
            update.replacement.clone();
    }
    let output = package
        .write_replaced_part_to_vec(main, &patched)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    verify_table_row_output_bytes(&output, &expected)?;
    Ok(output)
}

pub fn set_table_cell_text(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetTableCellText,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let target = match resolve_table_cell(source, &operation.target) {
        Ok(target) => target,
        Err(result) => return result,
    };
    if target.text != operation.expected_current_text {
        return OperationResult::failed(
            "PRECONDITION_FAILED",
            "resolved cell text does not match expected current text",
        );
    }
    let patches = match table_cell_patches(source, &target, &operation.replacement) {
        Ok(patches) => patches,
        Err(result) => return result,
    };
    let patched = match apply_patches(source, patches) {
        Ok(patched) => patched,
        Err(result) => return result,
    };
    let before = match all_table_cell_texts(source) {
        Ok(before) => before,
        Err(result) => return result,
    };
    let Some(index) = before.iter().position(|item| item.0 == target.cell) else {
        return OperationResult::failed(
            "DOCUMENT_INVALID",
            "resolved target cell is not in a supported table",
        );
    };
    let expected = before
        .into_iter()
        .enumerate()
        .map(|(item_index, (_, text))| {
            if item_index == index {
                operation.replacement.clone()
            } else {
                text
            }
        })
        .collect::<Vec<_>>();
    let temporary = temporary_path(output);
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) = verify_table_cell_output(
        &temporary,
        &operation.target,
        &operation.replacement,
        &expected,
    ) {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::table_cell_text_set(target.text, operation.replacement.clone())
}

pub(super) struct ResolvedTableCell {
    pub(super) cell: NodeId,
    pub(super) paragraph: NodeId,
    text: String,
    table_index: usize,
    row_index: usize,
    column_index: usize,
}

fn resolve_table_cell(
    source: &SourceDocument,
    target: &TableCellTarget,
) -> Result<ResolvedTableCell, OperationResult> {
    if let Some(handle) = &target.handle {
        let (table_index, row_index, column_index) = parse_cell_handle(handle)?;
        let table = TableTarget {
            header_cells: Vec::new(),
            occurrence: None,
            handle: Some(format!("t{table_index}")),
        };
        let (_, table_id, rows, _) = resolve_table(source, &table)?;
        if !simple_table(source, table_id) {
            return Err(unsupported(
                "set_table_cell_text supports only simple rectangular tables",
            ));
        }
        let row = rows.get(row_index).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "table cell handle row was not found")
                .with_reason_code("TABLE_ROW_NOT_FOUND")
        })?;
        let cells = row.cells().collect::<Vec<_>>();
        let cell = cells
            .get(column_index)
            .ok_or_else(|| {
                OperationResult::failed(
                    "TARGET_NOT_FOUND",
                    "table cell handle column was not found",
                )
                .with_reason_code("TABLE_COLUMN_NOT_FOUND")
            })?
            .source_id();
        let paragraphs = direct_cell_paragraphs(source, cell);
        if let Some(reason) = table_cell_text_reason(source, cell) {
            return Err(unsupported(
                "set_table_cell_text requires one ordinary paragraph with direct runs",
            )
            .with_reason_code(reason.as_str()));
        }
        return Ok(ResolvedTableCell {
            cell,
            paragraph: paragraphs[0],
            text: cell_current_text(source, cell)?,
            table_index,
            row_index,
            column_index,
        });
    }
    let mut candidates = Vec::new();
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    for block in document.blocks() {
        let crate::BodyBlock::Table(table) = block else {
            continue;
        };
        if !is_direct_body_table(source, table.source_id()) {
            continue;
        }
        let rows = table.rows().collect::<Vec<_>>();
        let Some(header) = rows.first() else {
            continue;
        };
        let header_cells = header.cells().collect::<Vec<_>>();
        for (column, header_cell) in header_cells.iter().enumerate().skip(1) {
            if !label_matches(
                &header_cell
                    .text_for_view(RevisionView::Current)
                    .map_err(document_invalid)?,
                &target.column_header,
            ) {
                continue;
            }
            for row in rows.iter().skip(1) {
                let cells = row.cells().collect::<Vec<_>>();
                if cells.first().is_some_and(|cell| {
                    cell.text_for_view(RevisionView::Current)
                        .ok()
                        .as_deref()
                        .is_some_and(|text| label_matches(text, &target.row_label))
                }) && cells.len() > column
                {
                    candidates.push((table.source_id(), cells[column].source_id()));
                }
            }
        }
    }
    if candidates.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "table row label and column header intersection was not found",
        ));
    }
    let (table_id, cell) = if let Some(occurrence) = target.occurrence {
        candidates.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed(
                "TARGET_NOT_FOUND",
                "table cell target occurrence was not found",
            )
        })?
    } else if candidates.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "table cell target matches more than one current semantic cell",
        ));
    } else {
        candidates.pop().expect("one candidate")
    };
    if !simple_table(source, table_id) {
        return Err(unsupported(
            "set_table_cell_text supports only simple rectangular tables",
        ));
    }
    let paragraphs = direct_cell_paragraphs(source, cell);
    if let Some(reason) = table_cell_text_reason(source, cell) {
        return Err(unsupported(
            "set_table_cell_text requires one ordinary paragraph with direct runs",
        )
        .with_reason_code(reason.as_str()));
    }
    let text = cell_current_text(source, cell)?;
    Ok(ResolvedTableCell {
        cell,
        paragraph: paragraphs[0],
        text,
        table_index: 0,
        row_index: 0,
        column_index: 0,
    })
}

pub(super) fn resolve_semantic_row_index(
    rows: &[crate::Row<'_>],
    row: &TableCellRow,
) -> Result<usize, OperationResult> {
    match row {
        TableCellRow::Header => Ok(0),
        TableCellRow::Label { text, occurrence } => select_cell_axis(
            rows.iter()
                .enumerate()
                .skip(1)
                .filter_map(|(index, row)| {
                    row.cells().next().and_then(|cell| {
                        cell.text_for_view(RevisionView::Current)
                            .ok()
                            .filter(|actual| label_matches(actual, text))
                            .map(|_| index)
                    })
                })
                .collect(),
            *occurrence,
            "TABLE_ROW_NOT_FOUND",
        ),
        TableCellRow::Index {
            index,
            expected_first_cell_text,
        } => {
            let actual = rows
                .get(*index)
                .and_then(|row| row.cells().next())
                .map(|cell| cell.text_for_view(RevisionView::Current))
                .transpose()
                .map_err(document_invalid)?;
            if actual.as_deref() != Some(expected_first_cell_text.as_str()) {
                return Err(OperationResult::failed(
                    "PRECONDITION_FAILED",
                    "indexed row first cell does not match expected text",
                )
                .with_reason_code("EXPECTED_TEXT_MISMATCH"));
            }
            Ok(*index)
        }
    }
}

fn select_cell_axis(
    matches: Vec<usize>,
    occurrence: Option<usize>,
    not_found_reason: &str,
) -> Result<usize, OperationResult> {
    if let Some(occurrence) = occurrence {
        return matches.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "cell selector occurrence was not found")
                .with_reason_code(not_found_reason)
        });
    }
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(
            OperationResult::failed("TARGET_NOT_FOUND", "cell selector was not found")
                .with_reason_code(not_found_reason),
        ),
        _ => Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "cell selector matches more than one row or column",
        )),
    }
}

pub(super) fn resolve_table_cell_in_table(
    source: &SourceDocument,
    table_index: usize,
    rows: &[crate::Row<'_>],
    headers: &[String],
    target: &TableCellTarget,
) -> Result<ResolvedTableCell, OperationResult> {
    if (target.row.is_some() || target.column.is_some())
        && (target.handle.is_some()
            || !target.row_label.is_empty()
            || !target.column_header.is_empty()
            || target.occurrence.is_some())
    {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "use either row and column or a legacy cell target",
        ));
    }
    if let Some(handle) = &target.handle {
        let (handle_table, row_index, column_index) = parse_cell_handle(handle)?;
        if handle_table != table_index {
            return Err(OperationResult::failed(
                "PRECONDITION_FAILED",
                "cell handle does not belong to the selected table",
            ));
        }
        let row = rows.get(row_index).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "table cell handle row was not found")
        })?;
        let cells = row.cells().collect::<Vec<_>>();
        let cell = cells
            .get(column_index)
            .ok_or_else(|| {
                OperationResult::failed(
                    "TARGET_NOT_FOUND",
                    "table cell handle column was not found",
                )
            })?
            .source_id();
        let paragraphs = direct_cell_paragraphs(source, cell);
        if let Some(reason) = table_cell_text_reason(source, cell) {
            return Err(unsupported(
                "set_table_cells_text requires one ordinary paragraph with direct runs",
            )
            .with_reason_code(reason.as_str()));
        }
        return Ok(ResolvedTableCell {
            cell,
            paragraph: paragraphs[0],
            text: cell_current_text(source, cell)?,
            table_index,
            row_index,
            column_index,
        });
    }
    if target.row.is_some() || target.column.is_some() {
        let (Some(row), Some(column)) = (&target.row, &target.column) else {
            return Err(OperationResult::failed(
                "PRECONDITION_FAILED",
                "cell target needs both row and column selectors",
            ));
        };
        let row_index = resolve_semantic_row_index(rows, row)?;
        let column_index = match column {
            TableCellColumn::First => 0,
            TableCellColumn::Header { text, occurrence } => select_cell_axis(
                headers
                    .iter()
                    .enumerate()
                    .skip(1)
                    .filter_map(|(index, actual)| label_matches(actual, text).then_some(index))
                    .collect(),
                *occurrence,
                "TABLE_COLUMN_NOT_FOUND",
            )?,
            TableCellColumn::Index {
                index,
                expected_header_text,
            } => {
                if headers.get(*index).map(String::as_str) != Some(expected_header_text.as_str()) {
                    return Err(OperationResult::failed(
                        "PRECONDITION_FAILED",
                        "indexed column header does not match expected text",
                    )
                    .with_reason_code("EXPECTED_TEXT_MISMATCH"));
                }
                *index
            }
        };
        let cell = rows[row_index]
            .cells()
            .nth(column_index)
            .ok_or_else(|| OperationResult::failed("TARGET_NOT_FOUND", "table cell was not found"))?
            .source_id();
        let paragraphs = direct_cell_paragraphs(source, cell);
        if let Some(reason) = table_cell_text_reason(source, cell) {
            return Err(unsupported(
                "set_table_cells_text requires one ordinary paragraph with direct runs",
            )
            .with_reason_code(reason.as_str()));
        }
        return Ok(ResolvedTableCell {
            cell,
            paragraph: paragraphs[0],
            text: cell_current_text(source, cell)?,
            table_index,
            row_index,
            column_index,
        });
    }
    let columns = headers
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(index, header)| label_matches(header, &target.column_header).then_some(index))
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "table column header was not found",
        )
        .with_reason_code("TABLE_COLUMN_NOT_FOUND"));
    }
    let mut candidates = Vec::new();
    for (row_index, row) in rows.iter().enumerate().skip(1) {
        let cells = row.cells().collect::<Vec<_>>();
        if cells.first().is_some_and(|cell| {
            cell.text_for_view(RevisionView::Current)
                .ok()
                .as_deref()
                .is_some_and(|text| label_matches(text, &target.row_label))
        }) {
            for &column_index in &columns {
                candidates.push((row_index, column_index, cells[column_index].source_id()));
            }
        }
    }
    let (row_index, column_index, cell) = if let Some(occurrence) = target.occurrence {
        candidates.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed(
                "TARGET_NOT_FOUND",
                "table cell target occurrence was not found",
            )
            .with_reason_code("TABLE_ROW_NOT_FOUND")
        })?
    } else if candidates.len() != 1 {
        return Err(if candidates.is_empty() {
            OperationResult::failed("TARGET_NOT_FOUND", "table row label was not found")
                .with_reason_code("TABLE_ROW_NOT_FOUND")
        } else {
            OperationResult::failed(
                "TARGET_AMBIGUOUS",
                "table cell target matches more than one current semantic cell",
            )
            .with_reason_code("TABLE_CELL_AMBIGUOUS")
        });
    } else {
        candidates.pop().expect("one candidate")
    };
    let paragraphs = direct_cell_paragraphs(source, cell);
    if let Some(reason) = table_cell_text_reason(source, cell) {
        return Err(unsupported(
            "set_table_cells_text requires one ordinary paragraph with direct runs",
        )
        .with_reason_code(reason.as_str()));
    }
    Ok(ResolvedTableCell {
        cell,
        paragraph: paragraphs[0],
        text: cell_current_text(source, cell)?,
        table_index,
        row_index,
        column_index,
    })
}

fn table_cell_patches(
    source: &SourceDocument,
    target: &ResolvedTableCell,
    replacement: &str,
) -> Result<Vec<Patch>, OperationResult> {
    if target.text.is_empty() {
        return if replacement.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(vec![empty_cell_patch(
                source,
                target.paragraph,
                replacement,
            )?])
        };
    }
    let matches = crate::text_search::resolve_text(source, &target.text)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    let matched = matches
        .into_iter()
        .find(|matched| {
            matched.start == 0
                && matched.end == target.text.len()
                && matched
                    .segments
                    .iter()
                    .all(|segment| is_descendant(source, segment.id, target.cell))
        })
        .ok_or_else(|| {
            OperationResult::failed(
                "DOCUMENT_INVALID",
                "resolved cell has no compatible text source range",
            )
        })?;
    patches_for_match(source, &matched, replacement)
}

fn cell_current_text(source: &SourceDocument, cell: NodeId) -> Result<String, OperationResult> {
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    for block in document.blocks() {
        let crate::BodyBlock::Table(table) = block else {
            continue;
        };
        for row in table.rows() {
            for item in row.cells() {
                if item.source_id() == cell {
                    return item
                        .text_for_view(RevisionView::Current)
                        .map_err(document_invalid);
                }
            }
        }
    }
    Err(OperationResult::failed(
        "DOCUMENT_INVALID",
        "resolved table cell is unavailable",
    ))
}

fn all_table_cell_texts(source: &SourceDocument) -> Result<Vec<(NodeId, String)>, OperationResult> {
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    let mut values = Vec::new();
    for block in document.blocks() {
        let crate::BodyBlock::Table(table) = block else {
            continue;
        };
        if !is_direct_body_table(source, table.source_id()) {
            continue;
        }
        for row in table.rows() {
            for cell in row.cells() {
                values.push((
                    cell.source_id(),
                    cell.text_for_view(RevisionView::Current)
                        .map_err(document_invalid)?,
                ));
            }
        }
    }
    Ok(values)
}

pub(crate) fn table_cell_text_reason(
    source: &SourceDocument,
    cell: NodeId,
) -> Option<AffordanceReason> {
    let paragraphs = direct_cell_paragraphs(source, cell);
    match paragraphs.len() {
        0 => Some(AffordanceReason::UnsafeCellStructure),
        1 if safe_table_paragraph(source, paragraphs[0]) => None,
        1 => Some(AffordanceReason::UnsafeParagraphStructure),
        _ => Some(AffordanceReason::MultipleParagraphs),
    }
}

fn verify_table_cell_output(
    output: &Path,
    target: &TableCellTarget,
    replacement: &str,
    expected_cells: &[String],
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let resolved = resolve_table_cell(&source, target)?;
    if resolved.text != replacement {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output target cell text does not match replacement",
        ));
    }
    let actual = all_table_cell_texts(&source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    (actual == expected_cells).then_some(()).ok_or_else(|| {
        OperationResult::failed(
            "DOCUMENT_INVALID",
            "output table cells do not preserve expected semantic structure",
        )
    })
}
