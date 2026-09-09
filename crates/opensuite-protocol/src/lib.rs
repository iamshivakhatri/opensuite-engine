//! Stable runtime boundary between the engine and its consumers.

/// Identifies this architectural layer.
pub const LAYER: &str = "protocol";
pub const PROTOCOL_VERSION: u32 = 1;

/// A runtime's declared, externally usable capabilities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCapabilities {
    pub protocol_version: u32,
    pub engine_version: String,
    pub formats: Vec<FormatCapabilities>,
}

impl RuntimeCapabilities {
    /// Declares the DOCX capabilities implemented by this engine version.
    pub fn docx(engine_version: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            engine_version: engine_version.into(),
            formats: vec![FormatCapabilities {
                format: DocumentFormat::Docx,
                capabilities: DOCX_CAPABILITIES
                    .iter()
                    .map(|id| CapabilityId::new(*id))
                    .collect(),
            }],
        }
    }

    /// Compatibility name retained while DOCX capabilities grow beyond inspection.
    pub fn docx_read_only(engine_version: impl Into<String>) -> Self {
        Self::docx(engine_version)
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "protocol_version": self.protocol_version,
            "engine_version": self.engine_version,
            "formats": self.formats.iter().map(FormatCapabilities::to_json).collect::<Vec<_>>(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatCapabilities {
    pub format: DocumentFormat,
    pub capabilities: Vec<CapabilityId>,
}

impl FormatCapabilities {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "format": self.format.as_str(),
            "capabilities": self.capabilities.iter().map(CapabilityId::as_str).collect::<Vec<_>>(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentFormat {
    Docx,
}

impl DocumentFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Docx => "docx",
        }
    }
}

/// A stable, machine-readable capability identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityId(String);

impl CapabilityId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

const DOCX_CAPABILITIES: &[&str] = &[
    "inspect",
    "text",
    "tables",
    "styles",
    "paragraph_formatting",
    "numbering",
    "sections",
    "headers_footers",
    "references",
    "pictures",
    "fields",
    "content_controls",
    "tracked_changes",
    "comments",
    "revision_views",
    "replace_text",
    "create_blank_docx",
    "body_blocks",
    "insert_paragraph",
    "insert_paragraphs",
    "insert_paragraph_after",
    "delete_paragraph",
    "set_table_cell_text",
    "set_content_control_text",
    "set_paragraph_formatting",
    "set_text_formatting",
    "set_paragraph_style",
    "replace_picture",
    "insert_picture",
    "insert_table_row",
    "insert_table_rows",
    "set_table_cells_text",
    "insert_table_column",
    "create_table",
    "delete_table",
    "delete_table_row",
    "delete_table_column",
    "set_table_formatting",
    "find_text",
    "inspect_context",
];

/// A DOCX text target. `occurrence` is zero-based when repeated exact text needs disambiguation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextTarget {
    pub text: String,
    pub occurrence: Option<usize>,
}

/// The first typed DOCX mutation. `base_revision` is opaque caller metadata only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplaceText {
    pub target: TextTarget,
    pub expected_current_text: String,
    pub replacement: String,
    pub base_revision: Option<String>,
}

/// Inserts a plain paragraph after the paragraph containing `anchor`.
/// `base_revision` is opaque caller metadata only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsertParagraphAfter {
    pub anchor: TextTarget,
    pub text: String,
    pub base_revision: Option<String>,
}

/// Places a new paragraph in the direct, Current-view document body sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsertParagraph {
    pub text: String,
    pub placement: ParagraphPlacement,
    pub base_revision: Option<String>,
}

/// Inserts several plain paragraphs as one atomic body mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsertParagraphs {
    pub texts: Vec<String>,
    pub placement: ParagraphPlacement,
    pub base_revision: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParagraphPlacement {
    Start,
    End,
    Before { handle: String },
    After { handle: String },
}

/// Deletes the ordinary body paragraph containing `target`.
/// `base_revision` is opaque caller metadata only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteParagraph {
    pub target: TextTarget,
    pub base_revision: Option<String>,
}

/// A semantic table-cell target using the first-row header and first-cell row label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableCellTarget {
    pub row_label: String,
    pub column_header: String,
    pub occurrence: Option<usize>,
    /// Opaque Current-view structural handle returned by table inspection.
    pub handle: Option<String>,
}

