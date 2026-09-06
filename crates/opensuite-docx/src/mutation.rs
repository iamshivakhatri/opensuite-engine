use std::{collections::HashSet, path::Path};

use quick_xml::escape::escape;

use opensuite_opc::{Package, Part};
use opensuite_protocol::{
    ContentControlTarget, DeleteParagraph, InsertParagraphAfter, InsertTableRowAfter,
    InsertTableRowsAfter, OperationResult, ParagraphFormattingPatch, PropertyPatch, ReplacePicture,
    ReplaceText, SetContentControlText, SetParagraphFormatting, SetParagraphStyle,
    SetTableCellText, SetTableCellsText, SetTextFormatting, TableCellTarget, TableRowTarget,
    TableTarget, TextFormattingPatch, TextTarget,
};

use crate::{NodeId, RevisionView, SemanticError, SourceDocument, SourceNodeKind, SourceSpan};

const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
const MAX_TABLE_ROWS_PER_OPERATION: usize = 100;
const MAX_TABLE_CELL_UPDATES: usize = 100;

/// Applies one preservation-safe replacement across compatible `w:t` source regions.
pub fn replace_text(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &ReplaceText,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let target = match resolve_target(source, operation) {
        Ok(target) => target,
        Err(result) => return result,
    };
    if target.value != operation.expected_current_text {
        return OperationResult::failed(
            "PRECONDITION_FAILED",
            "resolved text does not match expected current text",
        );
    }
    let patched = match apply_patches(source, target.patches) {
        Ok(patched) => patched,
        Err(result) => return result,
    };
    let temporary = output.with_file_name(format!(
        ".opensuite-{}-{}.docx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) = verify_output(&temporary, &operation.replacement) {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::applied(
        operation.expected_current_text.clone(),
        operation.replacement.clone(),
    )
}

/// Applies one preservation-safe text replacement and returns the updated DOCX bytes.
pub fn replace_text_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &ReplaceText,
) -> Result<Vec<u8>, OperationResult> {
    let target = resolve_target(source, operation)?;
    if target.value != operation.expected_current_text {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "resolved text does not match expected current text",
        ));
    }
    let patched = apply_patches(source, target.patches)?;
    let output = package
        .write_replaced_part_to_vec(main, &patched)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    verify_output_bytes(&output, &operation.replacement)?;
    Ok(output)
}

/// Inserts one plain paragraph after a safe, direct main-body paragraph anchor.
pub fn insert_paragraph_after(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertParagraphAfter,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let (anchor, paragraph) = match resolve_paragraph_anchor(source, &operation.anchor) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let fragment = match paragraph_fragment(source, paragraph, &operation.text) {
        Ok(fragment) => fragment,
        Err(result) => return result,
    };
    let insertion = source
        .node(paragraph)
        .expect("anchor paragraph exists")
        .span()
        .end;
    let patched = match apply_patches(
        source,
        vec![Patch {
            span: SourceSpan {
                start: insertion,
                end: insertion,
            },
            replacement: fragment,
        }],
    ) {
        Ok(patched) => patched,
        Err(result) => return result,
    };
    let temporary = output.with_file_name(format!(
        ".opensuite-{}-{}.docx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) =
        verify_inserted_output(&temporary, &operation.anchor, &anchor, &operation.text)
    {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::paragraph_inserted(anchor, operation.text.clone())
}

/// Deletes one safe, direct main-body paragraph selected by Current-view text.
pub fn delete_paragraph(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &DeleteParagraph,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let (_target, paragraph) = match resolve_paragraph_anchor(source, &operation.target) {
        Ok(value) => value,
        Err(result) => return result,
    };
    if !safe_to_delete(source, paragraph) {
        return unsupported(
            "delete_paragraph rejects paragraphs with ranges, fields, controls, revisions, or unsupported wrappers",
        );
    }
    let body = match body_texts(source) {
        Ok(body) => body,
        Err(result) => return result,
    };
    let paragraph_text = match crate::DocxDocument::new(source)
        .map_err(document_invalid)
        .and_then(|document| {
            document
                .paragraphs()
                .find(|item| item.source_id() == paragraph)
                .ok_or_else(|| {
                    OperationResult::failed(
                        "DOCUMENT_INVALID",
                        "anchor paragraph is not in the body",
                    )
                })
                .and_then(|item| {
                    item.text_for_view(RevisionView::Current)
                        .map_err(document_invalid)
                })
        }) {
        Ok(text) => text,
        Err(result) => return result,
    };
    let body_index = match body.iter().position(|item| item.0 == paragraph) {
        Some(index) => index,
        None => {
            return OperationResult::failed(
                "DOCUMENT_INVALID",
                "anchor paragraph is not a body block",
            );
        }
    };
    let expected_body = body.into_iter().map(|(_, text)| text).collect::<Vec<_>>();
    let mut expected_after = expected_body.clone();
    expected_after.remove(body_index);
    let span = source
        .node(paragraph)
        .expect("anchor paragraph exists")
        .span();
    let patched = match apply_patches(
        source,
        vec![Patch {
            span,
            replacement: Vec::new(),
        }],
    ) {
        Ok(patched) => patched,
        Err(result) => return result,
    };
    let temporary = temporary_path(output);
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) = verify_deleted_output(&temporary, &expected_after) {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::paragraph_deleted(paragraph_text)
}

/// Sets visible text in one simple, semantically addressed main-body table cell.
/// Inserts one complete row after a semantic row anchor in a simple main-body table.
pub fn insert_table_row_after_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertTableRowAfter,
) -> Result<Vec<u8>, OperationResult> {
    let rows = std::slice::from_ref(&operation.cells);
    insert_table_rows_after(
        source,
        package,
        main,
        &operation.table,
        &operation.after,
        rows,
    )
}

/// Inserts several complete rows with one contiguous source insertion.
pub fn insert_table_rows_after_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertTableRowsAfter,
) -> Result<Vec<u8>, OperationResult> {
    insert_table_rows_after(
        source,
        package,
        main,
        &operation.table,
        &operation.after,
        &operation.rows,
    )
}

fn insert_table_rows_after(
    source: &SourceDocument,
    package: &Package,
    main: &Part,
    table: &TableTarget,
    after: &TableRowTarget,
    rows: &[Vec<String>],
) -> Result<Vec<u8>, OperationResult> {
    if rows.is_empty() || rows.len() > MAX_TABLE_ROWS_PER_OPERATION {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "inserted rows must contain between 1 and 100 rows",
        ));
    }
    let target = resolve_table_row(source, table, after, rows)?;
    let mut fragment = Vec::new();
    for cells in rows {
        fragment.extend(table_row_fragment(source, target.row, cells)?);
    }
    let insertion = source.node(target.row).expect("row exists").span().end;
    let patched = apply_patches(
        source,
        vec![Patch {
            span: SourceSpan {
                start: insertion,
                end: insertion,
            },
            replacement: fragment,
        }],
    )?;
    let before = all_table_rows(source)?;
    let mut expected = before.clone();
    expected[target.table_index].splice(
        target.row_index + 1..target.row_index + 1,
        rows.iter().cloned(),
    );
    let output = package
        .write_replaced_part_to_vec(main, &patched)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    verify_table_row_output_bytes(&output, &expected)?;
    Ok(output)
}

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
    if !simple_table(source, table) || has_revision_wrapper(source, table) {
        return Err(unsupported(
            "set_table_cells_text supports only simple rectangular tables without merges, nesting, or revisions",
        ));
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
            ));
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
    let patches = if target.text.is_empty() {
        if operation.replacement.is_empty() {
            Vec::new()
        } else {
            match empty_cell_patch(source, target.paragraph, &operation.replacement) {
                Ok(patch) => vec![patch],
                Err(result) => return result,
            }
        }
    } else {
        let matches = match crate::text_search::resolve_text(source, &target.text) {
            Ok(matches) => matches,
            Err(error) => return OperationResult::failed(error.code(), error.to_string()),
        };
        let Some(matched) = matches.into_iter().find(|matched| {
            matched.start == 0
                && matched.end == target.text.len()
                && matched
                    .segments
                    .iter()
                    .all(|segment| is_descendant(source, segment.id, target.cell))
        }) else {
            return OperationResult::failed(
                "DOCUMENT_INVALID",
                "resolved cell has no compatible text source range",
            );
        };
        match patches_for_match(source, &matched, &operation.replacement) {
            Ok(patches) => patches,
            Err(result) => return result,
        }
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

/// Sets visible text in one simple semantic Word content control.
pub fn set_content_control_text(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetContentControlText,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let target = match resolve_content_control(source, &operation.target) {
        Ok(target) => target,
        Err(result) => return result,
    };
    if target.text != operation.expected_current_text {
        return OperationResult::failed(
            "PRECONDITION_FAILED",
            "resolved content control text does not match expected current text",
        );
    }
    let patches = if target.text.is_empty() {
        if operation.replacement.is_empty() {
            Vec::new()
        } else {
            match empty_cell_patch(source, target.paragraph, &operation.replacement) {
                Ok(patch) => vec![patch],
                Err(result) => return result,
            }
        }
    } else {
        let matches = match crate::text_search::resolve_text(source, &target.text) {
            Ok(matches) => matches,
            Err(error) => return OperationResult::failed(error.code(), error.to_string()),
        };
        let Some(matched) = matches.into_iter().find(|matched| {
            matched.start == 0
                && matched.end == target.text.len()
                && matched
                    .segments
                    .iter()
                    .all(|segment| is_descendant(source, segment.id, target.control))
        }) else {
            return OperationResult::failed(
                "DOCUMENT_INVALID",
                "resolved content control has no compatible text source range",
            );
        };
        match patches_for_match(source, &matched, &operation.replacement) {
            Ok(patches) => patches,
            Err(result) => return result,
        }
    };
    let patched = match apply_patches(source, patches) {
        Ok(patched) => patched,
        Err(result) => return result,
    };
    let before = match all_content_control_texts(source) {
        Ok(values) => values,
        Err(result) => return result,
    };
    let Some(index) = before.iter().position(|item| item.0 == target.control) else {
        return OperationResult::failed(
            "DOCUMENT_INVALID",
            "resolved content control is unavailable",
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
    if let Err(result) = verify_content_control_output(
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
    OperationResult::content_control_text_set(target.text, operation.replacement.clone())
}

/// Sets selected direct paragraph-formatting properties without touching paragraph content.
pub fn set_paragraph_formatting(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetParagraphFormatting,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let (text, paragraph) = match resolve_paragraph_anchor(source, &operation.target) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let patches = match formatting_patches(source, paragraph, &operation.formatting) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let patched = match apply_patches(source, patches) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let temporary = temporary_path(output);
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) =
        verify_formatting_output(&temporary, &operation.target, &text, &operation.formatting)
    {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::paragraph_formatting_set(text)
}

/// Sets or clears only the direct paragraph style reference.
pub fn set_paragraph_style(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetParagraphStyle,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let (text, paragraph) = match resolve_paragraph_anchor(source, &operation.target) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let styles = match crate::load_styles(package, main) {
        Ok(styles) => styles,
        Err(error) => return document_invalid(error),
    };
    let resolved_style = match resolve_paragraph_style(styles.as_ref(), &operation.style) {
        Ok(style) => style,
        Err(result) => return result,
    };
    let before = direct_paragraph_style_name(source, paragraph, styles.as_ref());
    let direct_formatting = match source
        .children(paragraph)
        .find(|id| word(source, *id, "pPr"))
        .map(|ppr| crate::styles::paragraph_formatting(source, ppr))
        .transpose()
    {
        Ok(value) => value.unwrap_or_default(),
        Err(error) => return document_invalid(error),
    };
    let patches = match paragraph_style_patches(
        source,
        paragraph,
        resolved_style.as_ref().map(|style| style.0.as_str()),
    ) {
        Ok(patches) => patches,
        Err(result) => return result,
    };
    let patched = match apply_patches(source, patches) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let temporary = temporary_path(output);
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) = verify_paragraph_style_output(
        &temporary,
        &operation.target,
        &text,
        resolved_style.as_ref(),
        &direct_formatting,
    ) {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    let after = resolved_style.map_or_else(|| "direct style cleared".to_owned(), |style| style.1);
    OperationResult::paragraph_style_set(text, before, after)
}

pub fn replace_picture(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &ReplacePicture,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if operation.target.name.is_none() && operation.target.description.is_none() {
        return OperationResult::failed(
            "TARGET_NOT_FOUND",
            "picture target requires name or description",
        );
    }
    let document = match crate::DocxDocument::new(source) {
        Ok(value) => value,
        Err(error) => return document_invalid(error),
    };
    let mut pictures = document
        .pictures()
        .filter(|picture| {
            picture.metadata().is_some_and(|meta| {
                operation
                    .target
                    .name
                    .as_ref()
                    .is_none_or(|name| meta.name.as_ref() == Some(name))
                    && operation
                        .target
                        .description
                        .as_ref()
                        .is_none_or(|description| meta.description.as_ref() == Some(description))
            })
        })
        .collect::<Vec<_>>();
    let picture = if let Some(occurrence) = operation.target.occurrence {
        match pictures.get(occurrence) {
            Some(_) => pictures.remove(occurrence),
            None => {
                return OperationResult::failed(
                    "TARGET_NOT_FOUND",
                    "picture target occurrence was not found",
                );
            }
        }
    } else if pictures.len() != 1 {
        return OperationResult::failed(
            if pictures.is_empty() {
                "TARGET_NOT_FOUND"
            } else {
                "TARGET_AMBIGUOUS"
            },
            "picture target did not resolve deterministically",
        );
    } else {
        pictures.pop().expect("one picture")
    };
    let metadata = picture.metadata().expect("matched metadata");
    let name = metadata
        .name
        .clone()
        .or(metadata.description.clone())
        .unwrap_or_else(|| "picture".to_owned());
    let crate::ImageReference::Embedded(image) = (match picture.image_reference(package, main) {
        Ok(value) => value,
        Err(error) => return unsupported(error.to_string()),
    }) else {
        return unsupported("replace_picture supports only internal embedded images");
    };
    let content_type = image.part.content_type.as_str();
    if !matches!(content_type, "image/png" | "image/jpeg")
        || operation.replacement.content_type != content_type
        || !valid_image(&operation.replacement.bytes, content_type)
    {
        return unsupported(
            "replacement image must be a valid PNG or JPEG with the existing content type",
        );
    }
    let references = match crate::DocxDocument::new(source) { Ok(document) => document.pictures().filter_map(|item| item.image_reference(package, main).ok()).filter(|reference| matches!(reference, crate::ImageReference::Embedded(other) if other.part.name == image.part.name)).count(), Err(error) => return document_invalid(error) };
    if references != 1 {
        return unsupported("replace_picture does not replace shared image parts");
    }
    if let Err(error) =
        package.write_replaced_part(&image.part, &operation.replacement.bytes, output)
    {
        return OperationResult::failed(error.code(), error.to_string());
    }
    let reopened = match Package::open(output) {
        Ok(value) => value,
        Err(error) => return document_invalid(error),
    };
    if let Err(error) = reopened.verify() {
        return document_invalid(error);
    }
    let (reopened_main, reopened_source) = match crate::open_main_source(&reopened) {
        Ok(value) => value,
        Err(error) => return document_invalid(error),
    };
    let output_picture = match crate::DocxDocument::new(&reopened_source) {
        Ok(document) => document
            .pictures()
            .filter(|picture| {
                picture.metadata().is_some_and(|meta| {
                    operation
                        .target
                        .name
                        .as_ref()
                        .is_none_or(|name| meta.name.as_ref() == Some(name))
                        && operation
                            .target
                            .description
                            .as_ref()
                            .is_none_or(|description| {
                                meta.description.as_ref() == Some(description)
                            })
                })
            })
            .nth(operation.target.occurrence.unwrap_or(0)),
        Err(error) => return document_invalid(error),
    };
    let Some(output_picture) = output_picture else {
        return OperationResult::failed("DOCUMENT_INVALID", "output picture target was not found");
    };
    if output_picture.kind() != picture.kind()
        || output_picture.extent().ok() != picture.extent().ok()
    {
        return OperationResult::failed(
            "DOCUMENT_INVALID",
            "output picture metadata or dimensions changed",
        );
    }
    let crate::ImageReference::Embedded(output_image) =
        (match output_picture.image_reference(&reopened, &reopened_main) {
            Ok(value) => value,
            Err(error) => return document_invalid(error),
        })
    else {
        return OperationResult::failed(
            "DOCUMENT_INVALID",
            "output picture image relationship changed",
        );
    };
    if output_image.part.content_type != image.part.content_type
        || reopened
            .read_part(&output_image.part)
            .map_err(document_invalid)
            .ok()
            .as_deref()
            != Some(operation.replacement.bytes.as_slice())
    {
        return OperationResult::failed(
            "DOCUMENT_INVALID",
            "output picture payload does not match replacement",
        );
    }
    OperationResult::picture_replaced(
        name,
        image.size_bytes as usize,
        operation.replacement.bytes.len(),
    )
}

fn valid_image(bytes: &[u8], content_type: &str) -> bool {
    match content_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]) && bytes.ends_with(&[0xff, 0xd9]),
        _ => false,
    }
}

fn resolve_paragraph_style(
    styles: Option<&crate::StyleSheet>,
    style: &PropertyPatch<String>,
) -> Result<Option<(String, String)>, OperationResult> {
    let PropertyPatch::Set(name) = style else {
        return Ok(None);
    };
    if name.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "paragraph style name must not be empty",
        ));
    }
    let Some(styles) = styles else {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "document has no stylesheet",
        ));
    };
    let matches = styles
        .styles()
        .filter(|style| style.name() == Some(name.as_str()))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "paragraph style name was not found",
        ));
    }
    if matches.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "paragraph style name matches more than one style",
        ));
    }
    (matches[0].style_type() == crate::StyleType::Paragraph)
        .then_some(Some((matches[0].id().as_str().to_owned(), name.clone())))
        .ok_or_else(|| unsupported("requested style is not a paragraph style"))
}

