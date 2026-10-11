use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi_derive::napi;
use opensuite_docx::execute_docx_set_hyperlink;
use opensuite_protocol::SetHyperlink;

use crate::{SimpleTask, TextTargetInput, text_target};

#[napi(object)]
pub struct SetHyperlinkInput {
    pub target: TextTargetInput,
    /// @schema {"description":"External URL. Omit to clear the hyperlink."}
    pub url: Option<String>,
    pub base_revision: Option<String>,
}

/// Add or clear an external hyperlink on a text target.
#[napi(
    js_name = "executeDocxSetHyperlink",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_hyperlink_node(
    input: Buffer,
    operation: SetHyperlinkInput,
) -> AsyncTask<SimpleTask<SetHyperlink>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetHyperlink {
            target: text_target(operation.target),
            url: operation.url,
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_hyperlink,
    })
}