/// A simple table selected by its complete first-row cell text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableTarget {
    pub header_cells: Vec<String>,
    pub occurrence: Option<usize>,
    /// Opaque Current-view structural handle returned by table inspection.
    pub handle: Option<String>,
}

/// A row selected by its exact Current-view first-cell text within a selected table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableRowTarget {
    pub first_cell_text: String,
    pub occurrence: Option<usize>,
    /// Opaque Current-view structural handle returned by table inspection.
    pub handle: Option<String>,
}

/// Inserts one complete row after an existing row in a simple semantic table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsertTableRowAfter {
    pub table: TableTarget,
    pub after: TableRowTarget,
    pub cells: Vec<String>,
    pub base_revision: Option<String>,
}

/// Inserts several complete rows after an existing row in one simple semantic table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsertTableRowsAfter {
    pub table: TableTarget,
    pub after: TableRowTarget,
    pub rows: Vec<Vec<String>>,
    pub base_revision: Option<String>,
}

/// Replaces visible text in one simple table cell. `base_revision` is opaque caller metadata only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetTableCellText {
    pub target: TableCellTarget,
    pub expected_current_text: String,
    pub replacement: String,
    pub base_revision: Option<String>,
}

/// One semantic table-cell text replacement within a multi-cell operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableCellTextUpdate {
    pub target: TableCellTarget,
    pub expected_current_text: String,
    pub replacement: String,
}

/// Replaces visible text in several cells of one simple semantic table atomically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetTableCellsText {
    pub table: TableTarget,
    pub updates: Vec<TableCellTextUpdate>,
    pub base_revision: Option<String>,
}

/// Inserts one complete column after an exact header in a simple semantic table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsertTableColumnAfter {
    pub table: TableTarget,
    pub after_column_header: String,
    /// Opaque Current-view column handle returned by table inspection.
    pub after_column_handle: Option<String>,
    pub header: String,
    pub cells: Vec<String>,
    pub base_revision: Option<String>,
}

/// Creates one rectangular table at a direct body placement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateTable {
    pub rows: Vec<Vec<String>>,
    pub placement: ParagraphPlacement,
    pub base_revision: Option<String>,
}

/// Deletes one safely resolved direct body table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteTable {
    pub table: TableTarget,
    pub base_revision: Option<String>,
}

/// Deletes one row from a table with at least two rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteTableRow {
    pub table: TableTarget,
    pub row: TableRowTarget,
    pub base_revision: Option<String>,
}

/// Deletes one column from a table with at least two columns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteTableColumn {
    pub table: TableTarget,
    pub column_header: String,
    pub column_handle: Option<String>,
    pub base_revision: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableAlignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableBorders {
    Grid,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableCellMargins {
    pub top_twips: u16,
    pub right_twips: u16,
    pub bottom_twips: u16,
    pub left_twips: u16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TableFormattingPatch {
    pub alignment: Option<PropertyPatch<TableAlignment>>,
    pub cell_margins: Option<PropertyPatch<TableCellMargins>>,
    pub borders: Option<PropertyPatch<TableBorders>>,
}

/// Changes only basic direct table properties.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetTableFormatting {
    pub table: TableTarget,
    pub formatting: TableFormattingPatch,
    pub base_revision: Option<String>,
}

/// A semantic content-control target. At least one of `tag` or `alias` is required.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentControlTarget {
    pub tag: Option<String>,
    pub alias: Option<String>,
    pub occurrence: Option<usize>,
}

