use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi_derive::napi;
use opensuite_docx::execute_docx_set_table_cells_formatting;
use opensuite_protocol::{
    SetTableCellsFormatting, TableCellFormattingUpdate, TableCellTextFormatting,
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
    pub target: SemanticTableCellTargetInput,
    pub fill: Option<String>,
    pub text_formatting: Option<TableCellTextFormattingInput>,
}

#[napi(object)]
pub struct SetTableCellsFormattingInput {
    pub table: TableTargetInput,
    pub updates: Vec<TableCellFormattingUpdateInput>,
    pub base_revision: Option<String>,
}

#[napi(js_name = "executeDocxSetTableCellsFormatting")]
pub fn execute_docx_set_table_cells_formatting_node(
    input: Buffer,
    operation: SetTableCellsFormattingInput,
) -> napi::Result<AsyncTask<SimpleTask<SetTableCellsFormatting>>> {
    let updates = operation
        .updates
        .into_iter()
        .map(|update| {
            Ok(TableCellFormattingUpdate {
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
