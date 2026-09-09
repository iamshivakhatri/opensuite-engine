use opensuite_opc::Package;
use opensuite_protocol::{
    CreateTable, DeletePageBreak, DeleteParagraph, DeletePicture, DeleteTable, DeleteTableColumn,
    DeleteTableRow, Diagnostic, DiagnosticSeverity, FindText, FindTextResult, InsertPageBreak,
    InsertParagraph, InsertParagraphs, InsertPicture, InsertTableColumnAfter, InsertTableRowAfter,
    InsertTableRowsAfter, InspectDocx, InspectDocxResult, InspectTextContext,
    InspectTextContextResult, OperationResult, ReplaceText, SetHyperlink, SetPageSetup,
    SetParagraphFormatting, SetParagraphStyle, SetParagraphsList, SetPictureSize,
    SetTableCellsText, SetTableFormatting, SetTextFormatting,
};

use crate::{
    create_table_to_vec, delete_page_break_to_vec, delete_paragraph_to_vec, delete_picture_to_vec,
    delete_table_column_to_vec, delete_table_row_to_vec, delete_table_to_vec,
    insert_page_break_to_vec, insert_paragraph_to_vec, insert_paragraphs_to_vec,
    insert_picture_to_vec, insert_table_column_after_to_vec, insert_table_row_after_to_vec,
    insert_table_rows_after_to_vec, open_main_source, replace_text_to_vec, set_hyperlink_to_vec,
    set_page_setup_to_vec, set_paragraph_formatting_to_vec, set_paragraph_style_to_vec,
    set_paragraphs_list_to_vec, set_picture_size_to_vec, set_table_cells_text_to_vec,
    set_table_formatting_to_vec, set_text_formatting_to_vec,
};

/// The result of executing one DOCX operation against an immutable artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxExecutionResult {
    pub operation: OperationResult,
    pub output_artifact: Option<Vec<u8>>,
}

/// Executes `InsertParagraph` against owned DOCX bytes and returns verified output bytes.
pub fn execute_docx_insert_paragraph(
    input_artifact: Vec<u8>,
    operation: &InsertParagraph,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match insert_paragraph_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::paragraph_inserted(String::new(), operation.text.clone()),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(
                error,
                "insert_paragraph",
                placement_handle(&operation.placement),
            ),
            output_artifact: None,
        },
    }
}

pub fn execute_docx_insert_page_break(
    input_artifact: Vec<u8>,
    operation: &InsertPageBreak,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match insert_page_break_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::applied(String::new(), String::new()),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(
                error,
                "insert_page_break",
                placement_handle(&operation.placement),
            ),
            output_artifact: None,
        },
    }
}

pub fn execute_docx_delete_page_break(
    input_artifact: Vec<u8>,
    operation: &DeletePageBreak,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match delete_page_break_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::applied(String::new(), String::new()),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(
                error,
                "delete_page_break",
                Some(&operation.target.handle),
            ),
            output_artifact: None,
        },
    }
}

/// Executes `SetPageSetup` against owned DOCX bytes and returns verified output bytes.
pub fn execute_docx_set_page_setup(
    input_artifact: Vec<u8>,
    operation: &SetPageSetup,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match set_page_setup_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::applied(String::new(), String::new()),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(error, "set_page_setup", None),
            output_artifact: None,
        },
    }
}

/// Executes atomic `InsertParagraphs` against owned DOCX bytes.
pub fn execute_docx_insert_paragraphs(
    input_artifact: Vec<u8>,
    operation: &InsertParagraphs,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match insert_paragraphs_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::paragraph_inserted(
                String::new(),
                operation.texts.join("\n"),
            ),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(
                error,
                "insert_paragraphs",
                placement_handle(&operation.placement),
            ),
            output_artifact: None,
        },
    }
}

/// Executes `InsertPicture` against owned DOCX bytes and returns verified output bytes.
pub fn execute_docx_insert_picture(
    input_artifact: Vec<u8>,
    operation: &InsertPicture,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match insert_picture_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::picture_inserted(),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(
                error,
                "insert_picture",
                placement_handle(&operation.placement),
            ),
            output_artifact: None,
        },
    }
}

pub fn execute_docx_delete_picture(
    input_artifact: Vec<u8>,
    operation: &DeletePicture,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match delete_picture_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::picture_deleted(),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(
                error,
                "delete_picture",
                operation.target.handle.as_deref(),
            ),
            output_artifact: None,
        },
    }
}

