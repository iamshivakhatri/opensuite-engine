mod notes;
pub use notes::{delete_note, insert_note, inspect_notes, update_note};
mod fields;
pub use fields::{insert_fields, insert_toc, inspect_fields};
mod revisions;
pub use revisions::{
    accept_revision, delete_tracked_text, insert_tracked_text, inspect_tracked_changes,
    reject_revision, replace_text_with_tracked_change,
};
mod comments;
pub use comments::{add_comment, delete_comment, inspect_comments, update_comment};
mod layout;
pub use layout::inspect_layout;
mod styles;
use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
pub use styles::{create_style, update_style};
mod content_controls;
mod hyperlinks;
mod lists;
mod page_composition;
mod pictures;
mod sections;
mod table_cell_formatting;
mod text_formatting;
pub use content_controls::execute_docx_set_content_control_text_node;
pub use hyperlinks::execute_docx_set_hyperlink_node;
pub use lists::execute_docx_set_paragraphs_list_node;
use opensuite_docx::{
    DocxExecutionResult, create_blank_docx, execute_docx_create_table,
    execute_docx_delete_paragraph, execute_docx_delete_table, execute_docx_delete_table_column,
    execute_docx_delete_table_row, execute_docx_insert_paragraph, execute_docx_insert_paragraphs,
    execute_docx_insert_table_column, execute_docx_insert_table_row,
    execute_docx_insert_table_rows, execute_docx_replace_text,
    execute_docx_set_paragraph_formatting, execute_docx_set_paragraph_style,
    execute_docx_set_table_cell_shading, execute_docx_set_table_cells_text,
    execute_docx_set_table_column_widths, execute_docx_set_table_formatting, find_docx_text,
    inspect_docx, inspect_docx_style_snapshot,
};
use opensuite_protocol::{
    Affordance, CreateTable, DOCX_CAPABILITIES, DeleteTable, DeleteTableColumn, DeleteTableRow,
    DeleteTableRowTarget, Diagnostic, DocxBodyBlock, DocxHeading, DocxOverview, DocxParagraph,
    DocxParagraphList, DocxPicture, DocxTable, DocxTableRow, DocxTableRowWindow, FindText,
    FindTextResult, InsertParagraph, InsertParagraphs, InsertTableColumnAfter, InspectDocx,
    InspectDocxContent, InspectDocxFocus, InspectDocxResult, InspectTextContext,
    InspectTextContextResult, InspectionPage, OperationResult, ParagraphFormattingPatch,
    ParagraphPlacement, PropertyPatch, ReplaceText, RuntimeCapabilities, SetParagraphFormatting,
    SetParagraphStyle, SetTableCellShading, SetTableColumnWidths, SetTableFormatting,
    TableAlignment, TableBorders, TableCellColumn, TableCellMargins, TableCellRow, TableCellTarget,
    TableCellTextUpdate, TableFormattingPatch, TableRowTarget, TableTarget, TextContainer,
    TextTarget,
};
pub use page_composition::{
    execute_docx_delete_page_break_node, execute_docx_insert_page_break_node,
    execute_docx_set_header_footer_text_node, execute_docx_set_page_number_node,
    execute_docx_set_page_setup_node,
};
pub use pictures::{
    execute_docx_delete_picture_node, execute_docx_insert_picture_node,
    execute_docx_replace_picture_node, execute_docx_set_picture_size_node, set_picture_layout,
};
pub use sections::{
    insert_section_break, inspect_docx_sections, set_odd_even_headers, set_section_header_footer,
    set_section_properties,
};
pub use table_cell_formatting::execute_docx_set_table_cells_formatting_node;
pub use text_formatting::execute_docx_set_text_formatting_node;

