use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
mod page_composition;
use opensuite_docx::{
    DocxExecutionResult, create_blank_docx, execute_docx_create_table,
    execute_docx_delete_paragraph, execute_docx_delete_table, execute_docx_delete_table_column,
    execute_docx_delete_table_row, execute_docx_insert_paragraph, execute_docx_insert_paragraphs,
    execute_docx_insert_table_column, execute_docx_insert_table_row,
    execute_docx_insert_table_rows, execute_docx_replace_text,
    execute_docx_set_paragraph_formatting, execute_docx_set_paragraph_style,
    execute_docx_set_table_cells_text, execute_docx_set_table_formatting,
    execute_docx_set_text_formatting, find_docx_text, inspect_docx,
};
use opensuite_protocol::{
    Affordance, CreateTable, DeleteTable, DeleteTableColumn, DeleteTableRow, Diagnostic,
    DocxBodyBlock, DocxHeading, DocxOverview, DocxParagraph, DocxTable, DocxTableRow, FindText,
    FindTextResult, InsertParagraph, InsertParagraphs, InsertTableColumnAfter, InspectDocx,
    InspectDocxContent, InspectDocxFocus, InspectDocxResult, InspectTextContext,
    InspectTextContextResult, InspectionPage, OperationResult, ParagraphAlignment,
    ParagraphFormattingPatch, ParagraphPlacement, PropertyPatch, ReplaceText, RuntimeCapabilities,
    SetParagraphFormatting, SetParagraphStyle, SetTableFormatting, SetTextFormatting,
    TableAlignment, TableBorders, TableCellMargins, TableCellTarget, TableCellTextUpdate,
    TableFormattingPatch, TableRowTarget, TableTarget, TextContainer, TextFormattingPatch,
    TextTarget,
};
pub use page_composition::{
    execute_docx_delete_page_break_node, execute_docx_insert_page_break_node,
    execute_docx_set_header_footer_text_node, execute_docx_set_page_number_node,
    execute_docx_set_page_setup_node,
};

#[napi(object)]
pub struct TextTargetInput {
    pub text: String,
    pub occurrence: Option<u32>,
}