pub fn execute_docx_set_picture_size(
    input_artifact: Vec<u8>,
    operation: &SetPictureSize,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match set_picture_size_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::picture_resized(),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(
                error,
                "set_picture_size",
                operation.target.handle.as_deref(),
            ),
            output_artifact: None,
        },
    }
}

fn placement_handle(placement: &opensuite_protocol::ParagraphPlacement) -> Option<&str> {
    match placement {
        opensuite_protocol::ParagraphPlacement::Before { handle }
        | opensuite_protocol::ParagraphPlacement::After { handle } => Some(handle),
        opensuite_protocol::ParagraphPlacement::Start
        | opensuite_protocol::ParagraphPlacement::End => None,
    }
}

macro_rules! execute_paragraph_mutation {
    ($name:ident, $operation:ty, $apply:ident, $id:literal) => {
        pub fn $name(input_artifact: Vec<u8>, operation: &$operation) -> DocxExecutionResult {
            let package = match Package::from_bytes(input_artifact) {
                Ok(value) => value,
                Err(error) => return failed(error.code(), "could not load DOCX artifact"),
            };
            let (main, source) = match open_main_source(&package) {
                Ok(value) => value,
                Err(error) => return failed(error.code(), "could not load DOCX artifact"),
            };
            match $apply(&package, &main, &source, operation) {
                Ok(output_artifact) => DocxExecutionResult {
                    operation: OperationResult::applied(String::new(), String::new()),
                    output_artifact: Some(output_artifact),
                },
                Err(error) => DocxExecutionResult {
                    operation: structured_failure(error, $id, None),
                    output_artifact: None,
                },
            }
        }
    };
}
execute_paragraph_mutation!(
    execute_docx_delete_paragraph,
    DeleteParagraph,
    delete_paragraph_to_vec,
    "delete_paragraph"
);
execute_paragraph_mutation!(
    execute_docx_set_paragraph_formatting,
    SetParagraphFormatting,
    set_paragraph_formatting_to_vec,
    "set_paragraph_formatting"
);
execute_paragraph_mutation!(
    execute_docx_set_paragraph_style,
    SetParagraphStyle,
    set_paragraph_style_to_vec,
    "set_paragraph_style"
);
execute_paragraph_mutation!(
    execute_docx_set_paragraphs_list,
    SetParagraphsList,
    set_paragraphs_list_to_vec,
    "set_paragraphs_list"
);
execute_paragraph_mutation!(
    execute_docx_set_text_formatting,
    SetTextFormatting,
    set_text_formatting_to_vec,
    "set_text_formatting"
);
execute_paragraph_mutation!(
    execute_docx_set_hyperlink,
    SetHyperlink,
    set_hyperlink_to_vec,
    "set_hyperlink"
);

macro_rules! execute_table_mutation {
    ($name:ident, $operation:ty, $apply:ident, $id:literal, $target:expr) => {
        pub fn $name(input_artifact: Vec<u8>, operation: &$operation) -> DocxExecutionResult {
            let package = match Package::from_bytes(input_artifact) {
                Ok(value) => value,
                Err(error) => return failed(error.code(), "could not load DOCX artifact"),
            };
            let (main, source) = match open_main_source(&package) {
                Ok(value) => value,
                Err(error) => return failed(error.code(), "could not load DOCX artifact"),
            };
            match $apply(&package, &main, &source, operation) {
                Ok(output_artifact) => DocxExecutionResult {
                    operation: OperationResult::applied(String::new(), String::new()),
                    output_artifact: Some(output_artifact),
                },
                Err(error) => {
                    let target = $target(operation);
                    DocxExecutionResult {
                        operation: structured_failure(error, $id, target.as_deref()),
                        output_artifact: None,
                    }
                }
            }
        }
    };
}
execute_table_mutation!(
    execute_docx_create_table,
    CreateTable,
    create_table_to_vec,
    "create_table",
    |operation: &CreateTable| placement_handle(&operation.placement).map(str::to_owned)
);
execute_table_mutation!(
    execute_docx_delete_table,
    DeleteTable,
    delete_table_to_vec,
    "delete_table",
    |operation: &DeleteTable| operation.table.handle.clone()
);
execute_table_mutation!(
    execute_docx_delete_table_row,
    DeleteTableRow,
    delete_table_row_to_vec,
    "delete_table_row",
    |operation: &DeleteTableRow| operation
        .row
        .handle
        .clone()
        .or(operation.table.handle.clone())
);
execute_table_mutation!(
    execute_docx_delete_table_column,
    DeleteTableColumn,
    delete_table_column_to_vec,
    "delete_table_column",
    |operation: &DeleteTableColumn| operation
        .column_handle
        .clone()
        .or(operation.table.handle.clone())
);
execute_table_mutation!(
    execute_docx_set_table_formatting,
    SetTableFormatting,
    set_table_formatting_to_vec,
    "set_table_formatting",
    |operation: &SetTableFormatting| operation.table.handle.clone()
);