fn direct_paragraph_style_name(
    source: &SourceDocument,
    paragraph: NodeId,
    styles: Option<&crate::StyleSheet>,
) -> String {
    let id = source
        .children(paragraph)
        .find(|id| word(source, *id, "pPr"))
        .and_then(|ppr| source.children(ppr).find(|id| word(source, *id, "pStyle")))
        .and_then(|id| source.node(id))
        .and_then(|node| node.attribute("val"));
    id.and_then(|id| {
        styles
            .and_then(|styles| styles.styles().find(|style| style.id().as_str() == id))
            .and_then(crate::Style::name)
    })
    .map(str::to_owned)
    .unwrap_or_else(|| "direct style cleared".to_owned())
}

fn paragraph_style_patches(
    source: &SourceDocument,
    paragraph: NodeId,
    style_id: Option<&str>,
) -> Result<Vec<Patch>, OperationResult> {
    let prefix = word_prefix(source, paragraph)?;
    let name = |local: &str| qualify(prefix, local);
    let ppr = source
        .children(paragraph)
        .find(|id| word(source, *id, "pPr"));
    match (ppr, style_id) {
        (Some(ppr), Some(style)) => {
            let replacement = format!(
                "<{} {}val=\"{}\"/>",
                name("pStyle"),
                attr_prefix(prefix),
                escape(style)
            );
            if let Some(existing) = source.children(ppr).find(|id| word(source, *id, "pStyle")) {
                Ok(vec![Patch {
                    span: source.node(existing).expect("node").span(),
                    replacement: replacement.into_bytes(),
                }])
            } else {
                Ok(vec![ppr_insertion_at_start(
                    source,
                    ppr,
                    replacement.into_bytes(),
                )?])
            }
        }
        (Some(ppr), None) => Ok(source
            .children(ppr)
            .find(|id| word(source, *id, "pStyle"))
            .map(|id| Patch {
                span: source.node(id).expect("node").span(),
                replacement: Vec::new(),
            })
            .into_iter()
            .collect()),
        (None, Some(style)) => {
            let at = source
                .children(paragraph)
                .find_map(|id| {
                    source
                        .node(id)
                        .filter(|node| matches!(node.kind(), SourceNodeKind::Element { .. }))
                        .map(|node| node.span().start)
                })
                .ok_or_else(|| unsupported("paragraph has no insertion boundary"))?;
            Ok(vec![Patch {
                span: SourceSpan { start: at, end: at },
                replacement: format!(
                    "<{}><{} {}val=\"{}\"/></{}>",
                    name("pPr"),
                    name("pStyle"),
                    attr_prefix(prefix),
                    escape(style),
                    name("pPr")
                )
                .into_bytes(),
            }])
        }
        (None, None) => Ok(Vec::new()),
    }
}

fn ppr_insertion_at_start(
    source: &SourceDocument,
    ppr: NodeId,
    replacement: Vec<u8>,
) -> Result<Patch, OperationResult> {
    if let Some(first) = source.children(ppr).next() {
        let at = source.node(first).expect("node").span().start;
        return Ok(Patch {
            span: SourceSpan { start: at, end: at },
            replacement,
        });
    }
    ppr_insertion(source, ppr, replacement)
}

/// Sets direct formatting on one complete, ordinary visible text run.
pub fn set_text_formatting(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetTextFormatting,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let (text, runs) = match resolve_formatting_runs(source, &operation.target) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let patches = match range_text_formatting_patches(source, &runs, &operation.formatting) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let patched = match apply_patches(source, patches) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let temporary = temporary_path(output);
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) =
        verify_text_formatting_output(&temporary, &operation.target, &text, &operation.formatting)
    {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::text_formatting_set(text)
}

type FormattingRangeRun = (NodeId, usize, usize, String);

fn resolve_formatting_runs(
    source: &SourceDocument,
    target: &TextTarget,
) -> Result<(String, Vec<FormattingRangeRun>), OperationResult> {
    let matches = crate::text_search::resolve_text(source, &target.text)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    let matched = if matches.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "text target was not found",
        ));
    } else if let Some(occurrence) = target.occurrence {
        matches.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "text target occurrence was not found")
        })?
    } else if matches.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "text target matches more than one current semantic range",
        ));
    } else {
        matches.into_iter().next().expect("one match")
    };
    let mut runs = Vec::new();
    let mut paragraph = None;
    for segment in &matched.segments {
        if segment.inside_tracked_change
            || source.children(segment.id).nth(1).is_some()
            || is_cdata(source, segment.id)
        {
            return Err(unsupported(
                "set_text_formatting requires ordinary visible text source regions",
            ));
        }
        let run = ordinary_run(source, segment.id)
            .ok_or_else(|| unsupported("set_text_formatting does not edit inline wrappers"))?;
        let parent = source
            .node(run)
            .and_then(|node| node.parent())
            .ok_or_else(|| unsupported("run has no paragraph"))?;
        if paragraph
            .replace(parent)
            .is_some_and(|value| value != parent)
            || source
                .children(run)
                .any(|child| !word(source, child, "rPr") && !word(source, child, "t"))
        {
            return Err(unsupported(
                "set_text_formatting requires simple direct runs in one paragraph",
            ));
        }
        let start = matched.start.max(segment.start) - segment.start;
        let end = matched.end.min(segment.end) - segment.start;
        runs.push((run, start, end, segment.source_text.clone()));
    }
    let paragraph = paragraph.ok_or_else(|| unsupported("text range has no paragraph"))?;
    if !safe_body_paragraph(source, paragraph)
        || source.children(paragraph).any(|child| {
            word(source, child, "bookmarkStart")
                || word(source, child, "bookmarkEnd")
                || word(source, child, "commentRangeStart")
                || word(source, child, "commentRangeEnd")
                || word(source, child, "commentReference")
        })
    {
        return Err(unsupported(
            "set_text_formatting supports only an ordinary direct body run without ranges or wrappers",
        ));
    }
    Ok((matched.text, runs))
}

fn range_text_formatting_patches(
    source: &SourceDocument,
    runs: &[FormattingRangeRun],
    patch: &TextFormattingPatch,
) -> Result<Vec<Patch>, OperationResult> {
    let mut patches = Vec::new();
    for (run, start, end, text) in runs {
        if *start == 0 && *end == text.len() {
            patches.extend(text_formatting_patches(source, *run, patch)?);
        } else {
            patches.push(Patch {
                span: source.node(*run).expect("run").span(),
                replacement: split_formatted_run(source, *run, text, *start, *end, patch)?,
            });
        }
    }
    Ok(patches)
}

fn split_formatted_run(
    source: &SourceDocument,
    run: NodeId,
    text: &str,
    start: usize,
    end: usize,
    patch: &TextFormattingPatch,
) -> Result<Vec<u8>, OperationResult> {
    let paragraph = source
        .node(run)
        .and_then(|node| node.parent())
        .expect("run parent");
    let prefix = word_prefix(source, paragraph)?;
    let name = |local: &str| qualify(prefix, local);
    let start_tag = match source.node(run).expect("run").kind() {
        SourceNodeKind::Element { start_tag, .. } => *start_tag,
        _ => return Err(unsupported("run has no source tag")),
    };
    let open = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
        .map_err(|_| unsupported("run tag is not UTF-8"))?;
    let rpr = source
        .children(run)
        .find(|id| word(source, *id, "rPr"))
        .map(|id| {
            let span = source.node(id).expect("rpr").span();
            String::from_utf8_lossy(&source.original_bytes()[span.start..span.end]).into_owned()
        })
        .unwrap_or_default();
    let fragment = |value: &str, formatted: bool| -> Result<String, OperationResult> {
        let space = if requires_space_preservation(value) {
            " xml:space=\"preserve\""
        } else {
            ""
        };
        let raw = format!(
            "{open}{rpr}<{}{}>{}</{}></{}>",
            name("t"),
            space,
            escape(value),
            name("t"),
            name("r")
        );
        if !formatted {
            return Ok(raw);
        }
        let xml = format!(
            "<{} xmlns:{}=\"{}\"><{}><{}>{}</{}></{}></{}>",
            name("document"),
            prefix,
            NS[0],
            name("body"),
            name("p"),
            raw,
            name("p"),
            name("body"),
            name("document")
        );
        let doc = SourceDocument::parse(xml.into_bytes()).map_err(document_invalid)?;
        let p = doc
            .children(doc.root())
            .find(|id| word(&doc, *id, "body"))
            .and_then(|body| doc.children(body).find(|id| word(&doc, *id, "p")))
            .and_then(|paragraph| doc.children(paragraph).find(|id| word(&doc, *id, "r")))
            .ok_or_else(|| unsupported("formatted fragment is invalid"))?;
        let data = apply_patches(&doc, text_formatting_patches(&doc, p, patch)?)?;
        let updated = SourceDocument::parse(data).map_err(document_invalid)?;
        let run = updated
            .children(updated.root())
            .find(|id| word(&updated, *id, "body"))
            .and_then(|body| updated.children(body).find(|id| word(&updated, *id, "p")))
            .and_then(|paragraph| {
                updated
                    .children(paragraph)
                    .find(|id| word(&updated, *id, "r"))
            })
            .ok_or_else(|| unsupported("formatted fragment is invalid"))?;
        let span = updated.node(run).expect("run").span();
        Ok(
            String::from_utf8(updated.original_bytes()[span.start..span.end].to_vec())
                .expect("utf8"),
        )
    };
    let mut result = Vec::new();
    if start > 0 {
        result.extend(fragment(&text[..start], false)?.bytes());
    }
    result.extend(fragment(&text[start..end], true)?.bytes());
    if end < text.len() {
        result.extend(fragment(&text[end..], false)?.bytes());
    }
    Ok(result)
}

fn text_formatting_patches(
    source: &SourceDocument,
    run: NodeId,
    patch: &TextFormattingPatch,
) -> Result<Vec<Patch>, OperationResult> {
    let prefix = word_prefix(
        source,
        source
            .node(run)
            .and_then(|node| node.parent())
            .expect("run parent"),
    )?;
    let name = |local: &str| qualify(prefix, local);
    let rpr = source.children(run).find(|id| word(source, *id, "rPr"));
    if rpr.is_none() {
        let children = new_text_formatting_children(patch, &name, prefix);
        if children.is_empty() {
            return Ok(Vec::new());
        }
        let at = source
            .children(run)
            .find_map(|id| source.node(id).map(|node| node.span().start))
            .ok_or_else(|| unsupported("run has no formatting insertion boundary"))?;
        return Ok(vec![Patch {
            span: SourceSpan { start: at, end: at },
            replacement: format!("<{}>{}</{}>", name("rPr"), children, name("rPr")).into_bytes(),
        }]);
    }
    let rpr = rpr.expect("checked");
    let mut patches = Vec::new();
    text_boolean_property(
        source,
        rpr,
        "b",
        patch.bold.as_ref(),
        prefix,
        2,
        &mut patches,
    )?;
    text_boolean_property(
        source,
        rpr,
        "i",
        patch.italic.as_ref(),
        prefix,
        3,
        &mut patches,
    )?;
    text_simple_property(
        source,
        rpr,
        "sz",
        patch
            .font_size_half_points
            .as_ref()
            .map(|value| match value {
                PropertyPatch::Set(value) => {
                    format!("<{} {}val=\"{}\"/>", name("sz"), attr_prefix(prefix), value)
                }
                PropertyPatch::Clear => String::new(),
            }),
        4,
        &mut patches,
    )?;
    font_property(
        source,
        rpr,
        patch.font_family.as_ref(),
        &name,
        prefix,
        &mut patches,
    )?;
    Ok(patches)
}

fn new_text_formatting_children(
    patch: &TextFormattingPatch,
    name: &impl Fn(&str) -> String,
    prefix: &str,
) -> String {
    let mut result = String::new();
    if let Some(PropertyPatch::Set(value)) = &patch.font_family {
        result.push_str(&font_xml(name, prefix, value));
    }
    if let Some(PropertyPatch::Set(value)) = &patch.bold {
        result.push_str(&text_boolean_xml(name, "b", *value, prefix));
    }
    if let Some(PropertyPatch::Set(value)) = &patch.italic {
        result.push_str(&text_boolean_xml(name, "i", *value, prefix));
    }
    if let Some(PropertyPatch::Set(value)) = &patch.font_size_half_points {
        result.push_str(&format!(
            "<{} {}val=\"{}\"/>",
            name("sz"),
            attr_prefix(prefix),
            value
        ));
    }
    result
}

fn text_boolean_xml(
    name: &impl Fn(&str) -> String,
    local: &str,
    value: bool,
    prefix: &str,
) -> String {
    if value {
        format!("<{} />", name(local))
    } else {
        format!("<{} {}val=\"0\"/>", name(local), attr_prefix(prefix))
    }
}
fn text_boolean_property(
    source: &SourceDocument,
    rpr: NodeId,
    local: &str,
    value: Option<&PropertyPatch<bool>>,
    prefix: &str,
    rank: usize,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    let name = |local: &str| qualify(prefix, local);
    text_simple_property(
        source,
        rpr,
        local,
        value.map(|value| match value {
            PropertyPatch::Set(value) => text_boolean_xml(&name, local, *value, prefix),
            PropertyPatch::Clear => String::new(),
        }),
        rank,
        patches,
    )
}
fn text_simple_property(
    source: &SourceDocument,
    rpr: NodeId,
    local: &str,
    replacement: Option<String>,
    rank: usize,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    match (
        source.children(rpr).find(|id| word(source, *id, local)),
        replacement,
    ) {
        (Some(id), Some(value)) => patches.push(Patch {
            span: source.node(id).expect("node").span(),
            replacement: value.into_bytes(),
        }),
        (None, Some(value)) if !value.is_empty() => {
            patches.push(rpr_insertion(source, rpr, value.into_bytes(), rank)?)
        }
        _ => {}
    }
    Ok(())
}
fn font_xml(name: &impl Fn(&str) -> String, prefix: &str, value: &str) -> String {
    format!(
        "<{} {}ascii=\"{}\" {}hAnsi=\"{}\"/>",
        name("rFonts"),
        attr_prefix(prefix),
        escape(value),
        attr_prefix(prefix),
        escape(value)
    )
}
fn font_property(
    source: &SourceDocument,
    rpr: NodeId,
    value: Option<&PropertyPatch<String>>,
    name: &impl Fn(&str) -> String,
    prefix: &str,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    let Some(value) = value else { return Ok(()) };
    if let Some(id) = source.children(rpr).find(|id| word(source, *id, "rFonts")) {
        let SourceNodeKind::Element { start_tag, .. } = source.node(id).expect("node").kind()
        else {
            return Err(unsupported("font properties have no source tag"));
        };
        let mut tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
            .map_err(|_| unsupported("font properties tag is not UTF-8"))?
            .to_owned();
        let prefix = attr_prefix(prefix);
        match value {
            PropertyPatch::Set(value) => {
                let value = escape(value).into_owned();
                tag = edit_attribute(tag, &format!("{prefix}ascii"), Some(&value));
                tag = edit_attribute(tag, &format!("{prefix}hAnsi"), Some(&value));
            }
            PropertyPatch::Clear => {
                tag = edit_attribute(tag, &format!("{prefix}ascii"), None);
                tag = edit_attribute(tag, &format!("{prefix}hAnsi"), None);
            }
        }
        patches.push(Patch {
            span: *start_tag,
            replacement: tag.into_bytes(),
        });
    } else if let PropertyPatch::Set(value) = value {
        patches.push(rpr_insertion(
            source,
            rpr,
            font_xml(name, prefix, value).into_bytes(),
            1,
        )?);
    }
    Ok(())
}
fn rpr_rank(source: &SourceDocument, id: NodeId) -> usize {
    if word(source, id, "rStyle") {
        0
    } else if word(source, id, "rFonts") {
        1
    } else if word(source, id, "b") {
        2
    } else if word(source, id, "i") {
        3
    } else if word(source, id, "sz") {
        4
    } else {
        100
    }
}
fn rpr_insertion(
    source: &SourceDocument,
    rpr: NodeId,
    replacement: Vec<u8>,
    rank: usize,
) -> Result<Patch, OperationResult> {
    let SourceNodeKind::Element {
        start_tag, end_tag, ..
    } = source.node(rpr).expect("node").kind()
    else {
        return Err(unsupported("run properties have no source tag"));
    };
    if let Some(next) = source.children(rpr).find(|id| rpr_rank(source, *id) > rank) {
        return Ok(Patch {
            span: SourceSpan {
                start: source.node(next).expect("node").span().start,
                end: source.node(next).expect("node").span().start,
            },
            replacement,
        });
    }
    if let Some(end) = end_tag {
        return Ok(Patch {
            span: SourceSpan {
                start: end.start,
                end: end.start,
            },
            replacement,
        });
    }
    let tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
        .map_err(|_| unsupported("run properties tag is not UTF-8"))?;
    let name = tag
        .trim_start_matches('<')
        .split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        .next()
        .ok_or_else(|| unsupported("run properties tag is invalid"))?;
    let offset = tag
        .rfind("/>")
        .ok_or_else(|| unsupported("run properties cannot receive insertion"))?;
    let mut value = b">".to_vec();
    value.extend(replacement);
    value.extend(format!("</{name}>").bytes());
    Ok(Patch {
        span: SourceSpan {
            start: start_tag.start + offset,
            end: start_tag.end,
        },
        replacement: value,
    })
}

