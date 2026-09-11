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

struct ResolvedTableCell {
    cell: NodeId,
    paragraph: NodeId,
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

fn resolve_table_cell_in_table(
    source: &SourceDocument,
    table_index: usize,
    rows: &[crate::Row<'_>],
    headers: &[String],
    target: &TableCellTarget,
) -> Result<ResolvedTableCell, OperationResult> {
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
        ));
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
        })?
    } else if candidates.len() != 1 {
        return Err(OperationResult::failed(
            if candidates.is_empty() {
                "TARGET_NOT_FOUND"
            } else {
                "TARGET_AMBIGUOUS"
            },
            "table cell target does not resolve to one current semantic cell",
        ));
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