#[napi(object)]
pub struct ReplaceTextInput {
    pub target: TextTargetInput,
    pub expected_current_text: String,
    pub replacement: String,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct InsertParagraphInput {
    pub text: String,
    pub placement: ParagraphPlacementInput,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct InsertParagraphsInput {
    pub texts: Vec<String>,
    pub placement: ParagraphPlacementInput,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct ParagraphPlacementInput {
    pub kind: String,
    pub handle: Option<String>,
}
#[napi(object)]
pub struct DeleteParagraphInput {
    pub target: TextTargetInput,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct SetParagraphStyleInput {
    pub target: TextTargetInput,
    pub style: Option<String>,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct SetParagraphFormattingInput {
    pub target: TextTargetInput,
    pub alignment: Option<String>,
    pub spacing_before_twips: Option<i32>,
    pub spacing_after_twips: Option<i32>,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct SetTextFormattingInput {
    pub target: TextTargetInput,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub font_size_half_points: Option<u32>,
    pub font_family: Option<String>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct DiagnosticOutput {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub reason_code: Option<String>,
    pub operation: Option<String>,
    pub target_handle: Option<String>,
}

#[napi(object)]
pub struct OperationChangeOutput {
    pub kind: String,
    pub before: String,
    pub after: String,
}

#[napi(object)]
pub struct OperationResultOutput {
    pub ok: bool,
    pub status: String,
    pub diagnostics: Vec<DiagnosticOutput>,
    pub changes: Vec<OperationChangeOutput>,
}

#[napi(object)]
pub struct ExecuteDocxReplaceTextOutput {
    pub result: OperationResultOutput,
    pub output: Option<Buffer>,
}

#[napi(object)]
pub struct TableTargetInput {
    pub header_cells: Option<Vec<String>>,
    pub occurrence: Option<u32>,
    pub handle: Option<String>,
}

#[napi(object)]
pub struct TableRowTargetInput {
    pub first_cell_text: Option<String>,
    pub occurrence: Option<u32>,
    pub handle: Option<String>,
}

#[napi(object)]
pub struct InsertTableRowInput {
    pub table: TableTargetInput,
    pub after: TableRowTargetInput,
    pub cells: Vec<String>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct InsertTableRowsInput {
    pub table: TableTargetInput,
    pub after: TableRowTargetInput,
    pub rows: Vec<Vec<String>>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct InsertTableColumnInput {
    pub table: TableTargetInput,
    pub after_column_header: Option<String>,
    pub after_column_handle: Option<String>,
    pub header: String,
    pub cells: Vec<String>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct TableCellTargetInput {
    pub row_label: Option<String>,
    pub column_header: Option<String>,
    pub occurrence: Option<u32>,
    pub handle: Option<String>,
}

#[napi(object)]
pub struct TableCellTextUpdateInput {
    pub target: TableCellTargetInput,
    pub expected_current_text: String,
    pub replacement: String,
}

#[napi(object)]
pub struct SetTableCellsTextInput {
    pub table: TableTargetInput,
    pub updates: Vec<TableCellTextUpdateInput>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct CreateTableInput {
    pub rows: Vec<Vec<String>>,
    pub placement: ParagraphPlacementInput,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct DeleteTableInput {
    pub table: TableTargetInput,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct DeleteTableRowInput {
    pub table: TableTargetInput,
    pub row: TableRowTargetInput,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct DeleteTableColumnInput {
    pub table: TableTargetInput,
    pub column_header: Option<String>,
    pub column_handle: Option<String>,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct SetTableFormattingInput {
    pub table: TableTargetInput,
    pub alignment: Option<String>,
    pub cell_margin_top_twips: Option<u32>,
    pub cell_margin_right_twips: Option<u32>,
    pub cell_margin_bottom_twips: Option<u32>,
    pub cell_margin_left_twips: Option<u32>,
    pub borders: Option<String>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct FormatCapabilitiesOutput {
    pub format: String,
    pub capabilities: Vec<String>,
}

#[napi(object)]
pub struct RuntimeCapabilitiesOutput {
    pub ok: bool,
    pub protocol_version: u32,
    pub engine_version: String,
    pub formats: Vec<FormatCapabilitiesOutput>,
}

#[napi(object)]
pub struct FindTextInput {
    pub text: String,
}

#[napi(object)]
pub struct FindTextMatchOutput {
    pub occurrence: u32,
    pub text: String,
    pub before: String,
    pub after: String,
    pub container: String,
}

#[napi(object)]
pub struct FindTextOutput {
    pub ok: bool,
    pub query: String,
    pub match_count: u32,
    pub matches: Vec<FindTextMatchOutput>,
    pub diagnostics: Vec<DiagnosticOutput>,
}

#[napi(object)]
pub struct InspectDocxFocusInput {
    pub kind: String,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
    pub text: Option<String>,
    pub occurrence: Option<u32>,
    pub before: Option<u32>,
    pub after: Option<u32>,
}

#[napi(object)]
pub struct InspectDocxInput {
    pub focus: Option<InspectDocxFocusInput>,
    /// Compatibility input for the original context-only binding.
    pub target: Option<TextTargetInput>,
    pub before: Option<u32>,
    pub after: Option<u32>,
}

#[napi(object)]
pub struct OverviewOutput {
    pub body_block_count: u32,
    pub paragraph_count: u32,
    pub table_count: u32,
    pub section_count: u32,
}

#[napi(object)]
pub struct HeadingOutput {
    pub occurrence: u32,
    pub text: String,
    pub style_name: String,
    pub level: Option<u32>,
}

#[napi(object)]
pub struct ParagraphOutput {
    pub occurrence: u32,
    pub handle: Option<String>,
    pub text: String,
    pub style_name: Option<String>,
}

#[napi(object)]
pub struct BodyBlockOutput {
    pub handle: String,
    pub kind: String,
    pub text: Option<String>,
    pub table_handle: Option<String>,
}

#[napi(object)]
pub struct BodyBlockPageOutput {
    pub page: InspectionPageOutput,
    pub items: Vec<BodyBlockOutput>,
}

#[napi(object)]
pub struct TableRowOutput {
    pub handle: String,
    pub cells: Vec<String>,
    pub cell_handles: Vec<String>,
    pub cell_affordances: Vec<Vec<AffordanceOutput>>,
}

#[napi(object)]
pub struct AffordanceOutput {
    pub capability: String,
    pub supported: bool,
    pub reason: Option<String>,
}

#[napi(object)]
pub struct TableColumnOutput {
    pub occurrence: u32,
    pub handle: String,
    pub text: String,
}

#[napi(object)]
pub struct TableOutput {
    pub occurrence: u32,
    pub handle: String,
    pub row_count: u32,
    pub is_rectangular: bool,
    pub affordances: Vec<AffordanceOutput>,
    pub columns: Vec<TableColumnOutput>,
    pub rows: Vec<TableRowOutput>,
}

#[napi(object)]
pub struct InspectionPageOutput {
    pub total: u32,
    pub offset: u32,
    pub returned: u32,
    pub has_more: bool,
}

#[napi(object)]
pub struct HeadingPageOutput {
    pub page: InspectionPageOutput,
    pub items: Vec<HeadingOutput>,
}

#[napi(object)]
pub struct ParagraphPageOutput {
    pub page: InspectionPageOutput,
    pub items: Vec<ParagraphOutput>,
}

#[napi(object)]
pub struct TablePageOutput {
    pub page: InspectionPageOutput,
    pub items: Vec<TableOutput>,
}

#[napi(object)]
pub struct TextContextContainerOutput {
    pub relative_position: i32,
    pub text: String,
    pub container: String,
}

#[napi(object)]
pub struct InspectDocxOutput {
    pub ok: bool,
    pub focus: String,
    pub overview: Option<OverviewOutput>,
    pub body_blocks: Option<BodyBlockPageOutput>,
    pub headings: Option<HeadingPageOutput>,
    pub paragraphs: Option<ParagraphPageOutput>,
    pub tables: Option<TablePageOutput>,
    pub context: Option<InspectContextOutput>,
    pub diagnostics: Vec<DiagnosticOutput>,
}

#[napi(object)]
pub struct InspectContextOutput {
    pub target: TextTargetInput,
    pub container: Option<TextContextContainerOutput>,
    pub nearby: Vec<TextContextContainerOutput>,
}

/// Returns the static DOCX runtime manifest without requiring artifact bytes.
#[napi(js_name = "getDocxCapabilities")]
pub fn get_docx_capabilities() -> RuntimeCapabilitiesOutput {
    let capabilities = RuntimeCapabilities::docx(env!("CARGO_PKG_VERSION"));
    RuntimeCapabilitiesOutput {
        ok: true,
        protocol_version: capabilities.protocol_version,
        engine_version: capabilities.engine_version,
        formats: capabilities
            .formats
            .into_iter()
            .map(|format| FormatCapabilitiesOutput {
                format: format.format.as_str().to_owned(),
                capabilities: format
                    .capabilities
                    .into_iter()
                    .map(|capability| capability.as_str().to_owned())
                    .collect(),
            })
            .collect(),
    }
}

/// Creates a valid empty DOCX without touching the filesystem.
#[napi(js_name = "createBlankDocx")]
pub fn create_blank_docx_node() -> Buffer {
    Buffer::from(create_blank_docx())
}

/// Finds DOCX text away from Node's event loop and returns semantic results.
#[napi(js_name = "findDocxText")]
pub fn find_docx_text_node(input: Buffer, request: FindTextInput) -> AsyncTask<FindTextTask> {
    AsyncTask::new(FindTextTask {
        input: input.to_vec(),
        request: FindText { text: request.text },
    })
}

pub struct FindTextTask {
    input: Vec<u8>,
    request: FindText,
}

impl Task for FindTextTask {
    type Output = FindTextResult;
    type JsValue = FindTextOutput;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(find_docx_text(
            std::mem::take(&mut self.input),
            &self.request,
        ))
    }

    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(find_text_output(result))
    }
}

/// Inspects bounded DOCX text context away from Node's event loop.
#[napi(js_name = "inspectDocx")]
pub fn inspect_docx_node(input: Buffer, request: InspectDocxInput) -> AsyncTask<InspectDocxTask> {
    let (focus, request) = inspect_request(request);
    AsyncTask::new(InspectDocxTask {
        input: input.to_vec(),
        focus,
        request,
    })
}

pub struct InspectDocxTask {
    input: Vec<u8>,
    focus: String,
    request: std::result::Result<InspectDocx, InspectDocxResult>,
}

impl Task for InspectDocxTask {
    type Output = InspectDocxResult;
    type JsValue = InspectDocxOutput;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(match &self.request {
            Ok(request) => inspect_docx(std::mem::take(&mut self.input), request),
            Err(result) => result.clone(),
        })
    }

    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(inspect_docx_output(self.focus.clone(), result))
    }
}

/// Runs DOCX replacement away from Node's event loop and returns a Promise.
#[napi(js_name = "executeDocxReplaceText")]
pub fn execute_docx_replace_text_node(
    input: Buffer,
    operation: ReplaceTextInput,
) -> AsyncTask<ReplaceTextTask> {
    AsyncTask::new(ReplaceTextTask {
        input: input.to_vec(),
        operation: ReplaceText {
            target: text_target(operation.target),
            expected_current_text: operation.expected_current_text,
            replacement: operation.replacement,
            base_revision: operation.base_revision,
        },
    })
}

/// Runs DOCX paragraph insertion away from Node's event loop and returns a Promise.
#[napi(js_name = "executeDocxInsertParagraph")]
pub fn execute_docx_insert_paragraph_node(
    input: Buffer,
    operation: InsertParagraphInput,
) -> AsyncTask<InsertParagraphTask> {
    let placement = match operation.placement.kind.as_str() {
        "start" => Ok(ParagraphPlacement::Start),
        "end" => Ok(ParagraphPlacement::End),
        "before" => operation
            .placement
            .handle
            .map(|handle| ParagraphPlacement::Before { handle })
            .ok_or("before placement requires a body block handle"),
        "after" => operation
            .placement
            .handle
            .map(|handle| ParagraphPlacement::After { handle })
            .ok_or("after placement requires a body block handle"),
        _ => Err("paragraph placement is not supported"),
    };
    AsyncTask::new(InsertParagraphTask {
        input: input.to_vec(),
        operation: placement.map(|placement| InsertParagraph {
            text: operation.text,
            placement,
            base_revision: operation.base_revision,
        }),
    })
}

#[napi(js_name = "executeDocxInsertParagraphs")]
pub fn execute_docx_insert_paragraphs_node(
    input: Buffer,
    operation: InsertParagraphsInput,
) -> AsyncTask<InsertParagraphsTask> {
    let placement = match operation.placement.kind.as_str() {
        "start" => Ok(ParagraphPlacement::Start),
        "end" => Ok(ParagraphPlacement::End),
        "before" => operation
            .placement
            .handle
            .map(|handle| ParagraphPlacement::Before { handle })
            .ok_or("before placement requires a body block handle"),
        "after" => operation
            .placement
            .handle
            .map(|handle| ParagraphPlacement::After { handle })
            .ok_or("after placement requires a body block handle"),
        _ => Err("paragraph placement is not supported"),
    };
    AsyncTask::new(InsertParagraphsTask {
        input: input.to_vec(),
        operation: placement.map(|placement| InsertParagraphs {
            texts: operation.texts,
            placement,
            base_revision: operation.base_revision,
        }),
    })
}

#[napi(js_name = "executeDocxDeleteParagraph")]
pub fn execute_docx_delete_paragraph_node(
    input: Buffer,
    operation: DeleteParagraphInput,
) -> AsyncTask<SimpleTask<opensuite_protocol::DeleteParagraph>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: opensuite_protocol::DeleteParagraph {
            target: text_target(operation.target),
            base_revision: operation.base_revision,
        },
        run: execute_docx_delete_paragraph,
    })
}
#[napi(js_name = "executeDocxSetParagraphStyle")]
pub fn execute_docx_set_paragraph_style_node(
    input: Buffer,
    operation: SetParagraphStyleInput,
) -> AsyncTask<SimpleTask<SetParagraphStyle>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetParagraphStyle {
            target: text_target(operation.target),
            style: operation
                .style
                .map(PropertyPatch::Set)
                .unwrap_or(PropertyPatch::Clear),
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_paragraph_style,
    })
}
#[napi(js_name = "executeDocxSetParagraphFormatting")]
pub fn execute_docx_set_paragraph_formatting_node(
    input: Buffer,
    operation: SetParagraphFormattingInput,
) -> AsyncTask<SimpleTask<SetParagraphFormatting>> {
    let alignment = operation
        .alignment
        .and_then(|value| match value.as_str() {
            "left" => Some(ParagraphAlignment::Left),
            "center" => Some(ParagraphAlignment::Center),
            "right" => Some(ParagraphAlignment::Right),
            _ => None,
        })
        .map(PropertyPatch::Set);
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetParagraphFormatting {
            target: text_target(operation.target),
            formatting: ParagraphFormattingPatch {
                alignment,
                spacing_before_twips: operation.spacing_before_twips.map(PropertyPatch::Set),
                spacing_after_twips: operation.spacing_after_twips.map(PropertyPatch::Set),
                ..Default::default()
            },
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_paragraph_formatting,
    })
}
#[napi(js_name = "executeDocxSetTextFormatting")]
pub fn execute_docx_set_text_formatting_node(
    input: Buffer,
    operation: SetTextFormattingInput,
) -> AsyncTask<SimpleTask<SetTextFormatting>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetTextFormatting {
            target: text_target(operation.target),
            formatting: TextFormattingPatch {
                bold: operation.bold.map(PropertyPatch::Set),
                italic: operation.italic.map(PropertyPatch::Set),
                font_size_half_points: operation
                    .font_size_half_points
                    .map(|value| PropertyPatch::Set(value as u16)),
                font_family: operation.font_family.map(PropertyPatch::Set),
            },
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_text_formatting,
    })
}

/// Runs DOCX table-row insertion away from Node's event loop and returns a Promise.
#[napi(js_name = "executeDocxInsertTableRow")]
pub fn execute_docx_insert_table_row_node(
    input: Buffer,
    operation: InsertTableRowInput,
) -> AsyncTask<InsertTableRowTask> {
    AsyncTask::new(InsertTableRowTask {
        input: input.to_vec(),
        operation: opensuite_protocol::InsertTableRowAfter {
            table: TableTarget {
                header_cells: operation.table.header_cells.unwrap_or_default(),
                occurrence: operation.table.occurrence.map(|value| value as usize),
                handle: operation.table.handle,
            },
            after: TableRowTarget {
                first_cell_text: operation.after.first_cell_text.unwrap_or_default(),
                occurrence: operation.after.occurrence.map(|value| value as usize),
                handle: operation.after.handle,
            },
            cells: operation.cells,
            base_revision: operation.base_revision,
        },
    })
}

/// Runs DOCX multi-row insertion away from Node's event loop and returns a Promise.
#[napi(js_name = "executeDocxInsertTableRows")]
pub fn execute_docx_insert_table_rows_node(
    input: Buffer,
    operation: InsertTableRowsInput,
) -> AsyncTask<InsertTableRowsTask> {
    AsyncTask::new(InsertTableRowsTask {
        input: input.to_vec(),
        operation: opensuite_protocol::InsertTableRowsAfter {
            table: table_target(operation.table),
            after: row_target(operation.after),
            rows: operation.rows,
            base_revision: operation.base_revision,
        },
    })
}

#[napi(js_name = "executeDocxInsertTableColumn")]
pub fn execute_docx_insert_table_column_node(
    input: Buffer,
    operation: InsertTableColumnInput,
) -> AsyncTask<InsertTableColumnTask> {
    AsyncTask::new(InsertTableColumnTask {
        input: input.to_vec(),
        operation: InsertTableColumnAfter {
            table: table_target(operation.table),
            after_column_header: operation.after_column_header.unwrap_or_default(),
            after_column_handle: operation.after_column_handle,
            header: operation.header,
            cells: operation.cells,
            base_revision: operation.base_revision,
        },
    })
}

/// Runs DOCX multi-cell text replacement away from Node's event loop and returns a Promise.
#[napi(js_name = "executeDocxSetTableCellsText")]
pub fn execute_docx_set_table_cells_text_node(
    input: Buffer,
    operation: SetTableCellsTextInput,
) -> AsyncTask<SetTableCellsTextTask> {
    AsyncTask::new(SetTableCellsTextTask {
        input: input.to_vec(),
        operation: opensuite_protocol::SetTableCellsText {
            table: table_target(operation.table),
            updates: operation
                .updates
                .into_iter()
                .map(|update| TableCellTextUpdate {
                    target: TableCellTarget {
                        row_label: update.target.row_label.unwrap_or_default(),
                        column_header: update.target.column_header.unwrap_or_default(),
                        occurrence: update.target.occurrence.map(|value| value as usize),
                        handle: update.target.handle,
                    },
                    expected_current_text: update.expected_current_text,
                    replacement: update.replacement,
                })
                .collect(),
            base_revision: operation.base_revision,
        },
    })
}

#[napi(js_name = "executeDocxSetTableFormatting")]
pub fn execute_docx_set_table_formatting_node(
    input: Buffer,
    operation: SetTableFormattingInput,
) -> AsyncTask<SetTableFormattingTask> {
    let margins = [
        operation.cell_margin_top_twips,
        operation.cell_margin_right_twips,
        operation.cell_margin_bottom_twips,
        operation.cell_margin_left_twips,
    ];
    AsyncTask::new(SetTableFormattingTask {
        input: input.to_vec(),
        operation: SetTableFormatting {
            table: table_target(operation.table),
            formatting: TableFormattingPatch {
                alignment: operation.alignment.and_then(|value| match value.as_str() {
                    "left" => Some(PropertyPatch::Set(TableAlignment::Left)),
                    "center" => Some(PropertyPatch::Set(TableAlignment::Center)),
                    "right" => Some(PropertyPatch::Set(TableAlignment::Right)),
                    "clear" => Some(PropertyPatch::Clear),
                    _ => None,
                }),
                cell_margins: (margins.iter().all(Option::is_some)).then(|| {
                    PropertyPatch::Set(TableCellMargins {
                        top_twips: margins[0].unwrap() as u16,
                        right_twips: margins[1].unwrap() as u16,
                        bottom_twips: margins[2].unwrap() as u16,
                        left_twips: margins[3].unwrap() as u16,
                    })
                }),
                borders: operation.borders.and_then(|value| match value.as_str() {
                    "grid" => Some(PropertyPatch::Set(TableBorders::Grid)),
                    "none" => Some(PropertyPatch::Set(TableBorders::None)),
                    "clear" => Some(PropertyPatch::Clear),
                    _ => None,
                }),
            },
            base_revision: operation.base_revision,
        },
    })
}

#[napi(js_name = "executeDocxCreateTable")]
pub fn execute_docx_create_table_node(
    input: Buffer,
    operation: CreateTableInput,
) -> AsyncTask<CreateTableTask> {
    let placement = match operation.placement.kind.as_str() {
        "start" => ParagraphPlacement::Start,
        "before" => operation
            .placement
            .handle
            .map(|handle| ParagraphPlacement::Before { handle })
            .unwrap_or(ParagraphPlacement::End),
        "after" => operation
            .placement
            .handle
            .map(|handle| ParagraphPlacement::After { handle })
            .unwrap_or(ParagraphPlacement::End),
        _ => ParagraphPlacement::End,
    };
    AsyncTask::new(CreateTableTask {
        input: input.to_vec(),
        operation: CreateTable {
            rows: operation.rows,
            placement,
            base_revision: operation.base_revision,
        },
    })
}
#[napi(js_name = "executeDocxDeleteTable")]
pub fn execute_docx_delete_table_node(
    input: Buffer,
    operation: DeleteTableInput,
) -> AsyncTask<DeleteTableTask> {
    AsyncTask::new(DeleteTableTask {
        input: input.to_vec(),
        operation: DeleteTable {
            table: table_target(operation.table),
            base_revision: operation.base_revision,
        },
    })
}
#[napi(js_name = "executeDocxDeleteTableRow")]
pub fn execute_docx_delete_table_row_node(
    input: Buffer,
    operation: DeleteTableRowInput,
) -> AsyncTask<DeleteTableRowTask> {
    AsyncTask::new(DeleteTableRowTask {
        input: input.to_vec(),
        operation: DeleteTableRow {
            table: table_target(operation.table),
            row: row_target(operation.row),
            base_revision: operation.base_revision,
        },
    })
}
#[napi(js_name = "executeDocxDeleteTableColumn")]
pub fn execute_docx_delete_table_column_node(
    input: Buffer,
    operation: DeleteTableColumnInput,
) -> AsyncTask<DeleteTableColumnTask> {
    AsyncTask::new(DeleteTableColumnTask {
        input: input.to_vec(),
        operation: DeleteTableColumn {
            table: table_target(operation.table),
            column_header: operation.column_header.unwrap_or_default(),
            column_handle: operation.column_handle,
            base_revision: operation.base_revision,
        },
    })
}

pub struct InsertTableRowTask {
    input: Vec<u8>,
    operation: opensuite_protocol::InsertTableRowAfter,
}

impl Task for InsertTableRowTask {
    type Output = opensuite_docx::DocxExecutionResult;
    type JsValue = ExecuteDocxReplaceTextOutput;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(execute_docx_insert_table_row(
            std::mem::take(&mut self.input),
            &self.operation,
        ))
    }

    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(ExecuteDocxReplaceTextOutput {
            result: operation_result_output(result.operation),
            output: result.output_artifact.map(Buffer::from),
        })
    }
}

pub struct InsertTableRowsTask {
    input: Vec<u8>,
    operation: opensuite_protocol::InsertTableRowsAfter,
}

pub struct InsertTableColumnTask {
    input: Vec<u8>,
    operation: InsertTableColumnAfter,
}

impl Task for InsertTableColumnTask {
    type Output = opensuite_docx::DocxExecutionResult;
    type JsValue = ExecuteDocxReplaceTextOutput;
    fn compute(&mut self) -> Result<Self::Output> {
        Ok(execute_docx_insert_table_column(
            std::mem::take(&mut self.input),
            &self.operation,
        ))
    }
    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(ExecuteDocxReplaceTextOutput {
            result: operation_result_output(result.operation),
            output: result.output_artifact.map(Buffer::from),
        })
    }
}

impl Task for InsertTableRowsTask {
    type Output = opensuite_docx::DocxExecutionResult;
    type JsValue = ExecuteDocxReplaceTextOutput;
    fn compute(&mut self) -> Result<Self::Output> {
        Ok(execute_docx_insert_table_rows(
            std::mem::take(&mut self.input),
            &self.operation,
        ))
    }
    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(ExecuteDocxReplaceTextOutput {
            result: operation_result_output(result.operation),
            output: result.output_artifact.map(Buffer::from),
        })
    }
}