fn formatting_patches(
    source: &SourceDocument,
    paragraph: NodeId,
    patch: &ParagraphFormattingPatch,
) -> Result<Vec<Patch>, OperationResult> {
    let prefix = word_prefix(source, paragraph)?;
    let name = |local: &str| qualify(prefix, local);
    let Some(ppr) = source
        .children(paragraph)
        .find(|id| word(source, *id, "pPr"))
    else {
        let children = new_formatting_children(patch, &name, prefix);
        if children.is_empty() {
            return Ok(Vec::new());
        }
        let at = source
            .children(paragraph)
            .find_map(|id| {
                source
                    .node(id)
                    .filter(|node| matches!(node.kind(), SourceNodeKind::Element { .. }))
                    .map(|node| node.span().start)
            })
            .ok_or_else(|| unsupported("paragraph has no insertion boundary"))?;
        return Ok(vec![Patch {
            span: SourceSpan { start: at, end: at },
            replacement: format!("<{}>{}</{}>", name("pPr"), children, name("pPr")).into_bytes(),
        }]);
    };
    let mut patches = Vec::new();
    simple_property(
        source,
        ppr,
        "jc",
        patch.alignment.as_ref().map(|value| match value {
            PropertyPatch::Set(value) => format!(
                "<{} {}val=\"{}\"/>",
                name("jc"),
                attr_prefix(prefix),
                alignment_value(*value)
            ),
            PropertyPatch::Clear => String::new(),
        }),
        &mut patches,
    )?;
    boolean_property(
        source,
        ppr,
        "keepNext",
        patch.keep_with_next.as_ref(),
        &name,
        prefix,
        &mut patches,
    )?;
    boolean_property(
        source,
        ppr,
        "keepLines",
        patch.keep_lines.as_ref(),
        &name,
        prefix,
        &mut patches,
    )?;
    compound_property(
        source,
        ppr,
        "spacing",
        spacing_changes(patch),
        &name,
        prefix,
        &mut patches,
    )?;
    compound_property(
        source,
        ppr,
        "ind",
        indent_changes(patch),
        &name,
        prefix,
        &mut patches,
    )?;
    Ok(patches)
}

type AttrChange = (&'static str, Option<String>);

fn qualify(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_owned()
    } else {
        format!("{prefix}:{local}")
    }
}
fn attr_prefix(prefix: &str) -> String {
    if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}:")
    }
}
fn alignment_value(value: opensuite_protocol::ParagraphAlignment) -> &'static str {
    match value {
        opensuite_protocol::ParagraphAlignment::Left => "left",
        opensuite_protocol::ParagraphAlignment::Center => "center",
        opensuite_protocol::ParagraphAlignment::Right => "right",
        opensuite_protocol::ParagraphAlignment::Both => "both",
        opensuite_protocol::ParagraphAlignment::Distribute => "distribute",
    }
}
fn scalar_change(value: Option<&PropertyPatch<i32>>) -> Option<Option<String>> {
    value.map(|value| match value {
        PropertyPatch::Set(value) => Some(value.to_string()),
        PropertyPatch::Clear => None,
    })
}
fn spacing_changes(patch: &ParagraphFormattingPatch) -> Vec<AttrChange> {
    let mut out = vec![];
    if let Some(value) = scalar_change(patch.spacing_before_twips.as_ref()) {
        out.push(("before", value));
    }
    if let Some(value) = scalar_change(patch.spacing_after_twips.as_ref()) {
        out.push(("after", value));
    }
    if let Some(value) = &patch.line_spacing {
        match value {
            PropertyPatch::Set(value) => {
                out.push(("line", Some(value.value.to_string())));
                out.push((
                    "lineRule",
                    value.rule.map(|rule| {
                        match rule {
                            opensuite_protocol::LineSpacingRule::Auto => "auto",
                            opensuite_protocol::LineSpacingRule::Exact => "exact",
                            opensuite_protocol::LineSpacingRule::AtLeast => "atLeast",
                        }
                        .to_owned()
                    }),
                ));
            }
            PropertyPatch::Clear => {
                out.push(("line", None));
                out.push(("lineRule", None));
            }
        }
    }
    out
}
fn indent_changes(patch: &ParagraphFormattingPatch) -> Vec<AttrChange> {
    [
        ("left", scalar_change(patch.left_indent_twips.as_ref())),
        ("right", scalar_change(patch.right_indent_twips.as_ref())),
        (
            "firstLine",
            scalar_change(patch.first_line_indent_twips.as_ref()),
        ),
        (
            "hanging",
            scalar_change(patch.hanging_indent_twips.as_ref()),
        ),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|value| (key, value)))
    .collect()
}
fn new_formatting_children(
    patch: &ParagraphFormattingPatch,
    name: &impl Fn(&str) -> String,
    prefix: &str,
) -> String {
    let mut out = String::new();
    if let Some(PropertyPatch::Set(value)) = &patch.keep_with_next {
        out.push_str(&boolean_xml(name, "keepNext", *value, prefix));
    }
    if let Some(PropertyPatch::Set(value)) = &patch.keep_lines {
        out.push_str(&boolean_xml(name, "keepLines", *value, prefix));
    }
    add_compound(&mut out, name, "spacing", spacing_changes(patch), prefix);
    add_compound(&mut out, name, "ind", indent_changes(patch), prefix);
    if let Some(PropertyPatch::Set(value)) = &patch.alignment {
        out.push_str(&format!(
            "<{} {}val=\"{}\"/>",
            name("jc"),
            attr_prefix(prefix),
            alignment_value(*value)
        ));
    }
    out
}
fn add_compound(
    out: &mut String,
    name: &impl Fn(&str) -> String,
    local: &str,
    changes: Vec<AttrChange>,
    prefix: &str,
) {
    let values = changes
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
        .collect::<Vec<_>>();
    if !values.is_empty() {
        out.push_str(&format!("<{}", name(local)));
        for (key, value) in values {
            out.push_str(&format!(" {}{}=\"{}\"", attr_prefix(prefix), key, value));
        }
        out.push_str("/>");
    }
}
fn simple_property(
    source: &SourceDocument,
    ppr: NodeId,
    local: &str,
    replacement: Option<String>,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    let existing = source.children(ppr).find(|id| word(source, *id, local));
    match (existing, replacement) {
        (Some(id), Some(value)) => patches.push(Patch {
            span: source.node(id).expect("node").span(),
            replacement: value.into_bytes(),
        }),
        (None, Some(value)) if !value.is_empty() => {
            patches.push(ppr_insertion(source, ppr, value.into_bytes())?)
        }
        _ => {}
    }
    Ok(())
}
fn boolean_xml(name: &impl Fn(&str) -> String, local: &str, value: bool, prefix: &str) -> String {
    if value {
        format!("<{} />", name(local))
    } else {
        format!("<{} {}val=\"0\"/>", name(local), attr_prefix(prefix))
    }
}
fn boolean_property(
    source: &SourceDocument,
    ppr: NodeId,
    local: &str,
    value: Option<&PropertyPatch<bool>>,
    name: &impl Fn(&str) -> String,
    prefix: &str,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    let replacement = value.map(|value| match value {
        PropertyPatch::Set(value) => boolean_xml(name, local, *value, prefix),
        PropertyPatch::Clear => String::new(),
    });
    simple_property(source, ppr, local, replacement, patches)
}
fn compound_property(
    source: &SourceDocument,
    ppr: NodeId,
    local: &str,
    changes: Vec<AttrChange>,
    name: &impl Fn(&str) -> String,
    prefix: &str,
    patches: &mut Vec<Patch>,
) -> Result<(), OperationResult> {
    if changes.is_empty() {
        return Ok(());
    }
    if let Some(id) = source.children(ppr).find(|id| word(source, *id, local)) {
        let SourceNodeKind::Element { start_tag, .. } = source.node(id).expect("node").kind()
        else {
            return Err(unsupported("formatting property has no source tag"));
        };
        let mut tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
            .map_err(|_| unsupported("formatting property tag is not UTF-8"))?
            .to_owned();
        for (key, value) in changes {
            tag = edit_attribute(
                tag,
                &format!("{}{}", attr_prefix(prefix), key),
                value.as_deref(),
            );
        }
        patches.push(Patch {
            span: *start_tag,
            replacement: tag.into_bytes(),
        });
    } else {
        let mut value = String::new();
        add_compound(&mut value, name, local, changes, prefix);
        if !value.is_empty() {
            patches.push(ppr_insertion(source, ppr, value.into_bytes())?);
        }
    }
    Ok(())
}
fn edit_attribute(mut tag: String, key: &str, value: Option<&str>) -> String {
    let needle = format!("{key}=\"");
    if let Some(start) = tag.find(&needle) {
        let end = start + needle.len() + tag[start + needle.len()..].find('"').unwrap_or(0) + 1;
        if let Some(value) = value {
            tag.replace_range(start..end, &format!("{key}=\"{value}\""));
        } else {
            let begin = tag[..start].rfind(char::is_whitespace).unwrap_or(start);
            tag.replace_range(begin..end, "");
        }
    } else if let Some(value) = value {
        let at = tag
            .rfind("/>")
            .or_else(|| tag.rfind('>'))
            .unwrap_or(tag.len());
        tag.insert_str(at, &format!(" {key}=\"{value}\""));
    }
    tag
}
fn ppr_insertion(
    source: &SourceDocument,
    ppr: NodeId,
    replacement: Vec<u8>,
) -> Result<Patch, OperationResult> {
    let SourceNodeKind::Element {
        start_tag, end_tag, ..
    } = source.node(ppr).expect("node").kind()
    else {
        return Err(unsupported("paragraph properties have no source tag"));
    };
    if let Some(end) = end_tag {
        return Ok(Patch {
            span: SourceSpan {
                start: end.start,
                end: end.start,
            },
            replacement,
        });
    }
    let tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
        .map_err(|_| unsupported("paragraph properties tag is not UTF-8"))?;
    let name = tag
        .trim_start_matches('<')
        .split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        .next()
        .ok_or_else(|| unsupported("paragraph properties tag is invalid"))?;
    let offset = tag
        .rfind("/>")
        .ok_or_else(|| unsupported("paragraph properties cannot receive insertion"))?;
    let mut value = b">".to_vec();
    value.extend(replacement);
    value.extend(format!("</{name}>").bytes());
    Ok(Patch {
        span: SourceSpan {
            start: start_tag.start + offset,
            end: start_tag.end,
        },
        replacement: value,
    })
}

struct ResolvedContentControl {
    control: NodeId,
    paragraph: NodeId,
    text: String,
}

fn resolve_content_control(
    source: &SourceDocument,
    target: &ContentControlTarget,
) -> Result<ResolvedContentControl, OperationResult> {
    if target.tag.is_none() && target.alias.is_none() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "content control target requires tag or alias",
        ));
    }
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    let mut controls = document
        .content_controls()
        .filter(|control| {
            let properties = control.properties();
            target
                .tag
                .as_ref()
                .is_none_or(|tag| properties.tag.as_ref() == Some(tag))
                && target
                    .alias
                    .as_ref()
                    .is_none_or(|alias| properties.alias.as_ref() == Some(alias))
        })
        .collect::<Vec<_>>();
    if controls.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "content control target was not found",
        ));
    }
    let control = if let Some(occurrence) = target.occurrence {
        if occurrence >= controls.len() {
            return Err(OperationResult::failed(
                "TARGET_NOT_FOUND",
                "content control target occurrence was not found",
            ));
        }
        controls.remove(occurrence)
    } else if controls.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "content control target matches more than one control",
        ));
    } else {
        controls.pop().expect("one control")
    };
    let properties = control.properties();
    if !matches!(
        properties.kind,
        crate::ContentControlKind::Text | crate::ContentControlKind::RichText
    ) || properties.data_binding.is_some()
        || properties.lock.is_some()
        || control.properties_id().is_some_and(|id| {
            source
                .children(id)
                .any(|child| word(source, child, "showingPlcHdr"))
        })
    {
        return Err(unsupported(
            "set_content_control_text supports only unlocked, unbound text controls without placeholder state",
        ));
    }
    let Some(content) = control.content_id() else {
        return Err(unsupported("content control has no editable content"));
    };
    let paragraphs = source
        .children(content)
        .filter(|id| word(source, *id, "p"))
        .collect::<Vec<_>>();
    if paragraphs.len() != 1
        || source.children(content).any(|id| {
            matches!(
                source.node(id).map(|node| node.kind()),
                Some(SourceNodeKind::Element { .. })
            ) && !word(source, id, "p")
        })
        || !safe_table_paragraph(source, paragraphs[0])
    {
        return Err(unsupported(
            "set_content_control_text requires one ordinary direct paragraph",
        ));
    }
    let text = crate::tracked_change::text_for_view(source, paragraphs[0], RevisionView::Current)
        .map_err(document_invalid)?;
    Ok(ResolvedContentControl {
        control: control.source_id(),
        paragraph: paragraphs[0],
        text,
    })
}

fn all_content_control_texts(
    source: &SourceDocument,
) -> Result<Vec<(NodeId, String)>, OperationResult> {
    crate::DocxDocument::new(source)
        .map_err(document_invalid)?
        .content_controls()
        .map(|control| {
            control
                .visible_text()
                .map(|text| (control.source_id(), text))
                .map_err(document_invalid)
        })
        .collect()
}

struct ResolvedTableCell {
    cell: NodeId,
    paragraph: NodeId,
    text: String,
    table_index: usize,
    row_index: usize,
    column_index: usize,
}

struct ResolvedTableRow {
    row: NodeId,
    table_index: usize,
    row_index: usize,
}

fn resolve_table_row(
    source: &SourceDocument,
    table_target: &TableTarget,
    after: &TableRowTarget,
    inserted_rows: &[Vec<String>],
) -> Result<ResolvedTableRow, OperationResult> {
    let (table_index, table, rows, _headers) = resolve_table(source, table_target)?;
    if !simple_table(source, table) || has_revision_wrapper(source, table) {
        return Err(unsupported(
            "insert_table_row supports only simple rectangular tables without merges, nesting, or revisions",
        ));
    }
    let width = rows[0].cells().count();
    if inserted_rows.iter().any(|cells| cells.len() != width) {
        return Err(OperationResult::failed(
            "PRECONDITION_FAILED",
            "inserted row cell count must match the table column count",
        ));
    }
    let mut matches = Vec::new();
    for (row_index, row) in rows.iter().enumerate().skip(1) {
        let cells = row.cells().collect::<Vec<_>>();
        if cells.first().is_some_and(|cell| {
            cell.text_for_view(RevisionView::Current).ok().as_deref()
                == Some(&after.first_cell_text)
        }) {
            matches.push((row_index, row.source_id()));
        }
    }
    let (row_index, row) = select_row(matches, after)?;
    if !safe_table_row(source, row) {
        return Err(unsupported(
            "insert_table_row requires an ordinary anchor row with one direct paragraph per cell",
        ));
    }
    Ok(ResolvedTableRow {
        row,
        table_index,
        row_index,
    })
}

