use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi_derive::napi;
use opensuite_docx::execute_docx_set_hyperlink;
use opensuite_protocol::SetHyperlink;

use crate::{SimpleTask, TextTargetInput, text_target};

#[napi(object)]
pub struct SetHyperlinkInput {
    pub target: TextTargetInput,
    /// @schema {"description":"External HTTP(S) URL. Supply url or bookmarkName; omit both to clear."}
    pub url: Option<String>,
    /// Exact existing bookmark name; no new bookmark or relationship is created.
    pub bookmark_name: Option<String>,
    pub base_revision: Option<String>,
}

/// Add an external URL or internal bookmark link on exact ordinary text, or clear an exact hyperlink.
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
            bookmark_name: operation.bookmark_name,
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_hyperlink,
    })
}