pub struct SetTableCellsTextTask {
    input: Vec<u8>,
    operation: opensuite_protocol::SetTableCellsText,
}

macro_rules! table_task {
    ($name:ident, $operation:ty, $execute:ident) => {
        pub struct $name {
            input: Vec<u8>,
            operation: $operation,
        }
        impl Task for $name {
            type Output = opensuite_docx::DocxExecutionResult;
            type JsValue = ExecuteDocxReplaceTextOutput;
            fn compute(&mut self) -> Result<Self::Output> {
                Ok($execute(std::mem::take(&mut self.input), &self.operation))
            }
            fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
                Ok(ExecuteDocxReplaceTextOutput {
                    result: operation_result_output(result.operation),
                    output: result.output_artifact.map(Buffer::from),
                })
            }
        }
    };
}
table_task!(CreateTableTask, CreateTable, execute_docx_create_table);
table_task!(DeleteTableTask, DeleteTable, execute_docx_delete_table);
table_task!(
    DeleteTableRowTask,
    DeleteTableRow,
    execute_docx_delete_table_row
);
table_task!(
    DeleteTableColumnTask,
    DeleteTableColumn,
    execute_docx_delete_table_column
);
table_task!(
    SetTableFormattingTask,
    SetTableFormatting,
    execute_docx_set_table_formatting
);