fn resolve_table<'a>(
    source: &'a SourceDocument,
    target: &TableTarget,
) -> Result<(usize, NodeId, Vec<crate::Row<'a>>, Vec<String>), OperationResult> {
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
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
        if headers == target.header_cells {
            tables.push((current_table_index, table.source_id(), rows, headers));
        }
    }
    select_table(tables, target)
}

fn select_table<'a>(
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

fn select_row(
    mut rows: Vec<(usize, NodeId)>,
    target: &TableRowTarget,
) -> Result<(usize, NodeId), OperationResult> {
    if rows.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "table row was not found",
        ));
    }
    if let Some(occurrence) = target.occurrence {
        return rows.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed(
                "TARGET_NOT_FOUND",
                "table row target occurrence was not found",
            )
        });
    }
    if rows.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "table row first-cell text matches more than one row",
        ));
    }
    Ok(rows.pop().expect("one row"))
}

fn safe_table_row(source: &SourceDocument, row: NodeId) -> bool {
    source
        .children(row)
        .all(|child| word(source, child, "trPr") || word(source, child, "tc"))
        && source
            .children(row)
            .filter(|child| word(source, *child, "tc"))
            .all(|cell| safe_template_cell(source, cell))
        && !source.node_ids().any(|id| {
            is_descendant(source, id, row)
                && (word(source, id, "fldChar")
                    || word(source, id, "fldSimple")
                    || word(source, id, "instrText")
                    || word(source, id, "sdt")
                    || word(source, id, "hyperlink")
                    || word(source, id, "drawing")
                    || word(source, id, "pict")
                    || word(source, id, "bookmarkStart")
                    || word(source, id, "bookmarkEnd")
                    || word(source, id, "commentRangeStart")
                    || word(source, id, "commentRangeEnd")
                    || word(source, id, "commentReference")
                    || word(source, id, "rPrChange"))
        })
}

fn safe_template_cell(source: &SourceDocument, cell: NodeId) -> bool {
    let paragraphs = direct_cell_paragraphs(source, cell);
    paragraphs.len() == 1
        && source
            .children(cell)
            .all(|child| word(source, child, "tcPr") || word(source, child, "p"))
        && safe_table_paragraph(source, paragraphs[0])
        && source
            .children(paragraphs[0])
            .filter(|child| word(source, *child, "r"))
            .count()
            <= 1
        && source.children(paragraphs[0]).all(|child| {
            word(source, child, "pPr")
                || word(source, child, "r")
                || matches!(
                    source.node(child).map(|node| node.kind()),
                    Some(SourceNodeKind::Text)
                )
        })
}

fn table_row_fragment(
    source: &SourceDocument,
    row: NodeId,
    cells: &[String],
) -> Result<Vec<u8>, OperationResult> {
    let prefix = word_element_prefix(source, row, "tr")?;
    let name = |local: &str| qualify(prefix, local);
    let row_properties = source
        .children(row)
        .find(|child| word(source, *child, "trPr"))
        .filter(|properties| safe_row_properties(source, *properties))
        .map(|properties| source_bytes(source, properties))
        .transpose()?
        .unwrap_or_default();
    let templates = source
        .children(row)
        .filter(|child| word(source, *child, "tc"))
        .collect::<Vec<_>>();
    let mut fragment = format!(
        "<{}>{}",
        name("tr"),
        String::from_utf8_lossy(&row_properties)
    );
    for (template, value) in templates.into_iter().zip(cells) {
        fragment.push_str(&cell_fragment(source, template, value, prefix)?);
    }
    fragment.push_str(&format!("</{}>", name("tr")));
    Ok(fragment.into_bytes())
}

fn safe_row_properties(source: &SourceDocument, properties: NodeId) -> bool {
    source.children(properties).all(|child| {
        word(source, child, "trHeight")
            || word(source, child, "cantSplit")
            || word(source, child, "jc")
            || word(source, child, "tblCellSpacing")
            || word(source, child, "cnfStyle")
    })
}

fn cell_fragment(
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

fn source_bytes(source: &SourceDocument, id: NodeId) -> Result<Vec<u8>, OperationResult> {
    let span = source
        .node(id)
        .ok_or_else(|| unsupported("template source is unavailable"))?
        .span();
    Ok(source.original_bytes()[span.start..span.end].to_vec())
}

fn word_element_prefix<'a>(
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

fn all_table_rows(source: &SourceDocument) -> Result<Vec<Vec<Vec<String>>>, OperationResult> {
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

fn verify_table_row_output_bytes(
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

fn resolve_table_cell(
    source: &SourceDocument,
    target: &TableCellTarget,
) -> Result<ResolvedTableCell, OperationResult> {
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
            if header_cell
                .text_for_view(RevisionView::Current)
                .map_err(document_invalid)?
                != target.column_header
            {
                continue;
            }
            for row in rows.iter().skip(1) {
                let cells = row.cells().collect::<Vec<_>>();
                if cells.first().is_some_and(|cell| {
                    cell.text_for_view(RevisionView::Current).ok().as_deref()
                        == Some(&target.row_label)
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
    if paragraphs.len() != 1 || !safe_table_paragraph(source, paragraphs[0]) {
        return Err(unsupported(
            "set_table_cell_text requires one ordinary paragraph with direct runs",
        ));
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
    let columns = headers
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(index, header)| (header == &target.column_header).then_some(index))
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
            cell.text_for_view(RevisionView::Current).ok().as_deref() == Some(&target.row_label)
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
    if paragraphs.len() != 1 || !safe_table_paragraph(source, paragraphs[0]) {
        return Err(unsupported(
            "set_table_cells_text requires one ordinary paragraph with direct runs",
        ));
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

fn is_direct_body_table(source: &SourceDocument, table: NodeId) -> bool {
    source
        .node(table)
        .and_then(|node| node.parent())
        .is_some_and(|body| word(source, body, "body"))
}

fn simple_table(source: &SourceDocument, table: NodeId) -> bool {
    if source
        .node_ids()
        .any(|id| id != table && word(source, id, "tbl") && is_descendant(source, id, table))
        || source.node_ids().any(|id| {
            is_descendant(source, id, table)
                && (word(source, id, "gridSpan") || word(source, id, "vMerge"))
        })
    {
        return false;
    }
    let rows = source
        .children(table)
        .filter(|id| word(source, *id, "tr"))
        .collect::<Vec<_>>();
    let Some(first) = rows.first() else {
        return false;
    };
    let width = source
        .children(*first)
        .filter(|id| word(source, *id, "tc"))
        .count();
    width > 1
        && rows.len() > 1
        && rows.iter().all(|row| {
            source
                .children(*row)
                .filter(|id| word(source, *id, "tc"))
                .count()
                == width
        })
}

fn direct_cell_paragraphs(source: &SourceDocument, cell: NodeId) -> Vec<NodeId> {
    source
        .children(cell)
        .filter(|id| word(source, *id, "p"))
        .collect()
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

fn safe_table_paragraph(source: &SourceDocument, paragraph: NodeId) -> bool {
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

fn is_descendant(source: &SourceDocument, mut id: NodeId, ancestor: NodeId) -> bool {
    while let Some(parent) = source.node(id).and_then(|node| node.parent()) {
        if parent == ancestor {
            return true;
        }
        id = parent;
    }
    false
}

fn empty_cell_patch(
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

fn run_fragment(
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

fn qualified_name(
    source: &SourceDocument,
    paragraph: NodeId,
    local: &str,
) -> Result<String, OperationResult> {
    let prefix = word_prefix(source, paragraph)?;
    Ok(if prefix.is_empty() {
        local.to_owned()
    } else {
        format!("{prefix}:{local}")
    })
}

fn temporary_path(output: &Path) -> std::path::PathBuf {
    output.with_file_name(format!(
        ".opensuite-{}-{}.docx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

struct ResolvedText {
    value: String,
    patches: Vec<Patch>,
}

struct Patch {
    span: SourceSpan,
    replacement: Vec<u8>,
}

fn resolve_paragraph_anchor(
    source: &SourceDocument,
    target: &TextTarget,
) -> Result<(String, NodeId), OperationResult> {
    let matches = crate::text_search::resolve_text(source, &target.text)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    if matches.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "text target was not found",
        ));
    }
    let matched = if let Some(occurrence) = target.occurrence {
        matches.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "text target occurrence was not found")
        })?
    } else if matches.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "text target matches more than one current semantic range",
        ));
    } else {
        matches.into_iter().next().expect("one match")
    };
    let paragraph = matched
        .segments
        .first()
        .and_then(|segment| paragraph_ancestor(source, segment.id))
        .ok_or_else(|| unsupported("insert_paragraph_after requires a paragraph anchor"))?;
    if matched
        .segments
        .iter()
        .any(|segment| paragraph_ancestor(source, segment.id) != Some(paragraph))
        || !safe_body_paragraph(source, paragraph)
    {
        return Err(unsupported(
            "insert_paragraph_after supports only ordinary direct body paragraphs",
        ));
    }
    Ok((matched.text, paragraph))
}

fn paragraph_ancestor(source: &SourceDocument, mut id: NodeId) -> Option<NodeId> {
    loop {
        if word(source, id, "p") {
            return Some(id);
        }
        id = source.node(id)?.parent()?;
    }
}

fn safe_body_paragraph(source: &SourceDocument, paragraph: NodeId) -> bool {
    let Some(body) = source.node(paragraph).and_then(|node| node.parent()) else {
        return false;
    };
    word(source, body, "body")
        && source
            .node(body)
            .and_then(|node| node.parent())
            .is_some_and(|document| word(source, document, "document"))
        && !source.children(paragraph).any(|id| {
            word(source, id, "pPr")
                && source
                    .children(id)
                    .any(|child| word(source, child, "sectPr"))
        })
        && !has_revision_wrapper(source, paragraph)
}

fn safe_to_delete(source: &SourceDocument, paragraph: NodeId) -> bool {
    source
        .children(paragraph)
        .all(|child| safe_paragraph_child(source, child))
}

fn safe_paragraph_child(source: &SourceDocument, id: NodeId) -> bool {
    if word(source, id, "pPr") {
        return !source
            .children(id)
            .any(|child| word(source, child, "pPrChange"));
    }
    if word(source, id, "r") {
        return source
            .children(id)
            .all(|child| safe_run_child(source, child));
    }
    word(source, id, "hyperlink")
        && source
            .children(id)
            .all(|child| word(source, child, "r") && safe_paragraph_child(source, child))
}

fn safe_run_child(source: &SourceDocument, id: NodeId) -> bool {
    if word(source, id, "rPr") {
        return true;
    }
    word(source, id, "t")
        || word(source, id, "tab")
        || word(source, id, "br")
        || word(source, id, "cr")
        || word(source, id, "noBreakHyphen")
        || word(source, id, "softHyphen")
        || word(source, id, "lastRenderedPageBreak")
}

fn has_revision_wrapper(source: &SourceDocument, id: NodeId) -> bool {
    word(source, id, "ins")
        || word(source, id, "del")
        || word(source, id, "moveFrom")
        || word(source, id, "moveTo")
        || source
            .children(id)
            .any(|child| has_revision_wrapper(source, child))
}

fn body_texts(source: &SourceDocument) -> Result<Vec<(NodeId, String)>, OperationResult> {
    crate::DocxDocument::new(source)
        .map_err(document_invalid)?
        .blocks()
        .map(|block| match block {
            crate::BodyBlock::Paragraph(paragraph) => paragraph
                .text_for_view(RevisionView::Current)
                .map(|text| (paragraph.source_id(), text))
                .map_err(document_invalid),
            crate::BodyBlock::Table(table) => {
                let source_id = table.source_id();
                table_current_text(table)
                    .map(|text| (source_id, text))
                    .map_err(document_invalid)
            }
        })
        .collect()
}

fn paragraph_fragment(
    source: &SourceDocument,
    paragraph: NodeId,
    text: &str,
) -> Result<Vec<u8>, OperationResult> {
    let prefix = word_prefix(source, paragraph)?;
    let name = |local: &str| {
        if prefix.is_empty() {
            local.to_owned()
        } else {
            format!("{prefix}:{local}")
        }
    };
    if text.is_empty() {
        return Ok(format!("<{}></{}>", name("p"), name("p")).into_bytes());
    }
    let space = if requires_space_preservation(text) {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    Ok(format!(
        "<{}><{}><{}{}>{}</{}></{}></{}>",
        name("p"),
        name("r"),
        name("t"),
        space,
        escape(text),
        name("t"),
        name("r"),
        name("p")
    )
    .into_bytes())
}

fn word_prefix(source: &SourceDocument, id: NodeId) -> Result<&str, OperationResult> {
    let SourceNodeKind::Element { start_tag, .. } =
        source.node(id).expect("source node exists").kind()
    else {
        return Err(unsupported("anchor paragraph has no source tag"));
    };
    let tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
        .map_err(|_| unsupported("anchor paragraph tag is not UTF-8"))?;
    let name = tag
        .strip_prefix('<')
        .and_then(|value| {
            value
                .split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
                .next()
        })
        .ok_or_else(|| unsupported("anchor paragraph tag is invalid"))?;
    let prefix = name
        .strip_suffix("p")
        .and_then(|value| value.strip_suffix(':'))
        .unwrap_or("");
    if name != "p" && !name.ends_with(":p") {
        return Err(unsupported("anchor paragraph tag is invalid"));
    }
    Ok(prefix)
}

fn resolve_target(
    source: &SourceDocument,
    operation: &ReplaceText,
) -> Result<ResolvedText, OperationResult> {
    let matches = crate::text_search::resolve_text(source, &operation.target.text)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    if matches.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "text target was not found",
        ));
    }
    let matched = if let Some(occurrence) = operation.target.occurrence {
        matches.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "text target occurrence was not found")
        })?
    } else if matches.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "text target matches more than one current semantic range",
        ));
    } else {
        matches.into_iter().next().expect("one match")
    };
    let patches = patches_for_match(source, &matched, &operation.replacement)?;
    Ok(ResolvedText {
        value: matched.text,
        patches,
    })
}

fn patches_for_match(
    source: &SourceDocument,
    matched: &crate::text_search::ResolvedTextMatch,
    replacement: &str,
) -> Result<Vec<Patch>, OperationResult> {
    let mut runs = Vec::new();
    for segment in &matched.segments {
        if segment.inside_tracked_change {
            return Err(unsupported(
                "replace_text does not edit tracked revision text",
            ));
        }
        let run = ordinary_run(source, segment.id)
            .ok_or_else(|| unsupported("replace_text does not cross inline wrapper boundaries"))?;
        let child = text_child(source, segment.id)
            .filter(|child| !is_cdata(source, *child))
            .ok_or_else(|| unsupported("replace_text requires replaceable text source regions"))?;
        if source.children(segment.id).nth(1).is_some() {
            return Err(unsupported(
                "replace_text requires one source region per text node",
            ));
        }
        runs.push((segment, run, child));
    }
    let formatting = run_properties(source, runs[0].1);
    if runs
        .iter()
        .any(|(_, run, _)| run_properties(source, *run) != formatting)
    {
        return Err(unsupported(
            "replace_text spans incompatible run formatting regions",
        ));
    }

    let mut remaining = replacement.chars();
    let mut patches = Vec::with_capacity(runs.len() * 2);
    for (index, (segment, _, child)) in runs.into_iter().enumerate() {
        let start = matched.start.max(segment.start) - segment.start;
        let end = matched.end.min(segment.end) - segment.start;
        let prefix = &segment.source_text[..start];
        let suffix = &segment.source_text[end..];
        let matched_chars = segment.source_text[start..end].chars().count();
        let distributed: String = if index + 1 == matched.segments.len() {
            remaining.by_ref().collect()
        } else {
            remaining.by_ref().take(matched_chars).collect()
        };
        let text = format!("{prefix}{distributed}{suffix}");
        let span = source.node(child).expect("source child exists").span();
        patches.push(Patch {
            span,
            replacement: escape(&text).into_owned().into_bytes(),
        });
        if requires_space_preservation(&text) && !has_xml_space(source, segment.id) {
            patches.push(Patch {
                span: start_tag_end(source, segment.id)?,
                replacement: b" xml:space=\"preserve\"".to_vec(),
            });
        }
    }
    Ok(patches)
}