/// Executes `ReplaceText` against owned DOCX bytes and returns verified output bytes on success.
pub fn execute_docx_replace_text(
    input_artifact: Vec<u8>,
    operation: &ReplaceText,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match replace_text_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::applied(
                operation.expected_current_text.clone(),
                operation.replacement.clone(),
            ),
            output_artifact: Some(output_artifact),
        },
        Err(mut operation) => {
            for diagnostic in &mut operation.diagnostics {
                if diagnostic.code == "DOCUMENT_INVALID" {
                    diagnostic.message = "output DOCX artifact failed validation".to_owned();
                }
            }
            DocxExecutionResult {
                operation: structured_failure(operation, "replace_text", None),
                output_artifact: None,
            }
        }
    }
}

/// Executes `InsertTableRowAfter` against owned DOCX bytes and returns verified output bytes.
pub fn execute_docx_insert_table_row(
    input_artifact: Vec<u8>,
    operation: &InsertTableRowAfter,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match insert_table_row_after_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::table_row_inserted(
                operation.table.header_cells.clone(),
                operation.after.first_cell_text.clone(),
                operation.cells.clone(),
            ),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(
                error,
                "insert_table_row",
                operation
                    .after
                    .handle
                    .as_deref()
                    .or(operation.table.handle.as_deref()),
            ),
            output_artifact: None,
        },
    }
}

/// Executes `InsertTableRowsAfter` against owned DOCX bytes and returns verified output bytes.
pub fn execute_docx_insert_table_rows(
    input_artifact: Vec<u8>,
    operation: &InsertTableRowsAfter,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match insert_table_rows_after_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::table_rows_inserted(
                operation.table.header_cells.clone(),
                operation.after.first_cell_text.clone(),
                operation.rows.clone(),
            ),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(
                error,
                "insert_table_rows",
                operation
                    .after
                    .handle
                    .as_deref()
                    .or(operation.table.handle.as_deref()),
            ),
            output_artifact: None,
        },
    }
}

/// Executes `InsertTableColumnAfter` against owned DOCX bytes and returns verified output bytes.
pub fn execute_docx_insert_table_column(
    input_artifact: Vec<u8>,
    operation: &InsertTableColumnAfter,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match insert_table_column_after_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::table_column_inserted(
                operation.table.header_cells.clone(),
                operation.after_column_header.clone(),
                operation.header.clone(),
            ),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(
                error,
                "insert_table_column",
                operation
                    .after_column_handle
                    .as_deref()
                    .or(operation.table.handle.as_deref()),
            ),
            output_artifact: None,
        },
    }
}

/// Executes `SetTableCellsText` against owned DOCX bytes and returns verified output bytes.
pub fn execute_docx_set_table_cells_text(
    input_artifact: Vec<u8>,
    operation: &SetTableCellsText,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match set_table_cells_text_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::table_cells_text_set(operation.updates.len()),
            output_artifact: Some(output_artifact),
        },
        Err(error) => DocxExecutionResult {
            operation: structured_failure(
                error,
                "set_table_cells_text",
                operation
                    .updates
                    .first()
                    .and_then(|update| update.target.handle.as_deref())
                    .or(operation.table.handle.as_deref()),
            ),
            output_artifact: None,
        },
    }
}

/// Finds exact Current-view text in an immutable DOCX artifact.
pub fn find_docx_text(input_artifact: Vec<u8>, request: &FindText) -> FindTextResult {
    let source = match source_from_artifact(input_artifact) {
        Ok(source) => source,
        Err(code) => return failed_find(request, code),
    };
    crate::find_text(&source, request).unwrap_or_else(|error| failed_find(request, error.code()))
}