impl Task for SetTableCellsTextTask {
    type Output = opensuite_docx::DocxExecutionResult;
    type JsValue = ExecuteDocxReplaceTextOutput;
    fn compute(&mut self) -> Result<Self::Output> {
        Ok(execute_docx_set_table_cells_text(
            std::mem::take(&mut self.input),
            &self.operation,
        ))
    }
    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(ExecuteDocxReplaceTextOutput {
            result: operation_result_output(result.operation),
            output: result.output_artifact.map(Buffer::from),
        })
    }
}

fn table_target(target: TableTargetInput) -> TableTarget {
    TableTarget {
        header_cells: target.header_cells.unwrap_or_default(),
        occurrence: target.occurrence.map(|value| value as usize),
        handle: target.handle,
    }
}

fn row_target(target: TableRowTargetInput) -> TableRowTarget {
    TableRowTarget {
        first_cell_text: target.first_cell_text.unwrap_or_default(),
        occurrence: target.occurrence.map(|value| value as usize),
        handle: target.handle,
    }
}

fn text_target(target: TextTargetInput) -> TextTarget {
    TextTarget {
        text: target.text,
        occurrence: target.occurrence.map(|value| value as usize),
    }
}

fn diagnostic_output(diagnostic: Diagnostic) -> DiagnosticOutput {
    DiagnosticOutput {
        code: diagnostic.code,
        severity: diagnostic.severity.as_str().to_owned(),
        message: diagnostic.message,
        reason_code: diagnostic.reason_code,
        operation: diagnostic.operation,
        target_handle: diagnostic.target.map(|target| target.handle),
    }
}

