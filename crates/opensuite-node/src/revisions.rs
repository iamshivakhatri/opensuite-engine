use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;

#[napi(object)]
pub struct RevisionInspectionInput {
    pub offset: Option<u32>,
    /// @schema {"minimum":1,"maximum":100}
    pub limit: Option<u32>,
}

/// Inspect existing Word tracked insertions/deletions: IDs, author/date, bounded text, source-order paragraph locations, totals of XML records/markers, original Word element names (revisionType), and unsupported move/property/range diagnostics. Main document including tables only. Default 20, maximum 100 per page; text/metadata capped at 2000 characters. Current document text includes insertions and excludes deletions. Does not edit or save a version.
/// @jsonOutput
#[napi(
    js_name = "inspectDocxTrackedChanges",
    ts_return_type = "Promise<string>"
)]
pub fn inspect_tracked_changes(
    input: Buffer,
    options: Option<RevisionInspectionInput>,
) -> AsyncTask<RevisionInspectionTask> {
    let options = options.unwrap_or(RevisionInspectionInput {
        offset: None,
        limit: None,
    });
    AsyncTask::new(RevisionInspectionTask {
        input: input.to_vec(),
        offset: options.offset.unwrap_or(0) as usize,
        limit: options.limit.unwrap_or(20) as usize,
    })
}

pub struct RevisionInspectionTask {
    input: Vec<u8>,
    offset: usize,
    limit: usize,
}
impl Task for RevisionInspectionTask {
    type Output = String;
    type JsValue = String;
    fn compute(&mut self) -> Result<String> {
        serde_json::to_string(&opensuite_docx::inspect_docx_tracked_changes(
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

use crate::{SimpleTask, TextTargetInput, text_target};
use opensuite_protocol::{
    DeleteTrackedText, InsertTrackedText, ReplaceTextWithTrackedChange, TrackedTextPosition,
};

#[napi(object)]
pub struct InsertTrackedTextInput {
    pub target: TextTargetInput,
    /// @schema {"minLength":1,"maxLength":32000}
    pub text: String,
    /// @schema {"enum":["before","after"]}
    pub position: Option<String>,
    /// @schema {"minLength":1}
    pub author: String,
    /// @schema {"description":"UTC ISO timestamp","x-client-default":"currentUtc"}
    pub date: String,
}
#[napi(object)]
pub struct DeleteTrackedTextInput {
    pub target: TextTargetInput,
    /// @schema {"minLength":1}
    pub author: String,
    /// @schema {"description":"UTC ISO timestamp","x-client-default":"currentUtc"}
    pub date: String,
}
#[napi(object)]
pub struct ReplaceTextWithTrackedChangeInput {
    pub target: TextTargetInput,
    /// @schema {"minLength":1,"maxLength":32000}
    pub replacement: String,
    /// @schema {"minLength":1}
    pub author: String,
    /// @schema {"description":"UTC ISO timestamp","x-client-default":"currentUtc"}
    pub date: String,
}

/// Insert text as a real Word tracked insertion before/after an exact ordinary text anchor in one body paragraph. Default after. Inherits formatting at the insertion boundary. Author is explicit; the client supplies current UTC when date is omitted. Fields, wrappers, tables, and selections crossing revisions or protected ranges are unsupported. Re-inspect after editing.
#[napi(
    js_name = "executeDocxInsertTrackedText",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn insert_tracked_text(
    input: Buffer,
    op: InsertTrackedTextInput,
) -> Result<AsyncTask<SimpleTask<InsertTrackedText>>> {
    let position = match op.position.as_deref().unwrap_or("after") {
        "before" => TrackedTextPosition::Before,
        "after" => TrackedTextPosition::After,
        _ => return Err(napi::Error::from_reason("position must be before or after")),
    };
    Ok(AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: InsertTrackedText {
            target: text_target(op.target),
            text: op.text,
            position,
            author: op.author,
            date: op.date,
        },
        run: opensuite_docx::execute_docx_insert_tracked_text,
    }))
}
/// Delete exact ordinary text as a real Word tracked deletion, retaining deleted text and run formatting for review. One body paragraph; simple multiple runs supported. Author explicit; the client supplies current UTC when date is omitted. Fields, wrappers, tables, and selections crossing revisions or protected ranges are unsupported. Re-inspect after editing.
#[napi(
    js_name = "executeDocxDeleteTrackedText",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn delete_tracked_text(
    input: Buffer,
    op: DeleteTrackedTextInput,
) -> AsyncTask<SimpleTask<DeleteTrackedText>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: DeleteTrackedText {
            target: text_target(op.target),
            author: op.author,
            date: op.date,
        },
        run: opensuite_docx::execute_docx_delete_tracked_text,
    })
}
/// Replace exact ordinary text with real Word deletion + insertion revisions, preserving source formatting. Simple multiple runs require identical formatting. Author explicit; the client supplies current UTC when date is omitted. Fields, wrappers, tables, and selections crossing revisions or protected ranges are unsupported. Re-inspect after editing.
#[napi(
    js_name = "executeDocxReplaceTextWithTrackedChange",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn replace_text_with_tracked_change(
    input: Buffer,
    op: ReplaceTextWithTrackedChangeInput,
) -> AsyncTask<SimpleTask<ReplaceTextWithTrackedChange>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: ReplaceTextWithTrackedChange {
            target: text_target(op.target),
            replacement: op.replacement,
            author: op.author,
            date: op.date,
        },
        run: opensuite_docx::execute_docx_replace_text_with_tracked_change,
    })
}

#[napi(object)]
pub struct RevisionDecisionInput {
    /// @schema {"minLength":1}
    pub handle: String,
}

/// Accept one supported insertion or deletion using its fresh inspect_tracked_changes handle. Keeps inserted content or removes deleted content. For replacement, decide both records individually and re-inspect after each successful decision. Complex revisions are unsupported.
#[napi(
    js_name = "executeDocxAcceptRevision",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn accept_revision(
    input: Buffer,
    op: RevisionDecisionInput,
) -> AsyncTask<SimpleTask<opensuite_protocol::AcceptRevision>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: opensuite_protocol::AcceptRevision { handle: op.handle },
        run: opensuite_docx::execute_docx_accept_revision,
    })
}
/// Reject one supported insertion or deletion using its fresh inspect_tracked_changes handle. Removes inserted content or restores deleted content with its formatting. For replacement, decide both records individually and re-inspect after each successful decision. Complex revisions are unsupported.
#[napi(
    js_name = "executeDocxRejectRevision",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn reject_revision(
    input: Buffer,
    op: RevisionDecisionInput,
) -> AsyncTask<SimpleTask<opensuite_protocol::RejectRevision>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: opensuite_protocol::RejectRevision { handle: op.handle },
        run: opensuite_docx::execute_docx_reject_revision,
    })
}
