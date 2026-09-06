use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
use opensuite_docx::{execute_docx_replace_text, find_docx_text, inspect_docx};
use opensuite_protocol::{
    Diagnostic, DocxHeading, DocxOverview, DocxParagraph, DocxTable, DocxTableRow, FindText,
    FindTextResult, InspectDocx, InspectDocxContent, InspectDocxFocus, InspectDocxResult,
    InspectTextContext, InspectTextContextResult, InspectionPage, OperationResult, ReplaceText,
    RuntimeCapabilities, TextContainer, TextTarget,
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
pub struct DiagnosticOutput {
    pub code: String,
    pub severity: String,
    pub message: String,
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
    pub text: String,
    pub style_name: Option<String>,
}

#[napi(object)]
pub struct TableRowOutput {
    pub cells: Vec<String>,
}

#[napi(object)]
pub struct TableOutput {
    pub occurrence: u32,
    pub row_count: u32,
    pub is_rectangular: bool,
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
                text: item.text,
                style_name: item.style_name,
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
        row_count: value.row_count as u32,
        is_rectangular: value.is_rectangular,
        rows: value
            .rows
            .into_iter()
            .map(|DocxTableRow { cells }| TableRowOutput { cells })
            .collect(),
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