fn container_name(container: TextContainer) -> String {
    container.as_str().to_owned()
}

fn find_text_output(result: FindTextResult) -> FindTextOutput {
    FindTextOutput {
        ok: result.diagnostics.is_empty(),
        query: result.query,
        match_count: result.matches.len() as u32,
        matches: result
            .matches
            .into_iter()
            .map(|item| FindTextMatchOutput {
                occurrence: item.occurrence as u32,
                text: item.text,
                before: item.before,
                after: item.after,
                container: container_name(item.container),
            })
            .collect(),
        diagnostics: result
            .diagnostics
            .into_iter()
            .map(diagnostic_output)
            .collect(),
    }
}

fn inspect_request(
    input: InspectDocxInput,
) -> (String, std::result::Result<InspectDocx, InspectDocxResult>) {
    let InspectDocxInput {
        focus,
        target,
        before,
        after,
    } = input;
    let (text, occurrence) = target
        .map(|target| (Some(target.text), target.occurrence))
        .unwrap_or((None, None));
    let focus = focus.unwrap_or_else(|| InspectDocxFocusInput {
        kind: "context".to_owned(),
        offset: None,
        limit: None,
        text,
        occurrence,
        before,
        after,
    });
    let kind = focus.kind.clone();
    let result = match focus.kind.as_str() {
        "overview" => Ok(InspectDocx {
            focus: InspectDocxFocus::Overview,
        }),
        "body_blocks" => Ok(InspectDocx {
            focus: InspectDocxFocus::BodyBlocks {
                offset: focus.offset.unwrap_or(0) as usize,
                limit: focus.limit.unwrap_or(20) as usize,
            },
        }),
        "headings" => Ok(InspectDocx {
            focus: InspectDocxFocus::Headings {
                offset: focus.offset.unwrap_or(0) as usize,
                limit: focus.limit.unwrap_or(20) as usize,
            },
        }),
        "paragraphs" => Ok(InspectDocx {
            focus: InspectDocxFocus::Paragraphs {
                offset: focus.offset.unwrap_or(0) as usize,
                limit: focus.limit.unwrap_or(20) as usize,
            },
        }),
        "tables" => Ok(InspectDocx {
            focus: InspectDocxFocus::Tables {
                offset: focus.offset.unwrap_or(0) as usize,
                limit: focus.limit.unwrap_or(20) as usize,
            },
        }),
        "context" => match focus.text {
            Some(text) => Ok(InspectDocx {
                focus: InspectDocxFocus::Context(InspectTextContext {
                    target: TextTarget {
                        text,
                        occurrence: focus.occurrence.map(|value| value as usize),
                    },
                    before: focus.before.unwrap_or(1) as usize,
                    after: focus.after.unwrap_or(1) as usize,
                }),
            }),
            None => Err(InspectDocxResult::failed(
                "INVALID_INSPECTION_REQUEST",
                "context inspection requires text",
            )),
        },
        _ => Err(InspectDocxResult::failed(
            "UNSUPPORTED_OPERATION",
            "inspection focus is not supported",
        )),
    };
    (kind, result)
}

