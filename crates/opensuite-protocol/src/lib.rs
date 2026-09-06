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
    "insert_paragraph_after",
    "delete_paragraph",
    "set_table_cell_text",
    "set_content_control_text",
    "set_paragraph_formatting",
    "set_text_formatting",
    "set_paragraph_style",
    "replace_picture",
    "insert_table_row",
    "insert_table_rows",
    "set_table_cells_text",
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
}

/// A simple table selected by its complete first-row cell text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableTarget {
    pub header_cells: Vec<String>,
    pub occurrence: Option<usize>,
}

/// A row selected by its exact Current-view first-cell text within a selected table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableRowTarget {
    pub first_cell_text: String,
    pub occurrence: Option<usize>,
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
    pub text: String,
    pub style_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxTable {
    pub occurrence: usize,
    pub row_count: usize,
    pub is_rectangular: bool,
    pub rows: Vec<DocxTableRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxTableRow {
    pub cells: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InspectDocxContent {
    Overview(DocxOverview),
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
        Self {
            status: OperationStatus::Failed,
            diagnostics: vec![Diagnostic::new(code, DiagnosticSeverity::Error, message)],
            changes: Vec::new(),
        }
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
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": self.code,
            "severity": self.severity.as_str(),
            "message": self.message,
        })
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
            r#"{"engine_version":"0.1.0","formats":[{"capabilities":["inspect","text","tables","styles","paragraph_formatting","numbering","sections","headers_footers","references","pictures","fields","content_controls","tracked_changes","comments","revision_views","replace_text","insert_paragraph_after","delete_paragraph","set_table_cell_text","set_content_control_text","set_paragraph_formatting","set_text_formatting","set_paragraph_style","replace_picture","insert_table_row","insert_table_rows","set_table_cells_text","find_text","inspect_context"],"format":"docx"}],"protocol_version":1}"#
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
    }
}