fn apply_patches(
    source: &SourceDocument,
    mut patches: Vec<Patch>,
) -> Result<Vec<u8>, OperationResult> {
    patches.sort_by_key(|patch| (patch.span.start, patch.span.end));
    if patches
        .windows(2)
        .any(|pair| pair[0].span.end > pair[1].span.start)
    {
        return Err(unsupported(
            "replace_text generated overlapping source patches",
        ));
    }
    let mut result = source.original_bytes().to_vec();
    for patch in patches.into_iter().rev() {
        result.splice(patch.span.start..patch.span.end, patch.replacement);
    }
    Ok(result)
}

fn ordinary_run(source: &SourceDocument, text: NodeId) -> Option<NodeId> {
    let run = source.node(text)?.parent()?;
    (word(source, run, "r")
        && source
            .node(run)?
            .parent()
            .is_some_and(|parent| word(source, parent, "p")))
    .then_some(run)
}

fn run_properties(source: &SourceDocument, run: NodeId) -> Vec<u8> {
    source
        .children(run)
        .find(|child| word(source, *child, "rPr"))
        .map_or_else(Vec::new, |properties| {
            let span = source.node(properties).expect("source node exists").span();
            source.original_bytes()[span.start..span.end].to_vec()
        })
}

fn requires_space_preservation(text: &str) -> bool {
    text.starts_with(char::is_whitespace) || text.ends_with(char::is_whitespace)
}

fn has_xml_space(source: &SourceDocument, text: NodeId) -> bool {
    let SourceNodeKind::Element { start_tag, .. } =
        source.node(text).expect("source node exists").kind()
    else {
        return false;
    };
    source.original_bytes()[start_tag.start..start_tag.end]
        .windows(b"xml:space".len())
        .any(|window| window == b"xml:space")
}

fn start_tag_end(source: &SourceDocument, text: NodeId) -> Result<SourceSpan, OperationResult> {
    let SourceNodeKind::Element { start_tag, .. } =
        source.node(text).expect("source node exists").kind()
    else {
        return Err(unsupported("replace_text text node has no start tag"));
    };
    let end = start_tag
        .end
        .checked_sub(1)
        .ok_or_else(|| unsupported("replace_text text tag is invalid"))?;
    Ok(SourceSpan { start: end, end })
}

fn unsupported(message: impl Into<String>) -> OperationResult {
    OperationResult::failed("UNSUPPORTED_OPERATION", message)
}

fn output_matches_input(package: &Package, output: &Path) -> bool {
    package.source_path().is_some_and(|input| {
        output == input
            || output.exists() && output.canonicalize().ok() == input.canonicalize().ok()
    })
}

fn verify_output_bytes(output: &[u8], replacement: &str) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let document = crate::DocxDocument::new(&source).map_err(document_invalid)?;
    let text = document
        .blocks()
        .map(|block| match block {
            crate::BodyBlock::Paragraph(paragraph) => {
                paragraph.text_for_view(RevisionView::Current)
            }
            crate::BodyBlock::Table(table) => table_current_text(table),
        })
        .collect::<Result<String, SemanticError>>()
        .map_err(document_invalid)?;
    text.contains(replacement).then_some(()).ok_or_else(|| {
        OperationResult::failed(
            "DOCUMENT_INVALID",
            format!(
                "output package {0} does not contain the replacement",
                main.name
            ),
        )
    })
}

fn verify_output(output: &Path, replacement: &str) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let document = crate::DocxDocument::new(&source).map_err(document_invalid)?;
    if !document
        .blocks()
        .map(|block| match block {
            crate::BodyBlock::Paragraph(paragraph) => {
                paragraph.text_for_view(RevisionView::Current)
            }
            crate::BodyBlock::Table(table) => table_current_text(table),
        })
        .collect::<Result<String, SemanticError>>()
        .map_err(document_invalid)?
        .contains(replacement)
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            format!(
                "output package {0} does not contain the replacement",
                main.name
            ),
        ));
    }
    Ok(())
}

fn verify_inserted_output(
    output: &Path,
    target: &TextTarget,
    anchor_text: &str,
    inserted_text: &str,
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (resolved_anchor, anchor) = resolve_paragraph_anchor(&source, target)?;
    if resolved_anchor != anchor_text {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output anchor text changed",
        ));
    }
    let document = crate::DocxDocument::new(&source).map_err(document_invalid)?;
    let mut blocks = document.blocks();
    while let Some(block) = blocks.next() {
        if matches!(block, crate::BodyBlock::Paragraph(ref paragraph) if paragraph.source_id() == anchor)
        {
            let Some(crate::BodyBlock::Paragraph(inserted)) = blocks.next() else {
                return Err(OperationResult::failed(
                    "DOCUMENT_INVALID",
                    "inserted paragraph is not immediately after anchor",
                ));
            };
            return (inserted
                .text_for_view(RevisionView::Current)
                .map_err(document_invalid)?
                == inserted_text)
                .then_some(())
                .ok_or_else(|| {
                    OperationResult::failed(
                        "DOCUMENT_INVALID",
                        "inserted paragraph text does not match request",
                    )
                });
        }
    }
    Err(OperationResult::failed(
        "DOCUMENT_INVALID",
        "output anchor paragraph was not found",
    ))
}

fn verify_deleted_output(output: &Path, expected_body: &[String]) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let actual = body_texts(&source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    (actual == expected_body).then_some(()).ok_or_else(|| {
        OperationResult::failed(
            "DOCUMENT_INVALID",
            "output body does not preserve expected neighboring structure",
        )
    })
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

fn verify_content_control_output(
    output: &Path,
    target: &ContentControlTarget,
    replacement: &str,
    expected_controls: &[String],
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    if resolve_content_control(&source, target)?.text != replacement {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output content control text does not match replacement",
        ));
    }
    let actual = all_content_control_texts(&source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    (actual == expected_controls).then_some(()).ok_or_else(|| {
        OperationResult::failed(
            "DOCUMENT_INVALID",
            "output content controls do not preserve expected semantic structure",
        )
    })
}

fn verify_formatting_output(
    output: &Path,
    target: &TextTarget,
    text: &str,
    patch: &ParagraphFormattingPatch,
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (resolved, paragraph) = resolve_paragraph_anchor(&source, target)?;
    if resolved != text {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output target paragraph text changed",
        ));
    }
    let direct = source
        .children(paragraph)
        .find(|id| word(&source, *id, "pPr"))
        .map(|ppr| crate::styles::paragraph_formatting(&source, ppr))
        .transpose()
        .map_err(document_invalid)?
        .unwrap_or_default();
    formatting_matches(&direct, patch)
        .then_some(())
        .ok_or_else(|| {
            OperationResult::failed(
                "DOCUMENT_INVALID",
                "output paragraph formatting does not match request",
            )
        })
}

fn verify_paragraph_style_output(
    output: &Path,
    target: &TextTarget,
    text: &str,
    style: Option<&(String, String)>,
    expected_formatting: &crate::styles::ParagraphFormatting,
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (resolved, paragraph) = resolve_paragraph_anchor(&source, target)?;
    if resolved != text {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output target paragraph text changed",
        ));
    }
    let ppr = source
        .children(paragraph)
        .find(|id| word(&source, *id, "pPr"));
    let direct = ppr
        .map(|id| crate::styles::paragraph_formatting(&source, id))
        .transpose()
        .map_err(document_invalid)?
        .unwrap_or_default();
    if &direct != expected_formatting {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output direct paragraph formatting changed",
        ));
    }
    let direct_id = ppr
        .and_then(|ppr| source.children(ppr).find(|id| word(&source, *id, "pStyle")))
        .and_then(|id| source.node(id))
        .and_then(|node| node.attribute("val"));
    match style {
        Some((id, name)) => {
            if direct_id != Some(id.as_str()) {
                return Err(OperationResult::failed(
                    "DOCUMENT_INVALID",
                    "output direct paragraph style does not match request",
                ));
            }
            let styles = crate::load_styles(&package, &main)
                .map_err(document_invalid)?
                .ok_or_else(|| {
                    OperationResult::failed("DOCUMENT_INVALID", "output stylesheet is unavailable")
                })?;
            if styles
                .styles()
                .find(|style| style.id().as_str() == id)
                .and_then(crate::Style::name)
                != Some(name.as_str())
            {
                return Err(OperationResult::failed(
                    "DOCUMENT_INVALID",
                    "output paragraph style name does not match request",
                ));
            }
        }
        None if direct_id.is_some() => {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "output direct paragraph style was not cleared",
            ));
        }
        None => {}
    }
    Ok(())
}

fn verify_text_formatting_output(
    output: &Path,
    target: &TextTarget,
    text: &str,
    patch: &TextFormattingPatch,
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (resolved, runs) = resolve_formatting_runs(&source, target)?;
    if resolved != text {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output target text changed",
        ));
    }
    let direct = source
        .children(runs[0].0)
        .find(|id| word(&source, *id, "rPr"))
        .map(|rpr| crate::styles::run_formatting(&source, rpr))
        .transpose()
        .map_err(document_invalid)?
        .unwrap_or_default();
    text_formatting_matches(&direct, patch)
        .then_some(())
        .ok_or_else(|| {
            OperationResult::failed(
                "DOCUMENT_INVALID",
                "output direct run formatting does not match request",
            )
        })
}

fn text_formatting_matches(
    actual: &crate::styles::RunFormatting,
    patch: &TextFormattingPatch,
) -> bool {
    matches_scalar(&actual.bold, patch.bold.as_ref(), |value| value)
        && matches_scalar(&actual.italic, patch.italic.as_ref(), |value| value)
        && matches_scalar(
            &actual.font_size_half_points,
            patch.font_size_half_points.as_ref(),
            |value| value,
        )
        && match patch.font_family.as_ref() {
            None => true,
            Some(PropertyPatch::Clear) => actual.font_family.is_none(),
            Some(PropertyPatch::Set(value)) => actual.font_family.as_ref() == Some(value),
        }
}

fn formatting_matches(
    actual: &crate::styles::ParagraphFormatting,
    patch: &ParagraphFormattingPatch,
) -> bool {
    matches_scalar(
        &actual.alignment,
        patch.alignment.as_ref(),
        |value| match value {
            opensuite_protocol::ParagraphAlignment::Left => crate::styles::ParagraphAlignment::Left,
            opensuite_protocol::ParagraphAlignment::Center => {
                crate::styles::ParagraphAlignment::Center
            }
            opensuite_protocol::ParagraphAlignment::Right => {
                crate::styles::ParagraphAlignment::Right
            }
            opensuite_protocol::ParagraphAlignment::Both => crate::styles::ParagraphAlignment::Both,
            opensuite_protocol::ParagraphAlignment::Distribute => {
                crate::styles::ParagraphAlignment::Distribute
            }
        },
    ) && matches_scalar(
        &actual.spacing_before_twips,
        patch.spacing_before_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.spacing_after_twips,
        patch.spacing_after_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.left_indent_twips,
        patch.left_indent_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.right_indent_twips,
        patch.right_indent_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.first_line_indent_twips,
        patch.first_line_indent_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.hanging_indent_twips,
        patch.hanging_indent_twips.as_ref(),
        |value| value,
    ) && matches_scalar(
        &actual.keep_with_next,
        patch.keep_with_next.as_ref(),
        |value| value,
    ) && matches_scalar(&actual.keep_lines, patch.keep_lines.as_ref(), |value| value)
        && matches_line(&actual.line_spacing, patch.line_spacing.as_ref())
}

fn matches_scalar<T: PartialEq, U>(
    actual: &Option<T>,
    patch: Option<&PropertyPatch<U>>,
    map: impl Fn(U) -> T,
) -> bool
where
    U: Copy,
{
    match patch {
        None => true,
        Some(PropertyPatch::Clear) => actual.is_none(),
        Some(PropertyPatch::Set(value)) => actual.as_ref() == Some(&map(*value)),
    }
}

fn matches_line(
    actual: &Option<crate::styles::LineSpacing>,
    patch: Option<&PropertyPatch<opensuite_protocol::LineSpacing>>,
) -> bool {
    match patch {
        None => true,
        Some(PropertyPatch::Clear) => actual.is_none(),
        Some(PropertyPatch::Set(value)) => actual.as_ref().is_some_and(|actual| {
            actual.value == value.value
                && actual.rule
                    == value.rule.map(|rule| match rule {
                        opensuite_protocol::LineSpacingRule::Auto => {
                            crate::styles::LineSpacingRule::Auto
                        }
                        opensuite_protocol::LineSpacingRule::Exact => {
                            crate::styles::LineSpacingRule::Exact
                        }
                        opensuite_protocol::LineSpacingRule::AtLeast => {
                            crate::styles::LineSpacingRule::AtLeast
                        }
                    })
        }),
    }
}

fn table_current_text(table: crate::Table<'_>) -> Result<String, SemanticError> {
    let mut text = String::new();
    for row in table.rows() {
        for cell in row.cells() {
            text.push_str(&cell.text_for_view(RevisionView::Current)?);
        }
    }
    Ok(text)
}

fn document_invalid(error: impl std::fmt::Display) -> OperationResult {
    OperationResult::failed("DOCUMENT_INVALID", error.to_string())
}

fn text_child(source: &SourceDocument, id: NodeId) -> Option<NodeId> {
    source.children(id).next()
}

fn is_cdata(source: &SourceDocument, id: NodeId) -> bool {
    let span = source.node(id).expect("source node exists").span();
    source.original_bytes()[..span.start].ends_with(b"<![CDATA[")
}