fn inspect_docx_output(focus: String, result: InspectDocxResult) -> InspectDocxOutput {
    let mut output = InspectDocxOutput {
        ok: result.diagnostics.is_empty(),
        focus,
        overview: None,
        body_blocks: None,
        headings: None,
        paragraphs: None,
        tables: None,
        context: None,
        diagnostics: result
            .diagnostics
            .into_iter()
            .map(diagnostic_output)
            .collect(),
    };
    match result.content {
        Some(InspectDocxContent::Overview(value)) => output.overview = Some(overview_output(value)),
        Some(InspectDocxContent::BodyBlocks(value)) => {
            output.body_blocks = Some(body_block_page_output(value))
        }
        Some(InspectDocxContent::Headings(value)) => {
            output.headings = Some(heading_page_output(value))
        }
        Some(InspectDocxContent::Paragraphs(value)) => {
            output.paragraphs = Some(paragraph_page_output(value));
        }
        Some(InspectDocxContent::Tables(value)) => output.tables = Some(table_page_output(value)),
        Some(InspectDocxContent::Context(value)) => output.context = Some(context_output(value)),
        None => {}
    }
    output
}

fn overview_output(value: DocxOverview) -> OverviewOutput {
    OverviewOutput {
        body_block_count: value.body_block_count as u32,
        paragraph_count: value.paragraph_count as u32,
        table_count: value.table_count as u32,
        section_count: value.section_count as u32,
    }
}

