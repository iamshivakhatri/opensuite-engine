use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
use opensuite_docx::execute_docx_set_content_control_text;
use opensuite_protocol::{ContentControlTarget, SetContentControlText};

use crate::SimpleTask;

#[napi(object)]
pub struct ContentControlTargetInput {
    pub tag: Option<String>,
    pub alias: Option<String>,
    /// @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."}
    pub occurrence: Option<u32>,
}

#[napi(object)]
pub struct SetContentControlTextInput {
    pub target: ContentControlTargetInput,
    pub expected_current_text: String,
    pub replacement: String,
    pub base_revision: Option<String>,
}

/// Set content-control text by tag and/or alias.
#[napi(
    js_name = "executeDocxSetContentControlText",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_content_control_text_node(
    input: Buffer,
    operation: SetContentControlTextInput,
) -> AsyncTask<SimpleTask<SetContentControlText>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetContentControlText {
            target: ContentControlTarget {
                tag: operation.target.tag,
                alias: operation.target.alias,
                occurrence: operation.target.occurrence.map(|value| value as usize),
            },
            expected_current_text: operation.expected_current_text,
            replacement: operation.replacement,
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_content_control_text,
    })
}

#[napi(object)]
pub struct ContentControlInspectionInput {
    pub offset: Option<u32>,
    /// @schema {"minimum":1,"maximum":100}
    pub limit: Option<u32>,
}
/// Inspect main-document content controls: tag, alias, kind, bounded current text, part/paragraph location, exact mutation target with occurrence, and unsupported reasons. Copy target and complete text to set_content_control_text. Default 20, maximum 100 records; strings capped at 2000 characters. Truncated values cannot be used for mutation. Does not change XML.
/// @jsonOutput
#[napi(
    js_name = "inspectDocxContentControls",
    ts_return_type = "Promise<string>"
)]
pub fn inspect_content_controls(
    input: Buffer,
    options: Option<ContentControlInspectionInput>,
) -> AsyncTask<ContentControlInspectionTask> {
    let options = options.unwrap_or(ContentControlInspectionInput {
        offset: None,
        limit: None,
    });
    AsyncTask::new(ContentControlInspectionTask {
        input: input.to_vec(),
        offset: options.offset.unwrap_or(0) as usize,
        limit: options.limit.unwrap_or(20) as usize,
    })
}
pub struct ContentControlInspectionTask {
    input: Vec<u8>,
    offset: usize,
    limit: usize,
}
impl Task for ContentControlInspectionTask {
    type Output = String;
    type JsValue = String;
    fn compute(&mut self) -> Result<String> {
        serde_json::to_string(&opensuite_docx::inspect_docx_content_controls(
            std::mem::take(&mut self.input),
            self.offset,
            self.limit,
        ))
        .map_err(|e| napi::Error::from_reason(e.to_string()))
    }
    fn resolve(&mut self, _env: Env, output: String) -> Result<String> {
        Ok(output)
    }
}
