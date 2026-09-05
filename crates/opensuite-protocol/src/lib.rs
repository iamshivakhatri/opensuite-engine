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
    /// Declares the read-only DOCX capabilities implemented by this engine version.
    pub fn docx_read_only(engine_version: impl Into<String>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            engine_version: engine_version.into(),
            formats: vec![FormatCapabilities {
                format: DocumentFormat::Docx,
                capabilities: DOCX_READ_ONLY_CAPABILITIES
                    .iter()
                    .map(|id| CapabilityId::new(*id))
                    .collect(),
            }],
        }
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

const DOCX_READ_ONLY_CAPABILITIES: &[&str] = &[
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
];

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
        let capabilities = RuntimeCapabilities::docx_read_only("0.1.0");
        let json = capabilities.to_json().to_string();

        assert_eq!(
            json,
            r#"{"engine_version":"0.1.0","formats":[{"capabilities":["inspect","text","tables","styles","paragraph_formatting","numbering","sections","headers_footers","references","pictures","fields","content_controls","tracked_changes"],"format":"docx"}],"protocol_version":1}"#
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