fn page_output<T>(value: &InspectionPage<T>) -> InspectionPageOutput {
    InspectionPageOutput {
        total: value.total as u32,
        offset: value.offset as u32,
        returned: value.returned as u32,
        has_more: value.has_more,
    }
}

fn heading_page_output(value: InspectionPage<DocxHeading>) -> HeadingPageOutput {
    HeadingPageOutput {
        page: page_output(&value),
        items: value
            .items
            .into_iter()
            .map(|item| HeadingOutput {
                occurrence: item.occurrence as u32,
                text: item.text,
                style_name: item.style_name,
                level: item.level.map(u32::from),
            })
            .collect(),
    }
}

fn paragraph_page_output(value: InspectionPage<DocxParagraph>) -> ParagraphPageOutput {
    ParagraphPageOutput {
        page: page_output(&value),
        items: value
            .items
            .into_iter()
            .map(|item| ParagraphOutput {
                occurrence: item.occurrence as u32,
                handle: item.handle,
                text: item.text,
                style_name: item.style_name,
            })
            .collect(),
    }
}

fn body_block_page_output(value: InspectionPage<DocxBodyBlock>) -> BodyBlockPageOutput {
    BodyBlockPageOutput {
        page: page_output(&value),
        items: value
            .items
            .into_iter()
            .map(|item| BodyBlockOutput {
                handle: item.handle,
                kind: item.kind.as_str().to_owned(),
                text: item.text,
                table_handle: item.table_handle,
            })
            .collect(),
    }
}

fn table_page_output(value: InspectionPage<DocxTable>) -> TablePageOutput {
    TablePageOutput {
        page: page_output(&value),
        items: value.items.into_iter().map(table_output).collect(),
    }
}

fn table_output(value: DocxTable) -> TableOutput {
    TableOutput {
        occurrence: value.occurrence as u32,
        handle: value.handle,
        row_count: value.row_count as u32,
        is_rectangular: value.is_rectangular,
        affordances: value
            .affordances
            .into_iter()
            .map(affordance_output)
            .collect(),
        columns: value
            .columns
            .into_iter()
            .map(|column| TableColumnOutput {
                occurrence: column.occurrence as u32,
                handle: column.handle,
                text: column.text,
            })
            .collect(),
        rows: value
            .rows
            .into_iter()
            .map(
                |DocxTableRow {
                     handle,
                     cells,
                     cell_handles,
                     cell_affordances,
                 }| TableRowOutput {
                    handle,
                    cells,
                    cell_handles,
                    cell_affordances: cell_affordances
                        .into_iter()
                        .map(|values| values.into_iter().map(affordance_output).collect())
                        .collect(),
                },
            )
            .collect(),
    }
}