/// Replaces visible text in one simple content control.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetContentControlText {
    pub target: ContentControlTarget,
    pub expected_current_text: String,
    pub replacement: String,
    pub base_revision: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropertyPatch<T> {
    Set(T),
    Clear,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParagraphAlignment {
    Left,
    Center,
    Right,
    Both,
    Distribute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineSpacingRule {
    Auto,
    Exact,
    AtLeast,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineSpacing {
    pub value: u32,
    pub rule: Option<LineSpacingRule>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParagraphFormattingPatch {
    pub alignment: Option<PropertyPatch<ParagraphAlignment>>,
    pub spacing_before_twips: Option<PropertyPatch<i32>>,
    pub spacing_after_twips: Option<PropertyPatch<i32>>,
    pub line_spacing: Option<PropertyPatch<LineSpacing>>,
    pub left_indent_twips: Option<PropertyPatch<i32>>,
    pub right_indent_twips: Option<PropertyPatch<i32>>,
    pub first_line_indent_twips: Option<PropertyPatch<i32>>,
    pub hanging_indent_twips: Option<PropertyPatch<i32>>,
    pub keep_with_next: Option<PropertyPatch<bool>>,
    pub keep_lines: Option<PropertyPatch<bool>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetParagraphFormatting {
    pub target: TextTarget,
    pub formatting: ParagraphFormattingPatch,
    pub base_revision: Option<String>,
}

/// Direct character formatting expressed in native Word half-point units.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextFormattingPatch {
    pub bold: Option<PropertyPatch<bool>>,
    pub italic: Option<PropertyPatch<bool>>,
    pub font_size_half_points: Option<PropertyPatch<u16>>,
    pub font_family: Option<PropertyPatch<String>>,
}

/// Changes selected direct run properties for one whole visible run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetTextFormatting {
    pub target: TextTarget,
    pub formatting: TextFormattingPatch,
    pub base_revision: Option<String>,
}

/// Sets or clears the direct paragraph-style reference using an existing style's display name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetParagraphStyle {
    pub target: TextTarget,
    pub style: PropertyPatch<String>,
    pub base_revision: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PictureTarget {
    pub name: Option<String>,
    pub description: Option<String>,
    pub occurrence: Option<usize>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImagePayload {
    pub content_type: String,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacePicture {
    pub target: PictureTarget,
    pub replacement: ImagePayload,
    pub base_revision: Option<String>,
}

/// Inserts one inline PNG or JPEG picture in the direct main-document body.
/// The engine determines the format and dimensions from the supplied bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsertPicture {
    pub image_bytes: Vec<u8>,
    pub placement: ParagraphPlacement,
    pub alt_text: Option<String>,
    pub base_revision: Option<String>,
}

/// An exact DOCX Current-view text search request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindText {
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindTextMatch {
    pub occurrence: usize,
    pub text: String,
    pub before: String,
    pub after: String,
    pub container: TextContainer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextContainer {
    Paragraph,
    TableCell,
}

impl TextContainer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paragraph => "paragraph",
            Self::TableCell => "table_cell",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindTextResult {
    pub query: String,
    pub matches: Vec<FindTextMatch>,
    pub diagnostics: Vec<Diagnostic>,
}

/// A request for the semantic container around one exact Current-view text target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectTextContext {
    pub target: TextTarget,
    pub before: usize,
    pub after: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextContextContainer {
    pub relative_position: isize,
    pub text: String,
    pub container: TextContainer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectTextContextResult {
    pub target: TextTarget,
    pub container: Option<TextContextContainer>,
    pub nearby: Vec<TextContextContainer>,
    pub diagnostics: Vec<Diagnostic>,
}

impl InspectTextContextResult {
    pub fn found(
        target: TextTarget,
        container: TextContextContainer,
        nearby: Vec<TextContextContainer>,
    ) -> Self {
        Self {
            target,
            container: Some(container),
            nearby,
            diagnostics: Vec::new(),
        }
    }

    pub fn failed(target: TextTarget, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            target,
            container: None,
            nearby: Vec::new(),
            diagnostics: vec![Diagnostic::new(code, DiagnosticSeverity::Error, message)],
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": self.diagnostics.is_empty(),
            "target": { "text": self.target.text, "occurrence": self.target.occurrence },
            "container": self.container.as_ref().map(context_container_json),
            "nearby": self.nearby.iter().map(context_container_json).collect::<Vec<_>>(),
            "diagnostics": self.diagnostics.iter().map(Diagnostic::to_json).collect::<Vec<_>>(),
        })
    }
}

fn context_container_json(container: &TextContextContainer) -> serde_json::Value {
    serde_json::json!({
        "relative_position": container.relative_position,
        "text": container.text,
        "container": container.container.as_str(),
    })
}

impl FindTextResult {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": self.diagnostics.is_empty(),
            "query": self.query,
            "match_count": self.matches.len(),
            "matches": self.matches.iter().map(|item| serde_json::json!({
                "occurrence": item.occurrence,
                "text": item.text,
                "before": item.before,
                "after": item.after,
                "container": item.container.as_str(),
            })).collect::<Vec<_>>(),
            "diagnostics": self.diagnostics.iter().map(Diagnostic::to_json).collect::<Vec<_>>(),
        })
    }
}

/// A bounded, DOCX-specific semantic inspection request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectDocx {
    pub focus: InspectDocxFocus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InspectDocxFocus {
    Overview,
    BodyBlocks { offset: usize, limit: usize },
    Headings { offset: usize, limit: usize },
    Paragraphs { offset: usize, limit: usize },
    Tables { offset: usize, limit: usize },
    Context(InspectTextContext),
}

/// A page of version-local, source-order semantic items.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionPage<T> {
    pub total: usize,
    pub offset: usize,
    pub returned: usize,
    pub has_more: bool,
    pub items: Vec<T>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxOverview {
    pub body_block_count: usize,
    pub paragraph_count: usize,
    pub table_count: usize,
    pub section_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxHeading {
    pub occurrence: usize,
    pub text: String,
    pub style_name: String,
    pub level: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxParagraph {
    pub occurrence: usize,
    pub handle: Option<String>,
    pub text: String,
    pub style_name: Option<String>,
}

/// One ordinary direct child of the document body in source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxBodyBlock {
    pub handle: String,
    pub kind: DocxBodyBlockKind,
    pub text: Option<String>,
    pub table_handle: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocxBodyBlockKind {
    Paragraph,
    Table,
}

impl DocxBodyBlockKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paragraph => "paragraph",
            Self::Table => "table",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxTable {
    pub occurrence: usize,
    pub handle: String,
    pub row_count: usize,
    pub is_rectangular: bool,
    pub affordances: Vec<Affordance>,
    pub columns: Vec<DocxTableColumn>,
    pub rows: Vec<DocxTableRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxTableRow {
    pub handle: String,
    pub cells: Vec<String>,
    pub cell_handles: Vec<String>,
    pub cell_affordances: Vec<Vec<Affordance>>,
}

/// Whether one implemented capability is structurally safe for an inspected object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Affordance {
    pub capability: CapabilityId,
    pub supported: bool,
    pub reason: Option<AffordanceReason>,
}

impl Affordance {
    pub fn supported(capability: &'static str) -> Self {
        Self {
            capability: CapabilityId::new(capability),
            supported: true,
            reason: None,
        }
    }

    pub fn unsupported(capability: &'static str, reason: AffordanceReason) -> Self {
        Self {
            capability: CapabilityId::new(capability),
            supported: false,
            reason: Some(reason),
        }
    }
}

/// A narrow, machine-readable reason why an inspected object is not editable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AffordanceReason {
    MultipleParagraphs,
    UnsafeCellStructure,
    UnsafeParagraphStructure,
    NonRectangularTable,
    MergedTableStructure,
    NestedTableStructure,
    RevisionWrapper,
    InvalidTableGrid,
    LastTableRow,
    LastTableColumn,
}

impl AffordanceReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MultipleParagraphs => "MULTIPLE_PARAGRAPHS",
            Self::UnsafeCellStructure => "UNSAFE_CELL_STRUCTURE",
            Self::UnsafeParagraphStructure => "UNSAFE_PARAGRAPH_STRUCTURE",
            Self::NonRectangularTable => "NON_RECTANGULAR_TABLE",
            Self::MergedTableStructure => "MERGED_TABLE_STRUCTURE",
            Self::NestedTableStructure => "NESTED_TABLE_STRUCTURE",
            Self::RevisionWrapper => "REVISION_WRAPPER",
            Self::InvalidTableGrid => "INVALID_TABLE_GRID",
            Self::LastTableRow => "LAST_TABLE_ROW",
            Self::LastTableColumn => "LAST_TABLE_COLUMN",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxTableColumn {
    pub occurrence: usize,
    pub handle: String,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InspectDocxContent {
    Overview(DocxOverview),
    BodyBlocks(InspectionPage<DocxBodyBlock>),
    Headings(InspectionPage<DocxHeading>),
    Paragraphs(InspectionPage<DocxParagraph>),
    Tables(InspectionPage<DocxTable>),
    Context(InspectTextContextResult),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectDocxResult {
    pub content: Option<InspectDocxContent>,
    pub diagnostics: Vec<Diagnostic>,
}

impl InspectDocxResult {
    pub fn success(content: InspectDocxContent) -> Self {
        Self {
            content: Some(content),
            diagnostics: Vec::new(),
        }
    }

    pub fn failed(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            content: None,
            diagnostics: vec![Diagnostic::new(code, DiagnosticSeverity::Error, message)],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationChange {
    pub kind: String,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationResult {
    pub status: OperationStatus,
    pub diagnostics: Vec<Diagnostic>,
    pub changes: Vec<OperationChange>,
}

impl OperationResult {
    pub fn applied(before: String, after: String) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "text_replaced".to_owned(),
                before,
                after,
            }],
        }
    }

    pub fn failed(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::failed_diagnostic(Diagnostic::new(code, DiagnosticSeverity::Error, message))
    }

    pub fn failed_diagnostic(diagnostic: Diagnostic) -> Self {
        Self {
            status: OperationStatus::Failed,
            diagnostics: vec![diagnostic],
            changes: Vec::new(),
        }
    }

    pub fn with_reason_code(mut self, reason_code: impl Into<String>) -> Self {
        let reason_code = reason_code.into();
        for diagnostic in &mut self.diagnostics {
            diagnostic.reason_code = Some(reason_code.clone());
        }
        self
    }

    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        let operation = operation.into();
        for diagnostic in &mut self.diagnostics {
            if diagnostic.operation.is_none() {
                diagnostic.operation = Some(operation.clone());
            }
        }
        self
    }

    pub fn with_target_handle(mut self, handle: impl Into<String>) -> Self {
        let handle = handle.into();
        for diagnostic in &mut self.diagnostics {
            if diagnostic.target.is_none() {
                diagnostic.target = Some(DiagnosticTarget {
                    handle: handle.clone(),
                });
            }
        }
        self
    }

    pub fn paragraph_inserted(anchor: String, text: String) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "paragraph_inserted".to_owned(),
                before: anchor,
                after: text,
            }],
        }
    }

    pub fn paragraph_deleted(before: String) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "paragraph_deleted".to_owned(),
                before,
                after: String::new(),
            }],
        }
    }

    pub fn table_cell_text_set(before: String, after: String) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "table_cell_text_set".to_owned(),
                before,
                after,
            }],
        }
    }

    pub fn table_row_inserted(headers: Vec<String>, anchor: String, cells: Vec<String>) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "table_row_inserted".to_owned(),
                before: format!("{} after {anchor}", headers.join(" | ")),
                after: cells.join(" | "),
            }],
        }
    }

    pub fn table_rows_inserted(
        headers: Vec<String>,
        anchor: String,
        rows: Vec<Vec<String>>,
    ) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "table_rows_inserted".to_owned(),
                before: format!("{} after {anchor}", headers.join(" | ")),
                after: format!("{} rows", rows.len()),
            }],
        }
    }

    pub fn table_cells_text_set(count: usize) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "table_cells_text_set".to_owned(),
                before: format!("{count} cells"),
                after: "updated".to_owned(),
            }],
        }
    }

    pub fn table_column_inserted(headers: Vec<String>, after: String, header: String) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "table_column_inserted".to_owned(),
                before: format!("{} after {after}", headers.join(" | ")),
                after: header,
            }],
        }
    }

    pub fn content_control_text_set(before: String, after: String) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "content_control_text_set".to_owned(),
                before,
                after,
            }],
        }
    }

    pub fn paragraph_formatting_set(text: String) -> Self {
        Self::formatting_set("paragraph_formatting_set", text)
    }

    pub fn text_formatting_set(text: String) -> Self {
        Self::formatting_set("text_formatting_set", text)
    }

    pub fn paragraph_style_set(text: String, before: String, after: String) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "paragraph_style_set".to_owned(),
                before: format!("{text} style: {before}"),
                after,
            }],
        }
    }
    pub fn picture_replaced(name: String, before: usize, after: usize) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "picture_replaced".to_owned(),
                before: format!("{name}: {before} bytes"),
                after: format!("{after} bytes"),
            }],
        }
    }

    pub fn picture_inserted() -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: "picture_inserted".to_owned(),
                before: "body".to_owned(),
                after: "inline picture added".to_owned(),
            }],
        }
    }

    fn formatting_set(kind: &str, text: String) -> Self {
        Self {
            status: OperationStatus::Applied,
            diagnostics: Vec::new(),
            changes: vec![OperationChange {
                kind: kind.to_owned(),
                before: format!("direct formatting for {text}"),
                after: "direct formatting updated".to_owned(),
            }],
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": self.status == OperationStatus::Applied,
            "status": self.status.as_str(),
            "changes": self.changes.iter().map(|change| serde_json::json!({
                "kind": change.kind,
                "before": change.before,
                "after": change.after,
            })).collect::<Vec<_>>(),
            "diagnostics": self.diagnostics.iter().map(Diagnostic::to_json).collect::<Vec<_>>(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationStatus {
    Applied,
    Failed,
}

impl OperationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Failed => "failed",
        }
    }
}

