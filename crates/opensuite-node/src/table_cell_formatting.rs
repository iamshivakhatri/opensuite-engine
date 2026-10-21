use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi_derive::napi;
use opensuite_docx::execute_docx_set_table_cells_formatting;
use opensuite_protocol::{
    CellVerticalAlignment, SetTableCellsFormatting, TableAlignment, TableCellFormattingUpdate,
    TableCellTextFormatting,
};

use crate::{
    SemanticTableCellTargetInput, SimpleTask, TableTargetInput, semantic_cell_target, table_target,
};

#[napi(object)]
pub struct TableCellTextFormattingInput {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub font_family: Option<String>,
    pub font_size_half_points: Option<u32>,
    pub color: Option<String>,
}

#[napi(object)]
pub struct TableCellFormattingUpdateInput {
    /// @schema {"description":"Prefer row + column. Use a handle only when these selectors cannot express the cell. Legacy rowLabel + columnHeader remains supported for data cells."}
    pub target: SemanticTableCellTargetInput,
    /// Applies to every direct paragraph, including empty paragraphs. Omission preserves alignment.
    /// @schema {"enum":["left","center","right"]}
    pub alignment: Option<String>,
    /// @schema {"enum":["top","center","bottom"]}
    pub vertical_alignment: Option<String>,
    /// Remove direct alignment and restore inheritance; omitted properties stay unchanged.
    /// @schema {"items":{"type":"string","enum":["alignment","verticalAlignment"]}}
    pub clear: Option<Vec<String>>,
    pub fill: Option<String>,
    pub text_formatting: Option<TableCellTextFormattingInput>,
}

#[napi(object)]
pub struct SetTableCellsFormattingInput {
    /// @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."}
    pub table: TableTargetInput,
    /// @schema {"minItems":1,"maxItems":100}
    pub updates: Vec<TableCellFormattingUpdateInput>,
    pub base_revision: Option<String>,
}

/// Format known whole table cells using table + row + column, including header and first-column cells, without find or inspect handles. Format all relevant cells in one call; set fill and direct text formatting together when needed. Alignment applies to every direct cell paragraph; verticalAlignment applies to the cell. clear: ["alignment", "verticalAlignment"] restores inheritance. Simple multi-paragraph cells are supported when alignment is requested. Supports fill (6-digit RGB), bold, italic, font family, font size in half-points, and text color. Handles remain available for cells that need them.
#[napi(
    js_name = "executeDocxSetTableCellsFormatting",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_table_cells_formatting_node(
    input: Buffer,
    operation: SetTableCellsFormattingInput,
) -> napi::Result<AsyncTask<SimpleTask<SetTableCellsFormatting>>> {
    let updates = operation
        .updates
        .into_iter()
        .map(|update| {
            let clear = update.clear.unwrap_or_default();
            if clear
                .iter()
                .any(|key| !["alignment", "verticalAlignment"].contains(&key.as_str()))
            {
                return Err(napi::Error::from_reason(
                    "unsupported cell property to clear",
                ));
            }
            let alignment = update
                .alignment
                .map(|value| match value.as_str() {
                    "left" => Ok(TableAlignment::Left),
                    "center" => Ok(TableAlignment::Center),
                    "right" => Ok(TableAlignment::Right),
                    _ => Err(napi::Error::from_reason("invalid cell paragraph alignment")),
                })
                .transpose()?;
            let vertical = update
                .vertical_alignment
                .map(|value| match value.as_str() {
                    "top" => Ok(CellVerticalAlignment::Top),
                    "center" => Ok(CellVerticalAlignment::Center),
                    "bottom" => Ok(CellVerticalAlignment::Bottom),
                    _ => Err(napi::Error::from_reason("invalid cell vertical alignment")),
                })
                .transpose()?;
            Ok(TableCellFormattingUpdate {
                alignment: crate::styles::property(alignment, &clear, "alignment"),
                vertical_alignment: crate::styles::property(vertical, &clear, "verticalAlignment"),
                target: semantic_cell_target(update.target)?,
                fill: update.fill,
                text_formatting: update
                    .text_formatting
                    .map(|formatting| TableCellTextFormatting {
                        bold: formatting.bold,
                        italic: formatting.italic,
                        font_family: formatting.font_family,
                        font_size_half_points: formatting.font_size_half_points,
                        color: formatting.color,
                    }),
            })
        })
        .collect::<napi::Result<Vec<_>>>()?;
    Ok(AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetTableCellsFormatting {
            table: table_target(operation.table),
            updates,
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_table_cells_formatting,
    }))
}
