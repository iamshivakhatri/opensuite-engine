use crate::{SimpleTask, TableRowTargetInput, row_target};
use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi_derive::napi;
use opensuite_docx::{execute_docx_set_table_row_layout, execute_docx_set_table_spacing};
use opensuite_protocol::{SetTableRowLayout, SetTableSpacing, TableLayoutTarget};

#[napi(object)]
pub struct TableLayoutTargetInput {
    /// Fresh tables.items[].layoutHandle from inspectDocx; re-inspect after any edit.
    pub handle: String,
}

#[napi(object)]
pub struct SetTableSpacingInput {
    pub before: TableLayoutTargetInput,
    pub after: TableLayoutTargetInput,
    /// @schema {"minimum":0.05,"maximum":1584,"multipleOf":0.05}
    pub spacing_points: f64,
    pub base_revision: Option<String>,
}

/// Set an exact paragraph gap in points between two adjacent body tables (8 points = 160 twips). Reuses only an existing engine-authored separator; refuses arbitrary intervening content. Requires fresh layout handles for both tables. Preserves intentional adjacency until explicitly called.
#[napi(
    js_name = "executeDocxSetTableSpacing",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn set_table_spacing(
    input: Buffer,
    operation: SetTableSpacingInput,
) -> AsyncTask<SimpleTask<SetTableSpacing>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetTableSpacing {
            before: TableLayoutTarget {
                handle: operation.before.handle,
            },
            after: TableLayoutTarget {
                handle: operation.after.handle,
            },
            spacing_points: operation.spacing_points,
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_table_spacing,
    })
}

#[napi(object)]
pub struct SetTableRowLayoutInput {
    pub table: TableLayoutTargetInput,
    /// Count of consecutive leading header rows; zero clears repetition. Omit to preserve headers.
    pub repeating_header_rows: Option<u32>,
    /// Existing row handle or exact data-row firstCellText selector, required with preventSplit.
    pub row: Option<TableRowTargetInput>,
    /// True prevents row splitting; false explicitly allows it. Oversized rows may still span pages.
    pub prevent_split: Option<bool>,
    pub base_revision: Option<String>,
}

/// Set the number of repeating leading table header rows and/or prevent a selected row from splitting across pages. Requires a fresh table layoutHandle; row handles must belong to that table. Omitted settings stay unchanged. Refuses merged, nested, floating, wrapped, protected, or ambiguous table structures. Pagination is determined by Word-compatible renderers.
#[napi(
    js_name = "executeDocxSetTableRowLayout",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn set_table_row_layout(
    input: Buffer,
    operation: SetTableRowLayoutInput,
) -> AsyncTask<SimpleTask<SetTableRowLayout>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetTableRowLayout {
            table: TableLayoutTarget {
                handle: operation.table.handle,
            },
            repeating_header_rows: operation.repeating_header_rows,
            row: operation.row.map(row_target),
            prevent_split: operation.prevent_split,
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_table_row_layout,
    })
}