/// A machine-readable runtime diagnostic without source or persistence details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub reason_code: Option<String>,
    pub operation: Option<String>,
    pub target: Option<DiagnosticTarget>,
}

/// Public, opaque context for a diagnostic target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticTarget {
    pub handle: String,
}

impl Diagnostic {
    pub fn new(
        code: impl Into<String>,
        severity: DiagnosticSeverity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity,
            message: message.into(),
            reason_code: None,
            operation: None,
            target: None,
        }
    }

    pub fn with_reason_code(mut self, reason_code: impl Into<String>) -> Self {
        self.reason_code = Some(reason_code.into());
        self
    }

    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }

    pub fn with_target_handle(mut self, handle: impl Into<String>) -> Self {
        self.target = Some(DiagnosticTarget {
            handle: handle.into(),
        });
        self
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut value = serde_json::json!({
            "code": self.code,
            "severity": self.severity.as_str(),
            "message": self.message,
        });
        let object = value.as_object_mut().expect("diagnostic is an object");
        if let Some(reason_code) = &self.reason_code {
            object.insert("reason_code".to_owned(), reason_code.clone().into());
        }
        if let Some(operation) = &self.operation {
            object.insert("operation".to_owned(), operation.clone().into());
        }
        if let Some(target) = &self.target {
            object.insert(
                "target".to_owned(),
                serde_json::json!({ "handle": target.handle }),
            );
        }
        value
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

impl DiagnosticSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_the_protocol_layer() {
        assert_eq!(LAYER, "protocol");
    }

    #[test]
    fn reports_only_implemented_docx_capabilities_in_a_stable_order() {
        let capabilities = RuntimeCapabilities::docx("0.1.0");
        let json = capabilities.to_json().to_string();

        assert_eq!(
            json,
            r#"{"engine_version":"0.1.0","formats":[{"capabilities":["inspect","text","tables","styles","paragraph_formatting","numbering","sections","headers_footers","references","pictures","fields","content_controls","tracked_changes","comments","revision_views","replace_text","create_blank_docx","body_blocks","insert_paragraph","insert_paragraphs","insert_paragraph_after","delete_paragraph","set_table_cell_text","set_content_control_text","set_paragraph_formatting","set_text_formatting","set_paragraph_style","replace_picture","insert_picture","insert_table_row","insert_table_rows","set_table_cells_text","insert_table_column","create_table","delete_table","delete_table_row","delete_table_column","set_table_formatting","find_text","inspect_context"],"format":"docx"}],"protocol_version":1}"#
        );
        assert!(!json.contains("mutation"));
        assert!(!json.contains("render"));
        assert!(!json.contains("NodeId"));
        assert!(!json.contains("source_span"));
        assert!(!json.contains("xml_path"));
    }

    #[test]
    fn serializes_diagnostics_without_requiring_message_parsing() {
        let diagnostic = Diagnostic::new(
            "UNSUPPORTED_OPERATION",
            DiagnosticSeverity::Error,
            "operation is not supported",
        );

        assert_eq!(
            diagnostic.to_json().to_string(),
            r#"{"code":"UNSUPPORTED_OPERATION","message":"operation is not supported","severity":"error"}"#
        );

        let structured = Diagnostic::new(
            "UNSUPPORTED_OPERATION",
            DiagnosticSeverity::Error,
            "cell cannot be rewritten safely",
        )
        .with_reason_code("MULTIPLE_PARAGRAPHS")
        .with_operation("set_table_cells_text")
        .with_target_handle("t0:r1:c1")
        .to_json();
        assert_eq!(structured["reason_code"], "MULTIPLE_PARAGRAPHS");
        assert_eq!(structured["operation"], "set_table_cells_text");
        assert_eq!(structured["target"]["handle"], "t0:r1:c1");
    }
}