/// Inspects an immutable DOCX artifact through a typed, bounded semantic request.
pub fn inspect_docx(input_artifact: Vec<u8>, request: &InspectDocx) -> InspectDocxResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => {
            return InspectDocxResult::failed(error.code(), "could not load DOCX artifact");
        }
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => {
            return InspectDocxResult::failed(error.code(), "could not load DOCX artifact");
        }
    };
    crate::inspect_docx_document(&package, &main, &source, request)
}

/// Inspects bounded text context in an immutable DOCX artifact.
pub fn inspect_docx_context(
    input_artifact: Vec<u8>,
    request: &InspectTextContext,
) -> InspectTextContextResult {
    let source = match source_from_artifact(input_artifact) {
        Ok(source) => source,
        Err(code) => {
            return InspectTextContextResult::failed(
                request.target.clone(),
                code,
                "could not load DOCX artifact",
            );
        }
    };
    crate::inspect_text_context(&source, request).unwrap_or_else(|error| {
        InspectTextContextResult::failed(
            request.target.clone(),
            error.code(),
            "could not inspect DOCX artifact",
        )
    })
}

fn source_from_artifact(input_artifact: Vec<u8>) -> Result<crate::SourceDocument, &'static str> {
    let package = Package::from_bytes(input_artifact).map_err(|error| error.code())?;
    let (_, source) = open_main_source(&package).map_err(|error| error.code())?;
    Ok(source)
}

fn failed_find(request: &FindText, code: impl Into<String>) -> FindTextResult {
    FindTextResult {
        query: request.text.clone(),
        matches: Vec::new(),
        diagnostics: vec![Diagnostic::new(
            code,
            DiagnosticSeverity::Error,
            "could not load DOCX artifact",
        )],
    }
}

fn failed(code: impl Into<String>, message: impl Into<String>) -> DocxExecutionResult {
    DocxExecutionResult {
        operation: OperationResult::failed(code, message),
        output_artifact: None,
    }
}

