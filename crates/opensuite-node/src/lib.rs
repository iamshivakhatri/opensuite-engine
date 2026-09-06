use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
use opensuite_docx::{execute_docx_replace_text, find_docx_text, inspect_docx};
use opensuite_protocol::{
    Diagnostic, FindText, FindTextResult, InspectTextContext, InspectTextContextResult,
    OperationResult, ReplaceText, RuntimeCapabilities, TextContainer, TextTarget,
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
pub struct InspectDocxInput {
    pub target: TextTargetInput,
    pub before: Option<u32>,
    pub after: Option<u32>,
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
    pub target: TextTargetInput,
    pub container: Option<TextContextContainerOutput>,
    pub nearby: Vec<TextContextContainerOutput>,
    pub diagnostics: Vec<DiagnosticOutput>,
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
    AsyncTask::new(InspectDocxTask {
        input: input.to_vec(),
        request: InspectTextContext {
            target: text_target(request.target),
            before: request.before.unwrap_or(1) as usize,
            after: request.after.unwrap_or(1) as usize,
        },
    })
}

pub struct InspectDocxTask {
    input: Vec<u8>,
    request: InspectTextContext,
}

impl Task for InspectDocxTask {
    type Output = InspectTextContextResult;
    type JsValue = InspectDocxOutput;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(inspect_docx(std::mem::take(&mut self.input), &self.request))
    }

    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(inspect_docx_output(result))
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

fn inspect_docx_output(result: InspectTextContextResult) -> InspectDocxOutput {
    let context_container =
        |item: opensuite_protocol::TextContextContainer| TextContextContainerOutput {
            relative_position: item.relative_position as i32,
            text: item.text,
            container: container_name(item.container),
        };
    InspectDocxOutput {
        ok: result.diagnostics.is_empty(),
        target: TextTargetInput {
            text: result.target.text,
            occurrence: result.target.occurrence.map(|value| value as u32),
        },
        container: result.container.map(context_container),
        nearby: result.nearby.into_iter().map(context_container).collect(),
        diagnostics: result
            .diagnostics
            .into_iter()
            .map(diagnostic_output)
            .collect(),
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