#[napi(object)]
pub struct TextTargetInput {
    /// @schema {"description":"Exact text to match"}
    pub text: String,
    /// @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."}
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
    /// @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."}
    pub placement: ParagraphPlacementInput,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct InsertParagraphsInput {
    pub texts: Vec<String>,
    /// @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."}
    pub placement: ParagraphPlacementInput,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct ParagraphPlacementInput {
    /// @schema {"enum":["start","end","before","after"],"description":"start/end need no handle; before/after need an inspected body_blocks handle."}
    pub kind: String,
    /// @schema {"description":"Omit for start/end. Required for before/after; copy from the latest relevant document.inspect."}
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
    /// @schema {"description":"Style display name such as 'Heading 1'. Omit to clear."}
    pub style: Option<String>,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct SetParagraphFormattingInput {
    pub target: TextTargetInput,
    /// @schema {"enum":["left","center","right","both","distribute","clear"]}
    pub alignment: Option<String>,
    pub spacing_before_twips: Option<i32>,
    pub spacing_after_twips: Option<i32>,
    pub left_indent_twips: Option<i32>,
    pub clear_left_indent: Option<bool>,
    pub line_spacing: Option<styles::LineSpacingInput>,
    pub right_indent_twips: Option<i32>,
    pub first_line_indent_twips: Option<i32>,
    pub hanging_indent_twips: Option<i32>,
    /// @schema {"description":"Keep this paragraph with the following paragraph."}
    pub keep_with_next: Option<bool>,
    /// @schema {"description":"Keep all lines of this paragraph on one page."}
    pub keep_lines: Option<bool>,
    /// @schema {"description":"Remove direct properties and restore inheritance. Omitted fields stay unchanged."}
    pub clear: Option<Vec<String>>,
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
    pub target_type: Option<String>,
    pub target_description: Option<String>,
    pub update_index: Option<u32>,
    pub candidate_count: Option<u32>,
    pub candidate_targets: Option<Vec<String>>,
    pub required_selector_kind: Option<String>,
    pub retryable: Option<bool>,
    pub recovery_kind: Option<String>,
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
    /// @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."}
    pub occurrence: Option<u32>,
    pub handle: Option<String>,
}

#[napi(object)]
pub struct TableRowTargetInput {
    pub first_cell_text: Option<String>,
    /// @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."}
    pub occurrence: Option<u32>,
    pub handle: Option<String>,
}

#[napi(object)]
pub struct DeleteTableRowTargetInput {
    /// @schema {"enum":["header","label","index"]}
    pub kind: Option<String>,
    /// @schema {"description":"First-cell row label for kind=label."}
    pub text: Option<String>,
    /// @schema {"description":"Zero-based row position for kind=index."}
    pub index: Option<u32>,
    /// @schema {"description":"Required for kind=index; prevents a shifted row from being edited."}
    pub expected_first_cell_text: Option<String>,
    pub first_cell_text: Option<String>,
    /// @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."}
    pub occurrence: Option<u32>,
    pub handle: Option<String>,
}

#[napi(object)]
pub struct InsertTableRowInput {
    /// @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."}
    pub table: TableTargetInput,
    /// @schema {"description":"Row selector. Use firstCellText/occurrence when unambiguous; inspect only when a current row handle is needed."}
    pub after: TableRowTargetInput,
    pub cells: Vec<String>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct InsertTableRowsInput {
    /// @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."}
    pub table: TableTargetInput,
    /// @schema {"description":"Row selector. Use firstCellText/occurrence when unambiguous; inspect only when a current row handle is needed."}
    pub after: TableRowTargetInput,
    pub rows: Vec<Vec<String>>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct InsertTableColumnInput {
    /// @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."}
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
pub struct TableCellRowInput {
    /// @schema {"enum":["header","label","index"]}
    pub kind: String,
    /// @schema {"description":"First-cell row label for kind=label."}
    pub text: Option<String>,
    /// @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."}
    pub occurrence: Option<u32>,
    /// @schema {"description":"Zero-based row position for kind=index."}
    pub index: Option<u32>,
    /// @schema {"description":"Required for kind=index; prevents a shifted row from being edited."}
    pub expected_first_cell_text: Option<String>,
}

#[napi(object)]
pub struct TableCellColumnInput {
    /// @schema {"enum":["first","header","index"]}
    pub kind: String,
    /// @schema {"description":"First-row column header for kind=header."}
    pub text: Option<String>,
    /// @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."}
    pub occurrence: Option<u32>,
    /// @schema {"description":"Zero-based column position for kind=index."}
    pub index: Option<u32>,
    /// @schema {"description":"Required for kind=index; prevents a shifted column from being edited."}
    pub expected_header_text: Option<String>,
}

#[napi(object)]
pub struct SemanticTableCellTargetInput {
    pub row: Option<TableCellRowInput>,
    pub column: Option<TableCellColumnInput>,
    pub row_label: Option<String>,
    pub column_header: Option<String>,
    /// @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."}
    pub occurrence: Option<u32>,
    pub handle: Option<String>,
}

#[napi(object)]
pub struct TableCellTextUpdateInput {
    /// @schema {"description":"Prefer row + column. Use a handle only when these selectors cannot express the cell. Legacy rowLabel + columnHeader remains supported for data cells."}
    pub target: SemanticTableCellTargetInput,
    pub expected_current_text: String,
    pub replacement: String,
}

#[napi(object)]
pub struct SetTableCellsTextInput {
    /// @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."}
    pub table: TableTargetInput,
    pub updates: Vec<TableCellTextUpdateInput>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct CreateTableInput {
    pub rows: Vec<Vec<String>>,
    /// @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."}
    pub placement: ParagraphPlacementInput,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct DeleteTableInput {
    /// @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."}
    pub table: TableTargetInput,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct DeleteTableRowInput {
    /// @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."}
    pub table: TableTargetInput,
    /// @schema {"description":"Prefer kind=label with text and occurrence when needed, or kind=index with expectedFirstCellText. Use a handle only when these cannot express the row. Older firstCellText + occurrence remains supported."}
    pub row: DeleteTableRowTargetInput,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct DeleteTableColumnInput {
    /// @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."}
    pub table: TableTargetInput,
    pub column_header: Option<String>,
    pub column_handle: Option<String>,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct SetTableFormattingInput {
    /// @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."}
    pub table: TableTargetInput,
    /// @schema {"enum":["left","center","right","clear"]}
    pub alignment: Option<String>,
    pub cell_margin_top_twips: Option<u32>,
    pub cell_margin_right_twips: Option<u32>,
    pub cell_margin_bottom_twips: Option<u32>,
    pub cell_margin_left_twips: Option<u32>,
    /// @schema {"enum":["grid","none","clear"]}
    pub borders: Option<String>,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct SetTableColumnWidthsInput {
    /// @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."}
    pub table: TableTargetInput,
    pub widths_twips: Vec<u32>,
    pub base_revision: Option<String>,
}
#[napi(object)]
pub struct TableCellShadingUpdateInput {
    /// @schema {"description":"Prefer row + column. Use a handle only when these selectors cannot express the cell. Legacy rowLabel + columnHeader remains supported for data cells."}
    pub target: SemanticTableCellTargetInput,
    pub fill: Option<String>,
}
#[napi(object)]
pub struct SetTableCellShadingInput {
    /// @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."}
    pub table: TableTargetInput,
    pub updates: Vec<TableCellShadingUpdateInput>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct DocxOperationOutput {
    pub id: String,
    pub kind: String,
    pub node_export: String,
    pub description: String,
    pub input_schema: Option<String>,
    pub json_input: bool,
    pub json_output: bool,
}

#[napi(object)]
pub struct FormatCapabilitiesOutput {
    pub format: String,
    pub capabilities: Vec<String>,
    pub operations: Vec<DocxOperationOutput>,
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
    /// @schema {"description":"Exact text to find"}
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
    /// @schema {"enum":["overview","headings","paragraphs","tables","body_blocks","table_rows","context"]}
    pub kind: String,
    pub offset: Option<u32>,
    /// @schema {"minimum":1,"maximum":100}
    pub limit: Option<u32>,
    pub table_handle: Option<String>,
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
    pub index: u32,
    pub handle: Option<String>,
    pub target_occurrence: Option<u32>,
    pub text: String,
    pub style_name: Option<String>,
    pub list: Option<ParagraphListOutput>,
}

#[napi(object)]
pub struct ParagraphListOutput {
    pub kind: String,
    pub level: u32,
    pub supported: bool,
}

#[napi(object)]
pub struct BodyBlockOutput {
    pub handle: String,
    pub kind: String,
    pub text: Option<String>,
    pub style_name: Option<String>,
    pub heading_level: Option<u32>,
    pub table_handle: Option<String>,
    pub row_count: Option<u32>,
    pub column_count: Option<u32>,
    pub header_texts: Option<Vec<String>>,
    pub picture: Option<PictureOutput>,
}

#[napi(object)]
pub struct PictureOutput {
    pub handle: String,
    pub format: String,
    pub width_emu: i64,
    pub height_emu: i64,
    pub alt_text: Option<String>,
    pub affordances: Vec<AffordanceOutput>,
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
pub struct TableRowWindowItemOutput {
    pub index: u32,
    pub cells: Vec<String>,
}
#[napi(object)]
pub struct TableRowWindowOutput {
    pub table_handle: String,
    pub row_count: u32,
    pub column_count: u32,
    pub header_texts: Vec<String>,
    pub row_offset: u32,
    pub rows: Vec<TableRowWindowItemOutput>,
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
    pub table_rows: Option<TableRowWindowOutput>,
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
    static INPUTS: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    let inputs = INPUTS.get_or_init(|| {
        serde_json::from_str(include_str!("../operation-inputs.json"))
            .expect("generated operation inputs must be valid JSON")
    });
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
                capabilities: DOCX_CAPABILITIES
                    .iter()
                    .filter(|capability| capability.node_available)
                    .map(|capability| capability.id.to_owned())
                    .collect(),
                operations: DOCX_CAPABILITIES
                    .iter()
                    .filter(|capability| capability.node_available)
                    .filter_map(|capability| {
                        capability
                            .node_export
                            .map(|node_export| DocxOperationOutput {
                                id: capability.id.to_owned(),
                                kind: capability.kind.as_str().to_owned(),
                                node_export: node_export.to_owned(),
                                description: inputs[node_export]["description"]
                                    .as_str()
                                    .unwrap_or(capability.id)
                                    .to_owned(),
                                input_schema: inputs[node_export]
                                    .get("inputSchema")
                                    .map(|schema| schema.to_string()),
                                json_output: inputs[node_export]["jsonOutput"]
                                    .as_bool()
                                    .unwrap_or(false),
                                json_input: inputs[node_export]["jsonInput"]
                                    .as_bool()
                                    .unwrap_or(false),
                            })
                    })
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
#[napi(js_name = "findDocxText", ts_return_type = "Promise<FindTextOutput>")]
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
#[napi(js_name = "inspectDocx", ts_return_type = "Promise<InspectDocxOutput>")]
pub fn inspect_docx_node(input: Buffer, request: InspectDocxInput) -> AsyncTask<InspectDocxTask> {
    let (focus, request) = inspect_request(request);
    AsyncTask::new(InspectDocxTask {
        input: input.to_vec(),
        focus,
        request,
    })
}

/// Inspects the bounded DOCX formatting system away from Node's event loop.
/// @jsonOutput
#[napi(
    js_name = "inspectDocxStyleSnapshot",
    ts_return_type = "Promise<string>"
)]
pub fn inspect_docx_style_snapshot_node(input: Buffer) -> AsyncTask<InspectDocxStyleSnapshotTask> {
    AsyncTask::new(InspectDocxStyleSnapshotTask {
        input: input.to_vec(),
    })
}

pub struct InspectDocxStyleSnapshotTask {
    input: Vec<u8>,
}

impl Task for InspectDocxStyleSnapshotTask {
    type Output = opensuite_docx::DocxStyleSnapshot;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(inspect_docx_style_snapshot(std::mem::take(&mut self.input)))
    }

    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        serde_json::to_string(&result).map_err(|error| napi::Error::from_reason(error.to_string()))
    }
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
/// Replace one exact text span. Requires target text, zero-based occurrence when ambiguous, expectedCurrentText (must match current content), and replacement.
#[napi(
    js_name = "executeDocxReplaceText",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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
/// Insert one paragraph. For a blank document, use placement {kind: "end"} or {kind: "start"} without a handle.
#[napi(
    js_name = "executeDocxInsertParagraph",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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

/// Insert multiple paragraphs. For a blank document, use placement {kind: "end"} or {kind: "start"} without a handle.
#[napi(
    js_name = "executeDocxInsertParagraphs",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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

/// Delete a paragraph identified by exact text target (occurrence zero-based when ambiguous).
#[napi(
    js_name = "executeDocxDeleteParagraph",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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
/// Set or clear a direct body paragraph style by display name (e.g. 'Heading 1'), not styleId. Does not format table cells. Target via exact text + zero-based occurrence. Omit style to clear.
#[napi(
    js_name = "executeDocxSetParagraphStyle",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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
/// Set alignment, spacing, indentation, or keep rules on a direct body paragraph (occurrence zero-based). Twips: 20 = 1 pt. both = justified.
#[napi(
    js_name = "executeDocxSetParagraphFormatting",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_paragraph_formatting_node(
    input: Buffer,
    operation: SetParagraphFormattingInput,
) -> napi::Result<AsyncTask<SimpleTask<SetParagraphFormatting>>> {
    let mut clear = operation.clear.unwrap_or_default();
    styles::validate_paragraph_clear(&clear)?;
    if operation.clear_left_indent == Some(true) {
        clear.push("leftIndentTwips".into());
    }
    if operation.alignment.as_deref() == Some("clear") {
        clear.push("alignment".into());
    }
    let alignment = styles::alignment(operation.alignment.filter(|value| value != "clear"))?;
    let line_spacing = styles::line_spacing(operation.line_spacing)?;
    Ok(AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetParagraphFormatting {
            target: text_target(operation.target),
            formatting: ParagraphFormattingPatch {
                alignment: styles::property(alignment, &clear, "alignment"),
                spacing_before_twips: styles::property(
                    operation.spacing_before_twips,
                    &clear,
                    "spacingBeforeTwips",
                ),
                spacing_after_twips: styles::property(
                    operation.spacing_after_twips,
                    &clear,
                    "spacingAfterTwips",
                ),
                line_spacing: styles::property(line_spacing, &clear, "lineSpacing"),
                left_indent_twips: styles::property(
                    operation.left_indent_twips,
                    &clear,
                    "leftIndentTwips",
                ),
                right_indent_twips: styles::property(
                    operation.right_indent_twips,
                    &clear,
                    "rightIndentTwips",
                ),
                first_line_indent_twips: styles::property(
                    operation.first_line_indent_twips,
                    &clear,
                    "firstLineIndentTwips",
                ),
                hanging_indent_twips: styles::property(
                    operation.hanging_indent_twips,
                    &clear,
                    "hangingIndentTwips",
                ),
                keep_with_next: styles::property(operation.keep_with_next, &clear, "keepWithNext"),
                keep_lines: styles::property(operation.keep_lines, &clear, "keepLines"),
            },
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_paragraph_formatting,
    }))
}
/// Runs DOCX table-row insertion away from Node's event loop and returns a Promise.
/// Insert one table row after an anchor; use insert_table_rows when several contiguous rows are known.
#[napi(
    js_name = "executeDocxInsertTableRow",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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
/// Insert several contiguous, known rows in one call after a row anchor; prefer this over repeated insert_table_row calls.
#[napi(
    js_name = "executeDocxInsertTableRows",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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

/// Insert a table column (header + one cell per existing row).
#[napi(
    js_name = "executeDocxInsertTableColumn",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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
/// Update known cells in one table. Select each by row (header, label, or checked index) and column (first, header, or checked index); use occurrence for duplicate labels. All selectors use the original table state, so headers and row labels can change in this call. Each update needs expectedCurrentText and replacement.
#[napi(
    js_name = "executeDocxSetTableCellsText",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_table_cells_text_node(
    input: Buffer,
    operation: SetTableCellsTextInput,
) -> Result<AsyncTask<SetTableCellsTextTask>> {
    let updates = operation
        .updates
        .into_iter()
        .map(|update| {
            Ok(TableCellTextUpdate {
                target: semantic_cell_target(update.target)?,
                expected_current_text: update.expected_current_text,
                replacement: update.replacement,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(AsyncTask::new(SetTableCellsTextTask {
        input: input.to_vec(),
        operation: opensuite_protocol::SetTableCellsText {
            table: table_target(operation.table),
            updates,
            base_revision: operation.base_revision,
        },
    }))
}

/// Set table alignment/margins/borders.
#[napi(
    js_name = "executeDocxSetTableFormatting",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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

/// Set table column widths in twips.
#[napi(
    js_name = "executeDocxSetTableColumnWidths",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_table_column_widths_node(
    input: Buffer,
    operation: SetTableColumnWidthsInput,
) -> AsyncTask<SetTableColumnWidthsTask> {
    AsyncTask::new(SetTableColumnWidthsTask {
        input: input.to_vec(),
        operation: SetTableColumnWidths {
            table: table_target(operation.table),
            widths_twips: operation
                .widths_twips
                .into_iter()
                .map(|width| width as u16)
                .collect(),
            base_revision: operation.base_revision,
        },
    })
}
/// Shade known table cells in one call using table + row + column, including header and first-column cells. Fill is 6-digit RGB without #; omit fill to clear. Use set_table_cells_formatting for fill and text together.
#[napi(
    js_name = "executeDocxSetTableCellShading",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_table_cell_shading_node(
    input: Buffer,
    operation: SetTableCellShadingInput,
) -> Result<AsyncTask<SetTableCellShadingTask>> {
    let updates = operation
        .updates
        .into_iter()
        .map(|update| {
            Ok(opensuite_protocol::TableCellShadingUpdate {
                target: semantic_cell_target(update.target)?,
                fill: update.fill,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(AsyncTask::new(SetTableCellShadingTask {
        input: input.to_vec(),
        operation: SetTableCellShading {
            table: table_target(operation.table),
            updates,
            base_revision: operation.base_revision,
        },
    }))
}

/// Create a table at a paragraph placement.
#[napi(
    js_name = "executeDocxCreateTable",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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
/// Delete a table.
#[napi(
    js_name = "executeDocxDeleteTable",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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
/// Delete a known table row using table + row. Prefer row {kind: 'label', text, occurrence?} or a checked index; a handle is only needed when these cannot express the row.
#[napi(
    js_name = "executeDocxDeleteTableRow",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_delete_table_row_node(
    input: Buffer,
    operation: DeleteTableRowInput,
) -> Result<AsyncTask<DeleteTableRowTask>> {
    let row = delete_row_target(operation.row)?;
    Ok(AsyncTask::new(DeleteTableRowTask {
        input: input.to_vec(),
        operation: DeleteTableRow {
            table: table_target(operation.table),
            row,
            base_revision: operation.base_revision,
        },
    }))
}
/// Delete a table column by header or column handle.
#[napi(
    js_name = "executeDocxDeleteTableColumn",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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
table_task!(
    SetTableColumnWidthsTask,
    SetTableColumnWidths,
    execute_docx_set_table_column_widths
);
table_task!(
    SetTableCellShadingTask,
    SetTableCellShading,
    execute_docx_set_table_cell_shading
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

pub(crate) fn semantic_cell_target(
    target: SemanticTableCellTargetInput,
) -> Result<TableCellTarget> {
    let row = target.row.map(semantic_row_target).transpose()?;
    let column = target
        .column
        .map(|column| match column.kind.as_str() {
            "first" => Ok(TableCellColumn::First),
            "header" => Ok(TableCellColumn::Header {
                text: column
                    .text
                    .ok_or_else(|| napi::Error::from_reason("column.text is required"))?,
                occurrence: column.occurrence.map(|value| value as usize),
            }),
            "index" => Ok(TableCellColumn::Index {
                index: column
                    .index
                    .ok_or_else(|| napi::Error::from_reason("column.index is required"))?
                    as usize,
                expected_header_text: column.expected_header_text.ok_or_else(|| {
                    napi::Error::from_reason("column.expectedHeaderText is required")
                })?,
            }),
            _ => Err(napi::Error::from_reason("unknown column selector kind")),
        })
        .transpose()?;
    if row.is_some() != column.is_some() {
        return Err(napi::Error::from_reason(
            "cell target needs both row and column",
        ));
    }
    if row.is_some()
        && (target.handle.is_some()
            || target.row_label.is_some()
            || target.column_header.is_some()
            || target.occurrence.is_some())
    {
        return Err(napi::Error::from_reason(
            "use either row and column or a legacy cell target",
        ));
    }
    Ok(TableCellTarget {
        row,
        column,
        row_label: target.row_label.unwrap_or_default(),
        column_header: target.column_header.unwrap_or_default(),
        occurrence: target.occurrence.map(|value| value as usize),
        handle: target.handle,
    })
}

fn semantic_row_target(row: TableCellRowInput) -> Result<TableCellRow> {
    match row.kind.as_str() {
        "header" => Ok(TableCellRow::Header),
        "label" => Ok(TableCellRow::Label {
            text: row
                .text
                .ok_or_else(|| napi::Error::from_reason("row.text is required"))?,
            occurrence: row.occurrence.map(|value| value as usize),
        }),
        "index" => Ok(TableCellRow::Index {
            index: row
                .index
                .ok_or_else(|| napi::Error::from_reason("row.index is required"))?
                as usize,
            expected_first_cell_text: row
                .expected_first_cell_text
                .ok_or_else(|| napi::Error::from_reason("row.expectedFirstCellText is required"))?,
        }),
        _ => Err(napi::Error::from_reason("unknown row selector kind")),
    }
}

fn delete_row_target(row: DeleteTableRowTargetInput) -> Result<DeleteTableRowTarget> {
    if let Some(kind) = row.kind {
        if row.handle.is_some() || row.first_cell_text.is_some() {
            return Err(napi::Error::from_reason(
                "use either a semantic row selector or a legacy row target",
            ));
        }
        return semantic_row_target(TableCellRowInput {
            kind,
            text: row.text,
            occurrence: row.occurrence,
            index: row.index,
            expected_first_cell_text: row.expected_first_cell_text,
        })
        .map(DeleteTableRowTarget::Semantic);
    }
    if row.text.is_some() || row.index.is_some() || row.expected_first_cell_text.is_some() {
        return Err(napi::Error::from_reason("row.kind is required"));
    }
    if row.handle.is_none() && row.first_cell_text.is_none() {
        return Err(napi::Error::from_reason(
            "row needs kind, firstCellText, or handle",
        ));
    }
    Ok(DeleteTableRowTarget::Legacy(TableRowTarget {
        first_cell_text: row.first_cell_text.unwrap_or_default(),
        occurrence: row.occurrence.map(|value| value as usize),
        handle: row.handle,
    }))
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
        target_type: diagnostic.target_type,
        target_description: diagnostic.target_description,
        update_index: diagnostic.update_index,
        candidate_count: diagnostic.candidate_count,
        candidate_targets: (!diagnostic.candidate_targets.is_empty())
            .then_some(diagnostic.candidate_targets),
        required_selector_kind: diagnostic.required_selector_kind,
        retryable: diagnostic.retryable,
        recovery_kind: diagnostic.recovery_kind,
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
        table_handle: None,
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
        "table_rows" => match (focus.table_handle, focus.offset, focus.limit) {
            (Some(table_handle), row_offset, row_limit) => Ok(InspectDocx {
                focus: InspectDocxFocus::TableRows {
                    table_handle,
                    row_offset: row_offset.unwrap_or(0) as usize,
                    row_limit: row_limit.unwrap_or(3) as usize,
                },
            }),
            _ => Err(InspectDocxResult::failed(
                "INVALID_INSPECTION_REQUEST",
                "table_rows inspection requires tableHandle",
            )),
        },
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
        table_rows: None,
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
        Some(InspectDocxContent::TableRows(value)) => {
            output.table_rows = Some(table_row_window_output(value))
        }
        Some(InspectDocxContent::Context(value)) => output.context = Some(context_output(value)),
        None => {}
    }
    output
}

fn table_row_window_output(value: DocxTableRowWindow) -> TableRowWindowOutput {
    TableRowWindowOutput {
        table_handle: value.table_handle,
        row_count: value.row_count as u32,
        column_count: value.column_count as u32,
        header_texts: value.header_texts,
        row_offset: value.row_offset as u32,
        rows: value
            .rows
            .into_iter()
            .map(|row| TableRowWindowItemOutput {
                index: row.index as u32,
                cells: row.cells,
            })
            .collect(),
    }
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

fn paragraph_list_output(value: DocxParagraphList) -> ParagraphListOutput {
    ParagraphListOutput {
        kind: value.kind,
        level: value.level as u32,
        supported: value.supported,
    }
}

fn paragraph_page_output(value: InspectionPage<DocxParagraph>) -> ParagraphPageOutput {
    ParagraphPageOutput {
        page: page_output(&value),
        items: value
            .items
            .into_iter()
            .map(|item| ParagraphOutput {
                index: item.index as u32,
                handle: item.handle,
                target_occurrence: item.target_occurrence.map(|value| value as u32),
                text: item.text,
                style_name: item.style_name,
                list: item.list.map(paragraph_list_output),
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
                style_name: item.style_name,
                heading_level: item.heading_level.map(u32::from),
                table_handle: item.table_handle,
                row_count: item.row_count.map(|value| value as u32),
                column_count: item.column_count.map(|value| value as u32),
                header_texts: item.header_texts,
                picture: item.picture.map(picture_output),
            })
            .collect(),
    }
}

fn picture_output(value: DocxPicture) -> PictureOutput {
    PictureOutput {
        handle: value.handle,
        format: value.format.as_str().to_owned(),
        width_emu: value.width_emu,
        height_emu: value.height_emu,
        alt_text: value.alt_text,
        affordances: value
            .affordances
            .into_iter()
            .map(affordance_output)
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
    pub(crate) input: Vec<u8>,
    pub(crate) operation: T,
    pub(crate) run: fn(Vec<u8>, &T) -> DocxExecutionResult,
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

pub(crate) fn operation_result_output(result: OperationResult) -> OperationResultOutput {
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
    fn advertises_the_style_snapshot_binding() {
        let capabilities = get_docx_capabilities();
        let docx = capabilities
            .formats
            .iter()
            .find(|format| format.format == "docx")
            .unwrap();

        assert!(
            docx.capabilities
                .iter()
                .any(|value| value == "style_snapshot")
        );
    }

    #[test]
    fn advertises_semantic_table_selectors() {
        let capabilities = get_docx_capabilities();
        let docx = capabilities
            .formats
            .iter()
            .find(|format| format.format == "docx")
            .unwrap();

        for id in ["semantic_table_cell_targets", "semantic_table_row_deletion"] {
            assert!(docx.capabilities.iter().any(|value| value == id), "{id}");
        }
    }

    #[test]
    fn preserves_operation_result_shape() {
        let mut diagnostic = Diagnostic::new(
            "TARGET_AMBIGUOUS",
            DiagnosticSeverity::Error,
            "two rows matched",
        )
        .with_reason_code("TABLE_ROW_AMBIGUOUS");
        diagnostic.candidate_count = Some(2);
        diagnostic.candidate_targets = vec!["row index 1".to_owned(), "row index 2".to_owned()];
        diagnostic.required_selector_kind = Some("rowOccurrence".to_owned());
        diagnostic.recovery_kind = Some("unsafe_source".to_owned());
        let output = operation_result_output(OperationResult {
            status: opensuite_protocol::OperationStatus::Failed,
            diagnostics: vec![diagnostic],
            changes: vec![OperationChange {
                kind: "text_replaced".to_owned(),
                before: "before".to_owned(),
                after: "after".to_owned(),
            }],
        });

        assert!(!output.ok);
        assert_eq!(output.status, "failed");
        assert_eq!(output.diagnostics[0].severity, "error");
        assert_eq!(
            output.diagnostics[0].reason_code.as_deref(),
            Some("TABLE_ROW_AMBIGUOUS")
        );
        assert_eq!(output.diagnostics[0].candidate_count, Some(2));
        assert_eq!(
            output.diagnostics[0].candidate_targets.as_ref().unwrap()[1],
            "row index 2"
        );
        assert_eq!(
            output.diagnostics[0].required_selector_kind.as_deref(),
            Some("rowOccurrence")
        );
        assert_eq!(
            output.diagnostics[0].recovery_kind.as_deref(),
            Some("unsafe_source")
        );
        assert_eq!(output.changes[0].kind, "text_replaced");
    }
}