fn affordance_output(value: Affordance) -> AffordanceOutput {
    AffordanceOutput {
        capability: value.capability.as_str().to_owned(),
        supported: value.supported,
        reason: value.reason.map(|reason| reason.as_str().to_owned()),
    }
}

fn context_output(result: InspectTextContextResult) -> InspectContextOutput {
    let context_container =
        |item: opensuite_protocol::TextContextContainer| TextContextContainerOutput {
            relative_position: item.relative_position as i32,
            text: item.text,
            container: container_name(item.container),
        };
    InspectContextOutput {
        target: TextTargetInput {
            text: result.target.text,
            occurrence: result.target.occurrence.map(|value| value as u32),
        },
        container: result.container.map(context_container),
        nearby: result.nearby.into_iter().map(context_container).collect(),
    }
}

pub struct ReplaceTextTask {
    input: Vec<u8>,
    operation: ReplaceText,
}

pub struct SimpleTask<T> {
    input: Vec<u8>,
    operation: T,
    run: fn(Vec<u8>, &T) -> DocxExecutionResult,
}
impl<T: Send> Task for SimpleTask<T> {
    type Output = DocxExecutionResult;
    type JsValue = ExecuteDocxReplaceTextOutput;
    fn compute(&mut self) -> Result<Self::Output> {
        Ok((self.run)(std::mem::take(&mut self.input), &self.operation))
    }
    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(ExecuteDocxReplaceTextOutput {
            result: operation_result_output(result.operation),
            output: result.output_artifact.map(Buffer::from),
        })
    }
}

pub struct InsertParagraphTask {
    input: Vec<u8>,
    operation: std::result::Result<InsertParagraph, &'static str>,
}

pub struct InsertParagraphsTask {
    input: Vec<u8>,
    operation: std::result::Result<InsertParagraphs, &'static str>,
}
impl Task for InsertParagraphsTask {
    type Output = DocxExecutionResult;
    type JsValue = ExecuteDocxReplaceTextOutput;
    fn compute(&mut self) -> Result<Self::Output> {
        match &self.operation {
            Ok(operation) => Ok(execute_docx_insert_paragraphs(
                std::mem::take(&mut self.input),
                operation,
            )),
            Err(message) => Ok(DocxExecutionResult {
                operation: OperationResult::failed("INVALID_OPERATION", *message),
                output_artifact: None,
            }),
        }
    }
    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(ExecuteDocxReplaceTextOutput {
            result: operation_result_output(result.operation),
            output: result.output_artifact.map(Buffer::from),
        })
    }
}

impl Task for InsertParagraphTask {
    type Output = DocxExecutionResult;
    type JsValue = ExecuteDocxReplaceTextOutput;

    fn compute(&mut self) -> Result<Self::Output> {
        match &self.operation {
            Ok(operation) => Ok(execute_docx_insert_paragraph(
                std::mem::take(&mut self.input),
                operation,
            )),
            Err(message) => Ok(DocxExecutionResult {
                operation: OperationResult::failed("INVALID_OPERATION", *message),
                output_artifact: None,
            }),
        }
    }

    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(ExecuteDocxReplaceTextOutput {
            result: operation_result_output(result.operation),
            output: result.output_artifact.map(Buffer::from),
        })
    }
}

impl Task for ReplaceTextTask {
    type Output = opensuite_docx::DocxExecutionResult;
    type JsValue = ExecuteDocxReplaceTextOutput;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(execute_docx_replace_text(
            std::mem::take(&mut self.input),
            &self.operation,
        ))
    }

    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(ExecuteDocxReplaceTextOutput {
            result: operation_result_output(result.operation),
            output: result.output_artifact.map(Buffer::from),
        })
    }
}

fn operation_result_output(result: OperationResult) -> OperationResultOutput {
    OperationResultOutput {
        ok: result.status == opensuite_protocol::OperationStatus::Applied,
        status: result.status.as_str().to_owned(),
        diagnostics: result
            .diagnostics
            .into_iter()
            .map(diagnostic_output)
            .collect(),
        changes: result
            .changes
            .into_iter()
            .map(|change| OperationChangeOutput {
                kind: change.kind,
                before: change.before,
                after: change.after,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use opensuite_protocol::{Diagnostic, DiagnosticSeverity, OperationChange};

    use super::*;

    #[test]
    fn preserves_operation_result_shape() {
        let output = operation_result_output(OperationResult {
            status: opensuite_protocol::OperationStatus::Failed,
            diagnostics: vec![Diagnostic::new(
                "TARGET_NOT_FOUND",
                DiagnosticSeverity::Error,
                "missing",
            )],
            changes: vec![OperationChange {
                kind: "text_replaced".to_owned(),
                before: "before".to_owned(),
                after: "after".to_owned(),
            }],
        });

        assert!(!output.ok);
        assert_eq!(output.status, "failed");
        assert_eq!(output.diagnostics[0].severity, "error");
        assert_eq!(output.changes[0].kind, "text_replaced");
    }
}
