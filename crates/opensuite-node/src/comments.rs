use crate::{SimpleTask, TextTargetInput};
use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
use opensuite_docx::{
    execute_docx_add_comment, execute_docx_delete_comment, execute_docx_update_comment,
    inspect_docx_comments,
};
use opensuite_protocol::{AddComment, DeleteComment, TextTarget, UpdateComment};

#[napi(object)]
pub struct AddCommentInput {
    pub target: TextTargetInput,
    /// @schema {"minLength":1,"maxLength":32000}
    pub text: String,
    /// @schema {"minLength":1}
    pub author: String,
    pub initials: Option<String>,
    /// Explicit ISO timestamp. The engine-client supplies the current time when omitted.
    /// @schema {"description":"ISO timestamp, e.g. 2026-10-04T12:00:00Z","x-client-default":"currentUtc"}
    pub date: String,
}
#[napi(object)]
pub struct UpdateCommentInput {
    pub handle: String,
    /// @schema {"minLength":1,"maxLength":32000}
    pub text: String,
}
#[napi(object)]
pub struct DeleteCommentInput {
    pub handle: String,
}
#[napi(object)]
pub struct CommentInspectionInput {
    pub offset: Option<u32>,
    /// @schema {"minimum":1,"maximum":100}
    pub limit: Option<u32>,
}

/// Attach a standard Word comment to exact text in one ordinary body paragraph, including simple formatted runs. Author is explicit; the client supplies current UTC when date is omitted. Wrappers, fields, revisions, cross-paragraph ranges, and threaded comments are unsupported.
#[napi(
    js_name = "executeDocxAddComment",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn add_comment(input: Buffer, operation: AddCommentInput) -> AsyncTask<SimpleTask<AddComment>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: AddComment {
            target: TextTarget {
                text: operation.target.text,
                occurrence: operation.target.occurrence.map(|v| v as usize),
            },
            text: operation.text,
            author: operation.author,
            initials: operation.initials,
            date: operation.date,
        },
        run: execute_docx_add_comment,
    })
}
/// Replace plain comment text using a fresh inspect_comments handle. Anchor, author, initials, and date stay unchanged. Inspect again after every edit.
#[napi(
    js_name = "executeDocxUpdateComment",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn update_comment(
    input: Buffer,
    operation: UpdateCommentInput,
) -> AsyncTask<SimpleTask<UpdateComment>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: UpdateComment {
            handle: operation.handle,
            text: operation.text,
        },
        run: execute_docx_update_comment,
    })
}
/// Delete a standard comment and its markers using a fresh inspect_comments handle, preserving the selected document text. Inspect again after every edit.
#[napi(
    js_name = "executeDocxDeleteComment",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn delete_comment(
    input: Buffer,
    operation: DeleteCommentInput,
) -> AsyncTask<SimpleTask<DeleteComment>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: DeleteComment {
            handle: operation.handle,
        },
        run: execute_docx_delete_comment,
    })
}
/// Inspect standard comments, author/date, bounded text, attached text, source-order paragraph locations, fresh handles, and malformed/orphan diagnostics. Default 20; maximum 100 per page. Re-inspect after editing. Resolved state and replies are unavailable.
/// @jsonOutput
#[napi(js_name = "inspectDocxComments", ts_return_type = "Promise<string>")]
pub fn inspect_comments(
    input: Buffer,
    options: Option<CommentInspectionInput>,
) -> AsyncTask<CommentInspectionTask> {
    let options = options.unwrap_or(CommentInspectionInput {
        offset: None,
        limit: None,
    });
    AsyncTask::new(CommentInspectionTask {
        input: input.to_vec(),
        offset: options.offset.unwrap_or(0) as usize,
        limit: options.limit.unwrap_or(20) as usize,
    })
}
pub struct CommentInspectionTask {
    input: Vec<u8>,
    offset: usize,
    limit: usize,
}
impl Task for CommentInspectionTask {
    type Output = String;
    type JsValue = String;
    fn compute(&mut self) -> Result<String> {
        serde_json::to_string(&inspect_docx_comments(
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