fn structured_failure(
    mut result: OperationResult,
    operation: &str,
    target_handle: Option<&str>,
) -> OperationResult {
    result = result.with_operation(operation);
    if let Some(handle) = target_handle {
        result = result.with_target_handle(handle);
    }
    for diagnostic in &mut result.diagnostics {
        if diagnostic.reason_code.is_none()
            && matches!(
                diagnostic.code.as_str(),
                "TARGET_NOT_FOUND" | "TARGET_AMBIGUOUS"
            )
        {
            diagnostic.reason_code = Some(diagnostic.code.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read, Write};

    use opensuite_protocol::{
        InspectDocxContent, InspectDocxFocus, OperationStatus, ParagraphPlacement, TextTarget,
    };
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

    use super::*;

    const OFFICE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    fn fixture() -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer
            .write_all(
                b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>",
            )
            .unwrap();
        writer.start_file("_rels/.rels", options).unwrap();
        writer.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"word/document.xml\"/></Relationships>").as_bytes()).unwrap();
        writer.start_file("word/document.xml", options).unwrap();
        writer.write_all(format!("<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>old text</w:t></w:r></w:p><w:p><w:r><w:t>duplicate</w:t></w:r><w:r><w:t>duplicate</w:t></w:r></w:p></w:body></w:document>").as_bytes()).unwrap();
        writer.start_file("word/media/image.bin", options).unwrap();
        writer.write_all(b"unchanged image").unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn table_fixture() -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer
            .write_all(
                b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>",
            )
            .unwrap();
        writer.start_file("_rels/.rels", options).unwrap();
        writer.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"word/document.xml\"/></Relationships>").as_bytes()).unwrap();
        writer.start_file("word/document.xml", options).unwrap();
        writer.write_all(format!("<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:sectPr/></w:body></w:document>").as_bytes()).unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn operation(target: &str, expected: &str) -> ReplaceText {
        ReplaceText {
            target: TextTarget {
                text: target.to_owned(),
                occurrence: None,
            },
            expected_current_text: expected.to_owned(),
            replacement: "new text".to_owned(),
            base_revision: Some("application-owned-version".to_owned()),
        }
    }

    fn entry(bytes: &[u8], name: &str) -> Vec<u8> {
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut entry = archive.by_name(name).unwrap();
        let mut value = Vec::new();
        entry.read_to_end(&mut value).unwrap();
        value
    }

    #[test]
    fn returns_verified_output_bytes_for_replace_text() {
        let input = fixture();
        let result = execute_docx_replace_text(input.clone(), &operation("old text", "old text"));

        assert_eq!(result.operation.status, OperationStatus::Applied);
        let output = result.output_artifact.unwrap();
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        let package = Package::from_bytes(output).unwrap();
        package.verify().unwrap();
        let (_, source) = open_main_source(&package).unwrap();
        assert_eq!(
            crate::DocxDocument::new(&source)
                .unwrap()
                .paragraphs()
                .next()
                .unwrap()
                .text()
                .unwrap(),
            "new text"
        );
    }

    #[test]
    fn returns_no_output_for_unresolved_or_ambiguous_targets() {
        for request in [
            operation("missing", "missing"),
            operation("duplicate", "duplicate"),
        ] {
            let result = execute_docx_replace_text(fixture(), &request);
            assert_eq!(result.operation.status, OperationStatus::Failed);
            assert!(result.output_artifact.is_none());
        }
    }

    #[test]
    fn returns_no_output_for_invalid_input_or_failed_precondition() {
        let invalid =
            execute_docx_replace_text(b"not a zip".to_vec(), &operation("old text", "old text"));
        assert_eq!(invalid.operation.diagnostics[0].code, "INVALID_ZIP");
        assert!(invalid.output_artifact.is_none());

        let stale = execute_docx_replace_text(fixture(), &operation("old text", "wrong text"));
        assert_eq!(stale.operation.diagnostics[0].code, "PRECONDITION_FAILED");
        assert!(stale.output_artifact.is_none());
    }

    #[test]
    fn blank_document_supports_ordered_paragraph_authoring() {
        let mut artifact = crate::create_blank_docx();
        for (text, placement) in [
            ("Heading", ParagraphPlacement::End),
            (
                "First",
                ParagraphPlacement::After {
                    handle: "b0".to_owned(),
                },
            ),
            (
                "Middle",
                ParagraphPlacement::Before {
                    handle: "b1".to_owned(),
                },
            ),
            ("Start", ParagraphPlacement::Start),
        ] {
            let result = execute_docx_insert_paragraph(
                artifact,
                &InsertParagraph {
                    text: text.to_owned(),
                    placement,
                    base_revision: None,
                },
            );
            assert_eq!(result.operation.status, OperationStatus::Applied);
            artifact = result.output_artifact.unwrap();
        }
        let result = inspect_docx(
            artifact.clone(),
            &InspectDocx {
                focus: InspectDocxFocus::BodyBlocks {
                    offset: 0,
                    limit: 20,
                },
            },
        );
        let Some(InspectDocxContent::BodyBlocks(page)) = result.content else {
            panic!("expected body blocks")
        };
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.text.as_deref())
                .collect::<Vec<_>>(),
            [
                Some("Start"),
                Some("Heading"),
                Some("Middle"),
                Some("First")
            ]
        );
        Package::from_bytes(artifact).unwrap().verify().unwrap();
    }

    #[test]
    fn inserts_a_paragraph_before_a_table_without_rewriting_it() {
        let input = table_fixture();
        let result = execute_docx_insert_paragraph(
            input.clone(),
            &InsertParagraph {
                text: "Above table".to_owned(),
                placement: ParagraphPlacement::Before {
                    handle: "b0".to_owned(),
                },
                base_revision: None,
            },
        );
        let output = result.output_artifact.unwrap();
        let inspected = inspect_docx(
            output,
            &InspectDocx {
                focus: InspectDocxFocus::BodyBlocks {
                    offset: 0,
                    limit: 20,
                },
            },
        );
        let Some(InspectDocxContent::BodyBlocks(page)) = inspected.content else {
            panic!("expected body blocks")
        };
        assert_eq!(page.items[0].text.as_deref(), Some("Above table"));
        assert_eq!(page.items[1].table_handle.as_deref(), Some("t0"));
        assert!(
            entry(&input, "word/document.xml")
                .windows(b"<w:tbl>".len())
                .any(|value| value == b"<w:tbl>")
        );
    }
}