fn word(source: &SourceDocument, id: NodeId, local_name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name, .. }) if name.local_name() == local_name && name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fmt::Write as _,
        fs,
        io::{Read, Write},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use opensuite_protocol::{
        ContentControlTarget, DeleteParagraph, InsertParagraphAfter, InsertTableRowAfter,
        InsertTableRowsAfter, ParagraphAlignment, ParagraphFormattingPatch, PropertyPatch,
        ReplaceText, SetContentControlText, SetParagraphFormatting, SetParagraphStyle,
        SetTableCellText, SetTableCellsText, SetTextFormatting, TableCellTarget,
        TableCellTextUpdate, TableRowTarget, TableTarget, TextFormattingPatch, TextTarget,
    };
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    const OFFICE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
    static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

    fn path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "opensuite-mutation-{name}-{}-{}.docx",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn valid_picture_fixture() -> std::path::PathBuf {
        let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/images.docx");
        let output = path("picture-input");
        let mut input = ZipArchive::new(fs::File::open(source).unwrap()).unwrap();
        let mut writer = ZipWriter::new(fs::File::create(&output).unwrap());
        for index in 0..input.len() {
            let mut entry = input.by_index(index).unwrap();
            let name = entry.name().to_owned();
            if entry.is_dir() {
                writer
                    .add_directory(name, SimpleFileOptions::default())
                    .unwrap();
            } else {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                writer
                    .start_file(name, SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(&bytes).unwrap();
            }
        }
        writer
            .start_file("media/missing.png", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"missing fixture image").unwrap();
        writer.finish().unwrap();
        output
    }

    fn payloads(path: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
        let mut archive = ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        (0..archive.len())
            .filter_map(|index| {
                let mut entry = archive.by_index(index).unwrap();
                (!entry.is_dir()).then(|| {
                    let mut bytes = Vec::new();
                    entry.read_to_end(&mut bytes).unwrap();
                    (entry.name().to_owned(), bytes)
                })
            })
            .collect()
    }

    fn picture_fixture(pictures: &[(&str, &str, &str)]) -> std::path::PathBuf {
        let output = path("picture-targets");
        let mut writer = ZipWriter::new(fs::File::create(&output).unwrap());
        let options = SimpleFileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer.write_all(b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/><Override PartName=\"/custom/main.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>").unwrap();
        writer.start_file("_rels/.rels", options).unwrap();
        writer.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"custom/main.xml\"/></Relationships>").as_bytes()).unwrap();
        writer.start_file("custom/main.xml", options).unwrap();
        let mut body = String::new();
        for (index, (name, relationship, _)) in pictures.iter().enumerate() {
            write!(body, "<w:p><w:r><w:drawing><wp:inline><wp:extent cx=\"1\" cy=\"2\"/><wp:docPr id=\"{}\" name=\"{}\"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed=\"{}\"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>", index + 1, name, relationship).unwrap();
        }
        writer.write_all(format!("<w:document xmlns:w=\"{WORD}\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><w:body>{body}</w:body></w:document>").as_bytes()).unwrap();
        writer
            .start_file("custom/_rels/main.xml.rels", options)
            .unwrap();
        let mut relationships = String::new();
        for (_, relationship, target) in pictures {
            write!(relationships, "<Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"{}\"/>", relationship, target).unwrap();
        }
        writer
            .write_all(format!("<Relationships>{relationships}</Relationships>").as_bytes())
            .unwrap();
        let mut written = std::collections::BTreeSet::new();
        for (_, _, target) in pictures {
            let name = target.trim_start_matches("../");
            if written.insert(name) {
                writer.start_file(name, options).unwrap();
                writer.write_all(b"original image").unwrap();
            }
        }
        writer.finish().unwrap();
        output
    }

    fn fixture() -> std::path::PathBuf {
        let path = path("input");
        let file = fs::File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(
            b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>",
        )
        .unwrap();
        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"word/document.xml\"/></Relationships>").as_bytes()).unwrap();
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(format!("<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>OLD UNIQUE TEXT</w:t></w:r></w:p><w:p><w:r><w:t>Duplicate</w:t></w:r><w:r><w:t>Duplicate</w:t></w:r></w:p><w:p><w:r><w:t>Cross </w:t></w:r><w:r><w:t>run</w:t></w:r></w:p><w:p><w:r><w:t>Styled </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>text</w:t></w:r></w:p><w:p><w:r><w:t>Linked </w:t></w:r><w:hyperlink><w:r><w:t>text</w:t></w:r></w:hyperlink></w:p><w:p><w:r><w:t>FY2026 Revenue </w:t></w:r><w:r><w:t>was $10M according</w:t></w:r></w:p><w:p><w:r><w:t xml:space=\"preserve\"> spaced </w:t></w:r></w:p><w:p><w:del><w:r><w:delText>OLD DELETED</w:delText></w:r></w:del><w:ins><w:r><w:t>NEW INSERTED</w:t></w:r></w:ins></w:p></w:body></w:document>").as_bytes()).unwrap();
        zip.start_file("word/media/image.bin", options).unwrap();
        zip.write_all(&[1, 2, 3]).unwrap();
        zip.finish().unwrap();
        path
    }

    fn table_fixture(document: &str) -> std::path::PathBuf {
        let path = path("table-input");
        let file = fs::File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(
            b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>",
        )
        .unwrap();
        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"word/document.xml\"/></Relationships>").as_bytes()).unwrap();
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(document.as_bytes()).unwrap();
        zip.start_file("word/media/image.bin", options).unwrap();
        zip.write_all(&[1, 2, 3]).unwrap();
        zip.finish().unwrap();
        path
    }

    fn styled_fixture(document: &str) -> std::path::PathBuf {
        let path = path("style-input");
        let file = fs::File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(
            b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>",
        )
        .unwrap();
        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"word/document.xml\"/></Relationships>").as_bytes()).unwrap();
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(document.as_bytes()).unwrap();
        zip.start_file("word/_rels/document.xml.rels", options)
            .unwrap();
        zip.write_all(b"<Relationships><Relationship Id=\"rIdStyles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>").unwrap();
        zip.start_file("word/styles.xml", options).unwrap();
        zip.write_all(format!("<w:styles xmlns:w=\"{WORD}\"><w:style w:type=\"paragraph\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style><w:style w:type=\"paragraph\" w:styleId=\"HeadingOne\"><w:name w:val=\"Heading 1\"/></w:style><w:style w:type=\"character\" w:styleId=\"Emphasis\"><w:name w:val=\"Emphasis\"/></w:style></w:styles>").as_bytes()).unwrap();
        zip.start_file("word/media/image.bin", options).unwrap();
        zip.write_all(&[1, 2, 3]).unwrap();
        zip.finish().unwrap();
        path
    }

    fn table_operation(
        row_label: &str,
        column_header: &str,
        expected: &str,
        replacement: &str,
    ) -> SetTableCellText {
        SetTableCellText {
            target: TableCellTarget {
                row_label: row_label.to_owned(),
                column_header: column_header.to_owned(),
                occurrence: None,
            },
            expected_current_text: expected.to_owned(),
            replacement: replacement.to_owned(),
            base_revision: Some("caller-version-7".to_owned()),
        }
    }

    fn table_execute(input: &Path, output: &Path, operation: &SetTableCellText) -> OperationResult {
        let package = Package::open(input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        set_table_cell_text(&package, &main, &source, operation, output)
    }

    fn row_operation(headers: &[&str], after: &str, cells: &[&str]) -> InsertTableRowAfter {
        InsertTableRowAfter {
            table: TableTarget {
                header_cells: headers.iter().map(|value| (*value).to_owned()).collect(),
                occurrence: None,
            },
            after: TableRowTarget {
                first_cell_text: after.to_owned(),
                occurrence: None,
            },
            cells: cells.iter().map(|value| (*value).to_owned()).collect(),
            base_revision: Some("caller-version-7".to_owned()),
        }
    }

    fn row_execute(
        input: &Path,
        operation: &InsertTableRowAfter,
    ) -> Result<Vec<u8>, OperationResult> {
        let package = Package::open(input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        insert_table_row_after_to_vec(&package, &main, &source, operation)
    }

    fn rows_execute(
        input: &Path,
        operation: &InsertTableRowsAfter,
    ) -> Result<Vec<u8>, OperationResult> {
        let package = Package::open(input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        insert_table_rows_after_to_vec(&package, &main, &source, operation)
    }

    fn cells_execute(
        input: &Path,
        operation: &SetTableCellsText,
    ) -> Result<Vec<u8>, OperationResult> {
        let package = Package::open(input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        set_table_cells_text_to_vec(&package, &main, &source, operation)
    }

    fn control_operation(
        tag: Option<&str>,
        alias: Option<&str>,
        expected: &str,
        replacement: &str,
    ) -> SetContentControlText {
        SetContentControlText {
            target: ContentControlTarget {
                tag: tag.map(str::to_owned),
                alias: alias.map(str::to_owned),
                occurrence: None,
            },
            expected_current_text: expected.to_owned(),
            replacement: replacement.to_owned(),
            base_revision: Some("caller-version-7".to_owned()),
        }
    }

    fn control_execute(
        input: &Path,
        output: &Path,
        operation: &SetContentControlText,
    ) -> OperationResult {
        let package = Package::open(input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        set_content_control_text(&package, &main, &source, operation, output)
    }

    fn operation(target: &str, expected: &str, replacement: &str) -> ReplaceText {
        ReplaceText {
            target: TextTarget {
                text: target.to_owned(),
                occurrence: None,
            },
            expected_current_text: expected.to_owned(),
            replacement: replacement.to_owned(),
            base_revision: Some("caller-version-7".to_owned()),
        }
    }

    fn execute(input: &Path, output: &Path, operation: &ReplaceText) -> OperationResult {
        let package = Package::open(input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        replace_text(&package, &main, &source, operation, output)
    }

    fn insert_operation(anchor: &str, text: &str) -> InsertParagraphAfter {
        InsertParagraphAfter {
            anchor: TextTarget {
                text: anchor.to_owned(),
                occurrence: None,
            },
            text: text.to_owned(),
            base_revision: Some("caller-version-7".to_owned()),
        }
    }

    fn insert_execute(
        input: &Path,
        output: &Path,
        operation: &InsertParagraphAfter,
    ) -> OperationResult {
        let package = Package::open(input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        insert_paragraph_after(&package, &main, &source, operation, output)
    }

    fn delete_operation(target: &str) -> DeleteParagraph {
        DeleteParagraph {
            target: TextTarget {
                text: target.to_owned(),
                occurrence: None,
            },
            base_revision: Some("caller-version-7".to_owned()),
        }
    }

    fn delete_execute(input: &Path, output: &Path, operation: &DeleteParagraph) -> OperationResult {
        let package = Package::open(input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        delete_paragraph(&package, &main, &source, operation, output)
    }

    fn entry(path: &Path, name: &str) -> Vec<u8> {
        let file = fs::File::open(path).unwrap();
        let mut zip = ZipArchive::new(file).unwrap();
        let mut value = Vec::new();
        std::io::Read::read_to_end(&mut zip.by_name(name).unwrap(), &mut value).unwrap();
        value
    }

    #[test]
    fn inspects_and_replaces_text_entirely_in_memory() {
        let input = fixture();
        let bytes = fs::read(&input).unwrap();
        fs::remove_file(input).unwrap();
        let package = Package::from_bytes(bytes).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(
            crate::DocxDocument::new(&source)
                .unwrap()
                .paragraphs()
                .next()
                .unwrap()
                .text_for_view(RevisionView::Current)
                .unwrap(),
            "OLD UNIQUE TEXT"
        );

        let output = replace_text_to_vec(
            &package,
            &main,
            &source,
            &operation("OLD UNIQUE TEXT", "OLD UNIQUE TEXT", "NEW UNIQUE TEXT"),
        )
        .unwrap();
        let reopened = Package::from_bytes(output).unwrap();
        reopened.verify().unwrap();
        let (_, source) = crate::open_main_source(&reopened).unwrap();
        assert_eq!(
            crate::DocxDocument::new(&source)
                .unwrap()
                .paragraphs()
                .next()
                .unwrap()
                .text_for_view(RevisionView::Current)
                .unwrap(),
            "NEW UNIQUE TEXT"
        );
    }

    #[test]
    fn replaces_one_current_text_node_without_changing_other_parts() {
        let input = fixture();
        let output = path("output");
        let result = execute(
            &input,
            &output,
            &operation("OLD UNIQUE TEXT", "OLD UNIQUE TEXT", "NEW & < >"),
        );

        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert_eq!(result.changes[0].before, "OLD UNIQUE TEXT");
        assert_eq!(result.changes[0].after, "NEW & < >");
        assert_eq!(result.to_json()["changes"][0]["kind"], "text_replaced");
        assert!(!result.to_json().to_string().contains("NodeId"));
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        assert_eq!(entry(&input, "_rels/.rels"), entry(&output, "_rels/.rels"));
        assert!(
            std::str::from_utf8(&entry(&output, "word/document.xml"))
                .unwrap()
                .contains("NEW &amp; &lt; &gt;")
        );
        assert!(
            std::str::from_utf8(&entry(&input, "word/document.xml"))
                .unwrap()
                .contains("OLD UNIQUE TEXT")
        );
        let package = Package::open(&output).unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        let document = crate::DocxDocument::new(&source).unwrap();
        assert!(
            document
                .paragraphs()
                .next()
                .unwrap()
                .text_for_view(RevisionView::Current)
                .unwrap()
                .contains("NEW & < >")
        );

        let whitespace_output = path("whitespace");
        let whitespace = execute(
            &input,
            &whitespace_output,
            &operation(" spaced ", " spaced ", " kept "),
        );
        assert_eq!(
            whitespace.status,
            opensuite_protocol::OperationStatus::Applied
        );
        assert!(
            std::str::from_utf8(&entry(&whitespace_output, "word/document.xml"))
                .unwrap()
                .contains("xml:space=\"preserve\"> kept </w:t>")
        );

        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
        fs::remove_file(whitespace_output).unwrap();
    }

    #[test]
    fn rejects_ambiguous_stale_unsafe_and_non_current_targets() {
        let input = fixture();
        let output = path("failed");
        for (target, expected, code) in [
            ("Duplicate", "Duplicate", "TARGET_AMBIGUOUS"),
            ("OLD UNIQUE TEXT", "stale", "PRECONDITION_FAILED"),
            ("Styled text", "Styled text", "UNSUPPORTED_OPERATION"),
            ("Linked text", "Linked text", "UNSUPPORTED_OPERATION"),
            ("OLD DELETED", "OLD DELETED", "TARGET_NOT_FOUND"),
            ("NEW INSERTED", "NEW INSERTED", "UNSUPPORTED_OPERATION"),
            ("missing", "missing", "TARGET_NOT_FOUND"),
        ] {
            let result = execute(&input, &output, &operation(target, expected, "new"));
            assert_eq!(result.status, opensuite_protocol::OperationStatus::Failed);
            assert_eq!(result.diagnostics[0].code, code);
            assert!(!output.exists());
        }
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn replaces_cross_run_text_with_distribution_partial_ranges_and_xml_space() {
        let input = fixture();
        let output = path("cross-run");
        let result = execute(
            &input,
            &output,
            &operation("Cross run", "Cross run", "Longer replacement"),
        );

        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
        assert!(xml.contains(
            "<w:t>Longer</w:t></w:r><w:r><w:t xml:space=\"preserve\"> replacement</w:t>"
        ));
        let package = Package::open(&output).unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        assert!(
            crate::find_text(
                &source,
                &opensuite_protocol::FindText {
                    text: "Longer replacement".to_owned()
                }
            )
            .unwrap()
            .matches
            .len()
                == 1
        );

        let partial_output = path("partial");
        let partial = execute(
            &input,
            &partial_output,
            &operation(
                "Revenue was $10M",
                "Revenue was $10M",
                "Profit was $12M & more",
            ),
        );
        assert_eq!(partial.status, opensuite_protocol::OperationStatus::Applied);
        let partial_xml = String::from_utf8(entry(&partial_output, "word/document.xml")).unwrap();
        assert!(partial_xml.contains("FY2026 Profit w"));
        assert!(partial_xml.contains("as $12M &amp; more according"));

        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
        fs::remove_file(partial_output).unwrap();
    }

    #[test]
    fn permits_shorter_cross_run_replacement_without_partial_output() {
        let input = fixture();
        let output = path("shorter");
        let result = execute(&input, &output, &operation("Cross run", "Cross run", "X"));

        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert!(
            String::from_utf8(entry(&output, "word/document.xml"))
                .unwrap()
                .contains("<w:t>X</w:t></w:r><w:r><w:t></w:t>")
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn uses_shared_occurrence_order_for_replacement() {
        let input = fixture();
        let output = path("occurrence");
        let mut request = operation("Duplicate", "Duplicate", "Changed");
        request.target.occurrence = Some(1);

        let result = execute(&input, &output, &request);

        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert!(
            std::str::from_utf8(&entry(&output, "word/document.xml"))
                .unwrap()
                .contains("<w:t>Duplicate</w:t></w:r><w:r><w:t>Changed</w:t>")
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn inserts_plain_paragraph_after_a_safe_anchor_and_preserves_other_parts() {
        let input = fixture();
        let output = path("inserted");
        let result = insert_execute(
            &input,
            &output,
            &insert_operation("OLD UNIQUE TEXT", "New & < > paragraph"),
        );

        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert_eq!(result.changes[0].kind, "paragraph_inserted");
        assert!(!result.to_json().to_string().contains("NodeId"));
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
        assert!(xml.contains("<w:t>OLD UNIQUE TEXT</w:t></w:r></w:p><w:p><w:r><w:t>New &amp; &lt; &gt; paragraph</w:t></w:r></w:p>"));
        let package = Package::open(&output).unwrap();
        package.verify().unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(
            crate::DocxDocument::new(&source)
                .unwrap()
                .paragraphs()
                .nth(1)
                .unwrap()
                .text()
                .unwrap(),
            "New & < > paragraph"
        );

        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn inserts_whitespace_unicode_and_empty_paragraphs() {
        for (text, expected) in [
            (" hello ", " xml:space=\"preserve\"> hello "),
            ("你好", ">你好<"),
            ("", "<w:p></w:p>"),
        ] {
            let input = fixture();
            let output = path("insert-text");
            let result =
                insert_execute(&input, &output, &insert_operation("OLD UNIQUE TEXT", text));
            assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
            assert!(
                String::from_utf8(entry(&output, "word/document.xml"))
                    .unwrap()
                    .contains(expected)
            );
            fs::remove_file(input).unwrap();
            fs::remove_file(output).unwrap();
        }
    }

    #[test]
    fn rejects_ambiguous_and_unsafe_structural_anchors() {
        let input = fixture();
        let output = path("insert-failed");
        assert_eq!(
            insert_execute(&input, &output, &insert_operation("Duplicate", "new")).diagnostics[0]
                .code,
            "TARGET_AMBIGUOUS"
        );
        let mut occurrence = insert_operation("Duplicate", "new");
        occurrence.anchor.occurrence = Some(1);
        assert_eq!(
            insert_execute(&input, &output, &occurrence).status,
            opensuite_protocol::OperationStatus::Applied
        );
        fs::remove_file(&output).unwrap();
        fs::remove_file(input).unwrap();

        for xml in [
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>needle</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr><w:sectPr/></w:pPr><w:r><w:t>needle</w:t></w:r></w:p></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:ins><w:r><w:t>needle</w:t></w:r></w:ins></w:p></w:body></w:document>"
            ),
        ] {
            let source = SourceDocument::parse(xml.into_bytes()).unwrap();
            assert_eq!(
                resolve_paragraph_anchor(
                    &source,
                    &TextTarget {
                        text: "needle".to_owned(),
                        occurrence: None
                    }
                )
                .unwrap_err()
                .diagnostics[0]
                    .code,
                "UNSUPPORTED_OPERATION"
            );
        }
    }

    #[test]
    fn uses_the_anchor_prefix_for_strict_wordprocessingml() {
        let strict = "http://purl.oclc.org/ooxml/wordprocessingml/main";
        let source = SourceDocument::parse(format!("<word:document xmlns:word=\"{strict}\"><word:body><word:p><word:r><word:t>needle</word:t></word:r></word:p><word:sectPr/></word:body></word:document>").into_bytes()).unwrap();
        let (_, paragraph) = resolve_paragraph_anchor(
            &source,
            &TextTarget {
                text: "needle".to_owned(),
                occurrence: None,
            },
        )
        .unwrap();
        assert_eq!(
            paragraph_fragment(&source, paragraph, "new").unwrap(),
            b"<word:p><word:r><word:t>new</word:t></word:r></word:p>"
        );
    }

    #[test]
    fn deletes_one_body_paragraph_without_changing_neighbor_or_other_part_bytes() {
        let input = fixture();
        let output = path("deleted");
        let package = Package::open(&input).unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        let (_, paragraph) = resolve_paragraph_anchor(
            &source,
            &TextTarget {
                text: "OLD UNIQUE TEXT".to_owned(),
                occurrence: None,
            },
        )
        .unwrap();
        let span = source.node(paragraph).unwrap().span();
        let mut expected = source.original_bytes()[..span.start].to_vec();
        expected.extend_from_slice(&source.original_bytes()[span.end..]);

        let result = delete_execute(&input, &output, &delete_operation("OLD UNIQUE TEXT"));

        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert_eq!(result.changes[0].kind, "paragraph_deleted");
        assert_eq!(result.changes[0].before, "OLD UNIQUE TEXT");
        assert!(!result.to_json().to_string().contains("SourceSpan"));
        assert_eq!(entry(&output, "word/document.xml"), expected);
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        let output_package = Package::open(&output).unwrap();
        output_package.verify().unwrap();
        let (_, output_source) = crate::open_main_source(&output_package).unwrap();
        let body = body_texts(&output_source).unwrap();
        assert_eq!(body[0].1, "DuplicateDuplicate");
        assert!(
            !body
                .iter()
                .any(|(_, text)| text.contains("OLD UNIQUE TEXT"))
        );

        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn deletion_uses_shared_occurrence_and_rejects_unsafe_ranges_or_wrappers() {
        let input = fixture();
        let output = path("delete-duplicate");
        assert_eq!(
            delete_execute(&input, &output, &delete_operation("Duplicate")).diagnostics[0].code,
            "TARGET_AMBIGUOUS"
        );
        let mut operation = delete_operation("Duplicate");
        operation.target.occurrence = Some(1);
        assert_eq!(
            delete_execute(&input, &output, &operation).status,
            opensuite_protocol::OperationStatus::Applied
        );
        fs::remove_file(&output).unwrap();
        fs::remove_file(input).unwrap();

        for xml in [
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>needle</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr><w:sectPr/></w:pPr><w:r><w:t>needle</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:sdt><w:sdtContent><w:p><w:r><w:t>needle</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:ins><w:r><w:t>needle</w:t></w:r></w:ins></w:p></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:bookmarkStart w:id=\"1\" w:name=\"mark\"/><w:r><w:t>needle</w:t></w:r><w:bookmarkEnd w:id=\"1\"/></w:p></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:commentRangeStart w:id=\"1\"/><w:r><w:t>needle</w:t></w:r><w:commentRangeEnd w:id=\"1\"/><w:r><w:commentReference w:id=\"1\"/></w:r></w:p></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:fldSimple w:instr=\"DATE\"><w:r><w:t>needle</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"
            ),
        ] {
            let source = SourceDocument::parse(xml.into_bytes()).unwrap();
            let result = resolve_paragraph_anchor(
                &source,
                &TextTarget {
                    text: "needle".to_owned(),
                    occurrence: None,
                },
            );
            if let Ok((_, paragraph)) = result {
                assert!(!safe_to_delete(&source, paragraph));
            } else {
                assert_eq!(
                    result.unwrap_err().diagnostics[0].code,
                    "UNSUPPORTED_OPERATION"
                );
            }
        }
    }

    #[test]
    fn sets_one_simple_table_cell_and_preserves_other_payloads() {
        let xml = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Status</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>100</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Draft</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Expenses</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>50</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Draft</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
        );
        let input = table_fixture(&xml);
        let output = path("table-output");
        let result = table_execute(
            &input,
            &output,
            &table_operation("Revenue", "Amount", "100", "125 & < >"),
        );
        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert_eq!(result.changes[0].kind, "table_cell_text_set");
        assert!(!result.to_json().to_string().contains("NodeId"));
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
        assert!(xml.contains("<w:t>125 &amp; &lt; &gt;</w:t>"));
        assert!(xml.contains("<w:t>Expenses</w:t>"));
        Package::open(&output).unwrap().verify().unwrap();
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn inserts_a_safe_row_after_a_semantic_anchor_and_preserves_tables() {
        let xml = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>Before</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:tcPr><w:shd w:fill=\"DDDDDD\"/></w:tcPr><w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Role</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:shd w:fill=\"EEEEEE\"/></w:tcPr><w:p><w:pPr><w:jc w:val=\"left\"/></w:pPr><w:r><w:rPr><w:i/></w:rPr><w:t>Alice</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CEO</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:shd w:fill=\"EEEEEE\"/></w:tcPr><w:p><w:pPr><w:jc w:val=\"left\"/></w:pPr><w:r><w:rPr><w:i/></w:rPr><w:t>Bob</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CTO</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Other</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Table</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Keep</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Same</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
        );
        let input = table_fixture(&xml);
        let output = row_execute(
            &input,
            &row_operation(&["Name", "Role"], "Bob", &[" Charlie ", "CFO & < >"]),
        )
        .unwrap();
        let package = Package::from_bytes(output.clone()).unwrap();
        package.verify().unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(
            all_table_rows(&source).unwrap(),
            vec![
                vec![
                    vec!["Name".to_owned(), "Role".to_owned()],
                    vec!["Alice".to_owned(), "CEO".to_owned()],
                    vec!["Bob".to_owned(), "CTO".to_owned()],
                    vec![" Charlie ".to_owned(), "CFO & < >".to_owned()],
                ],
                vec![
                    vec!["Other".to_owned(), "Table".to_owned()],
                    vec!["Keep".to_owned(), "Same".to_owned()],
                ],
            ]
        );
        let output_xml = String::from_utf8(
            package
                .read_part(&package.main_office_document().unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(output_xml.contains("xml:space=\"preserve\"> Charlie </w:t>"));
        assert!(output_xml.contains("CFO &amp; &lt; &gt;"));
        assert!(output_xml.contains("<w:shd w:fill=\"EEEEEE\"/>"));
        assert_eq!(entry(&input, "word/media/image.bin"), vec![1, 2, 3]);
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn inserts_multiple_rows_contiguously_and_validates_all_rows_first() {
        let xml = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Role</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Alice</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CEO</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Bob</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CTO</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
        );
        let input = table_fixture(&xml);
        let operation = InsertTableRowsAfter {
            table: TableTarget {
                header_cells: vec!["Name".to_owned(), "Role".to_owned()],
                occurrence: None,
            },
            after: TableRowTarget {
                first_cell_text: "Alice".to_owned(),
                occurrence: None,
            },
            rows: vec![
                vec!["Charlie".to_owned(), "CFO".to_owned()],
                vec!["David".to_owned(), "COO".to_owned()],
                vec!["Emma".to_owned(), String::new()],
            ],
            base_revision: None,
        };
        let output = rows_execute(&input, &operation).unwrap();
        let package = Package::from_bytes(output.clone()).unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(
            all_table_rows(&source).unwrap()[0][2..],
            [
                vec!["Charlie", "CFO"],
                vec!["David", "COO"],
                vec!["Emma", ""],
                vec!["Bob", "CTO"]
            ]
        );
        assert_eq!(entry(&input, "word/media/image.bin"), vec![1, 2, 3]);

        let after_final = InsertTableRowsAfter {
            after: TableRowTarget {
                first_cell_text: "Bob".to_owned(),
                occurrence: None,
            },
            rows: vec![
                vec!["Final one".to_owned(), "CIO".to_owned()],
                vec!["Final two".to_owned(), "CPO".to_owned()],
            ],
            ..operation.clone()
        };
        let output = rows_execute(&input, &after_final).unwrap();
        let package = Package::from_bytes(output).unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(
            all_table_rows(&source).unwrap()[0][3..],
            [vec!["Final one", "CIO"], vec!["Final two", "CPO"]]
        );

        let mut invalid = operation;
        invalid.rows[1].pop();
        assert_eq!(
            rows_execute(&input, &invalid).unwrap_err().diagnostics[0].code,
            "PRECONDITION_FAILED"
        );
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn sets_multiple_table_cells_atomically() {
        let xml = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Role</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Team</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Alice</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CEO</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Product</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Bob</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CTO</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Platform</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
        );
        let input = table_fixture(&xml);
        let update =
            |row: &str, column: &str, expected: &str, replacement: &str| TableCellTextUpdate {
                target: TableCellTarget {
                    row_label: row.to_owned(),
                    column_header: column.to_owned(),
                    occurrence: None,
                },
                expected_current_text: expected.to_owned(),
                replacement: replacement.to_owned(),
            };
        let operation = SetTableCellsText {
            table: TableTarget {
                header_cells: vec!["Name".to_owned(), "Role".to_owned(), "Team".to_owned()],
                occurrence: None,
            },
            updates: vec![
                update("Alice", "Role", "CEO", "Founder & CEO"),
                update("Bob", "Role", "CTO", "CTO & VP Engineering"),
                update("Bob", "Team", "Platform", "Engineering"),
            ],
            base_revision: None,
        };
        let output = cells_execute(&input, &operation).unwrap();
        let package = Package::from_bytes(output).unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(all_table_rows(&source).unwrap()[0][1][1], "Founder & CEO");
        assert_eq!(
            all_table_rows(&source).unwrap()[0][2][1..],
            ["CTO & VP Engineering", "Engineering"]
        );
        let mut bad = operation.clone();
        bad.updates[2].expected_current_text = "wrong".to_owned();
        assert_eq!(
            cells_execute(&input, &bad).unwrap_err().diagnostics[0].code,
            "PRECONDITION_FAILED"
        );
        let duplicate = SetTableCellsText {
            updates: vec![operation.updates[0].clone(), operation.updates[0].clone()],
            ..operation
        };
        assert_eq!(
            cells_execute(&input, &duplicate).unwrap_err().diagnostics[0].code,
            "PRECONDITION_FAILED"
        );
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn rejects_unsafe_or_ambiguous_row_insertions() {
        let table = "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Role</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Bob</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CTO</w:t></w:r></w:p></w:tc></w:tr></w:tbl>";
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body>{table}{table}</w:body></w:document>"
        ));
        let operation = row_operation(&["Name", "Role"], "Bob", &["Charlie", "CFO"]);
        assert_eq!(
            row_execute(&input, &operation).unwrap_err().diagnostics[0].code,
            "TARGET_AMBIGUOUS"
        );
        let wrong = row_operation(&["Name", "Role"], "Bob", &["Charlie"]);
        let single = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body>{table}</w:body></w:document>"
        ));
        assert_eq!(
            row_execute(&single, &wrong).unwrap_err().diagnostics[0].code,
            "PRECONDITION_FAILED"
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(single).unwrap();
    }

    #[test]
    fn fills_an_empty_simple_cell_and_rejects_ambiguous_or_complex_cells() {
        let empty = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:p/></w:tc></w:tr></w:tbl></w:body></w:document>"
        );
        let input = table_fixture(&empty);
        let output = path("table-empty");
        let result = table_execute(
            &input,
            &output,
            &table_operation("Revenue", "Amount", "", " kept "),
        );
        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert!(
            String::from_utf8(entry(&output, "word/document.xml"))
                .unwrap()
                .contains("<w:p><w:r><w:t xml:space=\"preserve\"> kept </w:t></w:r></w:p>")
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();

        for xml in [
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:p><w:fldSimple w:instr=\"DATE\"><w:r><w:t>100</w:t></w:r></w:fldSimple></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>100</w:t></w:r></w:p><w:p><w:r><w:t>more</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:gridSpan w:val=\"2\"/></w:tcPr><w:p><w:r><w:t>100</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
            ),
        ] {
            let input = table_fixture(&xml);
            let output = path("table-unsupported");
            assert_eq!(
                table_execute(
                    &input,
                    &output,
                    &table_operation("Revenue", "Amount", "100", "125")
                )
                .diagnostics[0]
                    .code,
                "UNSUPPORTED_OPERATION"
            );
            fs::remove_file(input).unwrap();
        }
    }

    #[test]
    fn table_targets_require_occurrence_and_current_text_preconditions() {
        let table = "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Revenue</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>100</w:t></w:r></w:p></w:tc></w:tr></w:tbl>";
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body>{table}{table}</w:body></w:document>"
        ));
        let output = path("table-ambiguous");
        let operation = table_operation("Revenue", "Amount", "100", "125");
        assert_eq!(
            table_execute(&input, &output, &operation).diagnostics[0].code,
            "TARGET_AMBIGUOUS"
        );
        let mut selected = operation.clone();
        selected.target.occurrence = Some(1);
        assert_eq!(
            table_execute(&input, &output, &selected).status,
            opensuite_protocol::OperationStatus::Applied
        );
        fs::remove_file(&output).unwrap();
        let mut wrong = table_operation("Revenue", "Amount", "wrong", "125");
        wrong.target.occurrence = Some(0);
        assert_eq!(
            table_execute(&input, &output, &wrong).diagnostics[0].code,
            "PRECONDITION_FAILED"
        );
        assert!(!output.exists());
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn sets_simple_content_control_text_by_tag_or_alias() {
        let xml = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:sdt><w:sdtPr><w:tag w:val=\"customer_name\"/><w:alias w:val=\"Customer Name\"/><w:id w:val=\"7\"/><w:text/></w:sdtPr><w:sdtContent><w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr><w:r><w:t>Acme</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"
        );
        let input = table_fixture(&xml);
        let output = path("control-output");
        let result = control_execute(
            &input,
            &output,
            &control_operation(
                Some("customer_name"),
                Some("Customer Name"),
                "Acme",
                "New & < >",
            ),
        );
        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert_eq!(result.changes[0].kind, "content_control_text_set");
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        let output_xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
        assert!(output_xml.contains("<w:t>New &amp; &lt; &gt;</w:t>"));
        assert!(output_xml.contains("w:tag w:val=\"customer_name\""));
        Package::open(&output).unwrap().verify().unwrap();
        fs::remove_file(output).unwrap();
        let alias_output = path("control-alias");
        assert_eq!(
            control_execute(
                &input,
                &alias_output,
                &control_operation(None, Some("Customer Name"), "Acme", "Alias value")
            )
            .status,
            opensuite_protocol::OperationStatus::Applied
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(alias_output).unwrap();
    }

    #[test]
    fn fills_empty_content_control_and_rejects_unsafe_controls() {
        let empty = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:sdt><w:sdtPr><w:tag w:val=\"name\"/><w:text/></w:sdtPr><w:sdtContent><w:p/></w:sdtContent></w:sdt></w:body></w:document>"
        );
        let input = table_fixture(&empty);
        let output = path("control-empty");
        assert_eq!(
            control_execute(
                &input,
                &output,
                &control_operation(Some("name"), None, "", " kept ")
            )
            .status,
            opensuite_protocol::OperationStatus::Applied
        );
        assert!(
            String::from_utf8(entry(&output, "word/document.xml"))
                .unwrap()
                .contains("<w:p><w:r><w:t xml:space=\"preserve\"> kept </w:t></w:r></w:p>")
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();

        for properties in [
            "<w:dataBinding w:xpath=\"/name\"/><w:text/>",
            "<w:lock w:val=\"contentLocked\"/><w:text/>",
            "<w:showingPlcHdr/><w:text/>",
            "<w:dropDownList/><w:text/>",
        ] {
            let xml = format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:sdt><w:sdtPr><w:tag w:val=\"name\"/>{properties}</w:sdtPr><w:sdtContent><w:p><w:r><w:t>Acme</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"
            );
            let input = table_fixture(&xml);
            let output = path("control-unsupported");
            assert_eq!(
                control_execute(
                    &input,
                    &output,
                    &control_operation(Some("name"), None, "Acme", "New")
                )
                .diagnostics[0]
                    .code,
                "UNSUPPORTED_OPERATION"
            );
            fs::remove_file(input).unwrap();
        }
    }

    #[test]
    fn content_control_targets_require_occurrence_and_expected_text() {
        let control = "<w:sdt><w:sdtPr><w:tag w:val=\"name\"/><w:text/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Acme</w:t></w:r></w:p></w:sdtContent></w:sdt>";
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body>{control}{control}</w:body></w:document>"
        ));
        let output = path("control-ambiguous");
        let operation = control_operation(Some("name"), None, "Acme", "New");
        assert_eq!(
            control_execute(&input, &output, &operation).diagnostics[0].code,
            "TARGET_AMBIGUOUS"
        );
        let mut selected = operation.clone();
        selected.target.occurrence = Some(1);
        assert_eq!(
            control_execute(&input, &output, &selected).status,
            opensuite_protocol::OperationStatus::Applied
        );
        fs::remove_file(&output).unwrap();
        let mut wrong = control_operation(Some("name"), None, "wrong", "New");
        wrong.target.occurrence = Some(0);
        assert_eq!(
            control_execute(&input, &output, &wrong).diagnostics[0].code,
            "PRECONDITION_FAILED"
        );
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn sets_direct_paragraph_formatting_without_rewriting_unknown_properties() {
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr w:custom=\"keep\"><w:pStyle w:val=\"Body\"/><w:unknown w:value=\"stay\"/></w:pPr><w:r><w:t>Format me</w:t></w:r></w:p></w:body></w:document>"
        ));
        let output = path("formatting");
        let operation = SetParagraphFormatting {
            target: TextTarget {
                text: "Format me".to_owned(),
                occurrence: None,
            },
            formatting: ParagraphFormattingPatch {
                alignment: Some(PropertyPatch::Set(ParagraphAlignment::Center)),
                spacing_before_twips: Some(PropertyPatch::Set(120)),
                keep_with_next: Some(PropertyPatch::Set(true)),
                ..Default::default()
            },
            base_revision: None,
        };
        let package = Package::open(&input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(
            set_paragraph_formatting(&package, &main, &source, &operation, &output).status,
            opensuite_protocol::OperationStatus::Applied
        );
        let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
        assert!(xml.contains("w:custom=\"keep\""));
        assert!(xml.contains("<w:unknown w:value=\"stay\"/>"));
        assert!(xml.contains("<w:jc w:val=\"center\"/>"));
        assert!(xml.contains("w:before=\"120\""));
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn clears_direct_paragraph_formatting_and_rejects_table_paragraphs() {
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr><w:jc w:val=\"right\"/><w:spacing w:before=\"120\"/></w:pPr><w:r><w:t>Clear me</w:t></w:r></w:p></w:body></w:document>"
        ));
        let output = path("formatting-clear");
        let operation = SetParagraphFormatting {
            target: TextTarget {
                text: "Clear me".to_owned(),
                occurrence: None,
            },
            formatting: ParagraphFormattingPatch {
                alignment: Some(PropertyPatch::Clear),
                spacing_before_twips: Some(PropertyPatch::Clear),
                ..Default::default()
            },
            base_revision: None,
        };
        let package = Package::open(&input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(
            set_paragraph_formatting(&package, &main, &source, &operation, &output).status,
            opensuite_protocol::OperationStatus::Applied
        );
        assert!(
            !String::from_utf8(entry(&output, "word/document.xml"))
                .unwrap()
                .contains("w:before=\"120\"")
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();

        let table = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>In table</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
        ));
        let package = Package::open(&table).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(
            set_paragraph_formatting(
                &package,
                &main,
                &source,
                &SetParagraphFormatting {
                    target: TextTarget {
                        text: "In table".to_owned(),
                        occurrence: None
                    },
                    formatting: ParagraphFormattingPatch::default(),
                    base_revision: None
                },
                path("formatting-table")
            )
            .diagnostics[0]
                .code,
            "UNSUPPORTED_OPERATION"
        );
        fs::remove_file(table).unwrap();
    }

    #[test]
    fn sets_text_formatting_without_rewriting_text_or_unknown_run_properties() {
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:rPr><w:unknown w:value=\"stay\"/><w:rFonts w:eastAsia=\"Keep\"/></w:rPr><w:t>Format run</w:t></w:r></w:p></w:body></w:document>"
        ));
        let output = path("text-formatting");
        let operation = SetTextFormatting {
            target: TextTarget {
                text: "Format run".to_owned(),
                occurrence: None,
            },
            formatting: TextFormattingPatch {
                bold: Some(PropertyPatch::Set(true)),
                italic: Some(PropertyPatch::Set(false)),
                font_size_half_points: Some(PropertyPatch::Set(28)),
                font_family: Some(PropertyPatch::Set("Aptos".to_owned())),
            },
            base_revision: None,
        };
        let package = Package::open(&input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let result = set_text_formatting(&package, &main, &source, &operation, &output);
        assert_eq!(
            result.status,
            opensuite_protocol::OperationStatus::Applied,
            "{result:?}"
        );
        let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
        assert!(xml.contains("<w:t>Format run</w:t>"));
        assert!(xml.contains("<w:b />"));
        assert!(xml.contains("<w:i w:val=\"0\"/>"));
        assert!(xml.contains("w:sz w:val=\"28\""));
        assert!(xml.contains("w:eastAsia=\"Keep\""));
        assert!(xml.contains("w:ascii=\"Aptos\""));
        assert!(xml.contains("<w:unknown w:value=\"stay\"/>"));
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn rejects_partial_or_cross_run_text_formatting_and_clears_direct_properties() {
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:rPr><w:b/><w:i/><w:sz w:val=\"24\"/><w:rFonts w:ascii=\"Aptos\" w:eastAsia=\"Keep\"/></w:rPr><w:t>Whole</w:t></w:r><w:r><w:t> run</w:t></w:r></w:p></w:body></w:document>"
        ));
        let output = path("text-formatting-clear");
        let package = Package::open(&input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let clear = SetTextFormatting {
            target: TextTarget {
                text: "Whole".to_owned(),
                occurrence: None,
            },
            formatting: TextFormattingPatch {
                bold: Some(PropertyPatch::Clear),
                italic: Some(PropertyPatch::Clear),
                font_size_half_points: Some(PropertyPatch::Clear),
                font_family: Some(PropertyPatch::Clear),
            },
            base_revision: None,
        };
        let result = set_text_formatting(&package, &main, &source, &clear, &output);
        assert_eq!(
            result.status,
            opensuite_protocol::OperationStatus::Applied,
            "{result:?}"
        );
        let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
        assert!(!xml.contains("<w:b/>"));
        assert!(!xml.contains("<w:i/>"));
        assert!(!xml.contains("w:sz"));
        assert!(xml.contains("w:eastAsia=\"Keep\""));
        fs::remove_file(output).unwrap();
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn rejects_wrapped_or_tracked_text_formatting() {
        for body in [
            "<w:p><w:hyperlink><w:r><w:t>Unsafe</w:t></w:r></w:hyperlink></w:p>",
            "<w:p><w:ins><w:r><w:t>Unsafe</w:t></w:r></w:ins></w:p>",
            "<w:sdt><w:sdtPr><w:tag w:val=\"x\"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Unsafe</w:t></w:r></w:p></w:sdtContent></w:sdt>",
        ] {
            let input = table_fixture(&format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body>{body}</w:body></w:document>"
            ));
            let package = Package::open(&input).unwrap();
            let (main, source) = crate::open_main_source(&package).unwrap();
            let result = set_text_formatting(
                &package,
                &main,
                &source,
                &SetTextFormatting {
                    target: TextTarget {
                        text: "Unsafe".to_owned(),
                        occurrence: None,
                    },
                    formatting: TextFormattingPatch {
                        bold: Some(PropertyPatch::Set(true)),
                        ..Default::default()
                    },
                    base_revision: None,
                },
                path("text-formatting-wrapper"),
            );
            assert_eq!(result.diagnostics[0].code, "UNSUPPORTED_OPERATION");
            fs::remove_file(input).unwrap();
        }
    }

    #[test]
    fn sets_and_clears_existing_paragraph_styles_by_name_without_touching_styles_part() {
        let input = styled_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr w:unknown=\"keep\"><w:pStyle w:val=\"Normal\"/><w:jc w:val=\"center\"/><w:unknown/></w:pPr><w:r><w:t>Style me</w:t></w:r></w:p><w:p><w:r><w:t>Other</w:t></w:r></w:p></w:body></w:document>"
        ));
        let output = path("style-output");
        let package = Package::open(&input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let operation = SetParagraphStyle {
            target: TextTarget {
                text: "Style me".to_owned(),
                occurrence: None,
            },
            style: PropertyPatch::Set("Heading 1".to_owned()),
            base_revision: None,
        };
        assert_eq!(
            set_paragraph_style(&package, &main, &source, &operation, &output).status,
            opensuite_protocol::OperationStatus::Applied
        );
        let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
        assert!(xml.contains("<w:pStyle w:val=\"HeadingOne\"/>"));
        assert!(xml.contains("<w:jc w:val=\"center\"/>"));
        assert!(xml.contains("<w:unknown/>"));
        assert_eq!(
            entry(&input, "word/styles.xml"),
            entry(&output, "word/styles.xml")
        );
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        let updated = Package::open(&output).unwrap();
        let (main, source) = crate::open_main_source(&updated).unwrap();
        let clear = SetParagraphStyle {
            target: TextTarget {
                text: "Style me".to_owned(),
                occurrence: None,
            },
            style: PropertyPatch::Clear,
            base_revision: None,
        };
        let cleared = path("style-cleared");
        assert_eq!(
            set_paragraph_style(&updated, &main, &source, &clear, &cleared).status,
            opensuite_protocol::OperationStatus::Applied
        );
        assert!(
            !String::from_utf8(entry(&cleared, "word/document.xml"))
                .unwrap()
                .contains("pStyle")
        );
        for (style, code) in [
            ("Emphasis", "UNSUPPORTED_OPERATION"),
            ("Missing", "TARGET_NOT_FOUND"),
        ] {
            let result = set_paragraph_style(
                &package,
                &main,
                &source,
                &SetParagraphStyle {
                    target: TextTarget {
                        text: "Style me".to_owned(),
                        occurrence: None,
                    },
                    style: PropertyPatch::Set(style.to_owned()),
                    base_revision: None,
                },
                path("style-rejected"),
            );
            assert_eq!(result.diagnostics[0].code, code);
        }
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
        fs::remove_file(cleared).unwrap();
    }

    #[test]
    fn replaces_unique_png_picture_and_preserves_xml_parts() {
        let input = valid_picture_fixture();
        let output = path("picture-output");
        let package = Package::open(&input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let replacement = b"\x89PNG\r\n\x1a\nreplacement".to_vec();
        let result = replace_picture(
            &package,
            &main,
            &source,
            &ReplacePicture {
                target: opensuite_protocol::PictureTarget {
                    name: None,
                    description: Some("Company logo".to_owned()),
                    occurrence: None,
                },
                replacement: opensuite_protocol::ImagePayload {
                    content_type: "image/png".to_owned(),
                    bytes: replacement.clone(),
                },
                base_revision: None,
            },
            &output,
        );
        assert_eq!(
            result.status,
            opensuite_protocol::OperationStatus::Applied,
            "{result:?}"
        );
        assert_eq!(entry(&output, "media/logo.png"), replacement);
        let mut input_payloads = payloads(&input);
        let mut output_payloads = payloads(&output);
        input_payloads.remove("media/logo.png");
        output_payloads.remove("media/logo.png");
        assert_eq!(input_payloads, output_payloads);
        let reopened = Package::open(&output).unwrap();
        let (main, source) = crate::open_main_source(&reopened).unwrap();
        let picture = crate::DocxDocument::new(&source)
            .unwrap()
            .pictures()
            .find(|picture| {
                picture.metadata().and_then(|meta| meta.name) == Some("Logo".to_owned())
            })
            .unwrap();
        assert_eq!(
            picture.metadata().unwrap().description.as_deref(),
            Some("Company logo")
        );
        assert_eq!(
            reopened
                .read_part(&match picture.image_reference(&reopened, &main).unwrap() {
                    crate::ImageReference::Embedded(image) => image.part,
                    _ => panic!(),
                })
                .unwrap(),
            replacement
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn rejects_picture_type_mismatch_invalid_and_ambiguous_targets() {
        let input = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/images.docx");
        let package = Package::open(&input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let run = |name: Option<&str>, content_type: &str, bytes: Vec<u8>| {
            replace_picture(
                &package,
                &main,
                &source,
                &ReplacePicture {
                    target: opensuite_protocol::PictureTarget {
                        name: name.map(str::to_owned),
                        description: None,
                        occurrence: None,
                    },
                    replacement: opensuite_protocol::ImagePayload {
                        content_type: content_type.to_owned(),
                        bytes,
                    },
                    base_revision: None,
                },
                path("picture-rejected"),
            )
        };
        assert_eq!(
            run(
                Some("Logo"),
                "image/jpeg",
                vec![0xff, 0xd8, 0xff, 0xff, 0xd9]
            )
            .diagnostics[0]
                .code,
            "UNSUPPORTED_OPERATION"
        );
        assert_eq!(
            run(Some("Logo"), "image/png", b"bad".to_vec()).diagnostics[0].code,
            "UNSUPPORTED_OPERATION"
        );
        assert_eq!(
            run(Some("Missing"), "image/png", b"\x89PNG\r\n\x1a\n".to_vec()).diagnostics[0].code,
            "TARGET_NOT_FOUND"
        );
    }

    #[test]
    fn replaces_unique_jpeg_picture_by_name() {
        let input = valid_picture_fixture();
        let output = path("picture-jpeg-output");
        let package = Package::open(&input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let replacement = vec![0xff, 0xd8, 0xff, 0, 0xff, 0xd9];
        let result = replace_picture(
            &package,
            &main,
            &source,
            &ReplacePicture {
                target: opensuite_protocol::PictureTarget {
                    name: Some("Photo".to_owned()),
                    description: None,
                    occurrence: None,
                },
                replacement: opensuite_protocol::ImagePayload {
                    content_type: "image/jpeg".to_owned(),
                    bytes: replacement.clone(),
                },
                base_revision: None,
            },
            &output,
        );
        assert_eq!(
            result.status,
            opensuite_protocol::OperationStatus::Applied,
            "{result:?}"
        );
        assert_eq!(entry(&output, "media/photo.jpeg"), replacement);
        assert_eq!(
            entry(&input, "media/logo.png"),
            entry(&output, "media/logo.png")
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn requires_occurrence_for_duplicate_pictures_and_rejects_shared_media() {
        let input = picture_fixture(&[
            ("Duplicate", "one", "../media/one.png"),
            ("Duplicate", "two", "../media/two.png"),
        ]);
        let output = path("picture-duplicate-output");
        let package = Package::open(&input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let operation = |occurrence| ReplacePicture {
            target: opensuite_protocol::PictureTarget {
                name: Some("Duplicate".to_owned()),
                description: None,
                occurrence,
            },
            replacement: opensuite_protocol::ImagePayload {
                content_type: "image/png".to_owned(),
                bytes: b"\x89PNG\r\n\x1a\nreplacement".to_vec(),
            },
            base_revision: None,
        };
        assert_eq!(
            replace_picture(&package, &main, &source, &operation(None), &output).diagnostics[0]
                .code,
            "TARGET_AMBIGUOUS"
        );
        let result = replace_picture(&package, &main, &source, &operation(Some(1)), &output);
        assert_eq!(
            result.status,
            opensuite_protocol::OperationStatus::Applied,
            "{result:?}"
        );
        assert_eq!(
            entry(&input, "media/one.png"),
            entry(&output, "media/one.png")
        );
        assert_eq!(
            entry(&output, "media/two.png"),
            b"\x89PNG\r\n\x1a\nreplacement"
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();

        let shared = picture_fixture(&[
            ("First", "one", "../media/shared.png"),
            ("Second", "two", "../media/shared.png"),
        ]);
        let package = Package::open(&shared).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let shared_operation = ReplacePicture {
            target: opensuite_protocol::PictureTarget {
                name: Some("First".to_owned()),
                description: None,
                occurrence: None,
            },
            ..operation(None)
        };
        assert_eq!(
            replace_picture(
                &package,
                &main,
                &source,
                &shared_operation,
                path("shared-output")
            )
            .diagnostics[0]
                .code,
            "UNSUPPORTED_OPERATION"
        );
        fs::remove_file(shared).unwrap();
    }
}
