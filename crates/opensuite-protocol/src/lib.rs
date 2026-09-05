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
            r#"{"engine_version":"0.1.0","formats":[{"capabilities":["inspect","text","tables","styles","paragraph_formatting","numbering","sections","headers_footers","references","pictures","fields","content_controls","tracked_changes","comments","revision_views","replace_text","insert_paragraph_after","find_text","inspect_context"],"format":"docx"}],"protocol_version":1}"#
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
