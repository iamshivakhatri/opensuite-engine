use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;

#[napi(object)]
pub struct RevisionInspectionInput {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

#[napi(js_name = "inspectDocxTrackedChanges")]
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
    pub text: String,
    pub position: Option<String>,
    pub author: String,
    pub date: String,
}
#[napi(object)]
pub struct DeleteTrackedTextInput {
    pub target: TextTargetInput,
    pub author: String,
    pub date: String,
}
#[napi(object)]
pub struct ReplaceTextWithTrackedChangeInput {
    pub target: TextTargetInput,
    pub replacement: String,
    pub author: String,
    pub date: String,
}

#[napi(js_name = "executeDocxInsertTrackedText")]
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
#[napi(js_name = "executeDocxDeleteTrackedText")]
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
#[napi(js_name = "executeDocxReplaceTextWithTrackedChange")]
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
