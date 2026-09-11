use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi_derive::napi;
use opensuite_docx::execute_docx_set_content_control_text;
use opensuite_protocol::{ContentControlTarget, SetContentControlText};

use crate::SimpleTask;

#[napi(object)]
pub struct ContentControlTargetInput {
    pub tag: Option<String>,
    pub alias: Option<String>,
    pub occurrence: Option<u32>,
}

#[napi(object)]
pub struct SetContentControlTextInput {
    pub target: ContentControlTargetInput,
    pub expected_current_text: String,
    pub replacement: String,
    pub base_revision: Option<String>,
}

#[napi(js_name = "executeDocxSetContentControlText")]
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
