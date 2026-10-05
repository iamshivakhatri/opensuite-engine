use crate::{SimpleTask, TextTargetInput};
use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
use opensuite_docx::{
    execute_docx_delete_note, execute_docx_insert_note, execute_docx_update_note,
    inspect_docx_notes,
};
use opensuite_protocol::{DeleteNote, InsertNote, NoteKind, TextTarget, UpdateNote};
#[napi(object)]
pub struct InsertNoteInput {
    pub kind: String,
    pub target: TextTargetInput,
    pub text: String,
}
#[napi(object)]
pub struct UpdateNoteInput {
    pub handle: String,
    pub text: String,
}
#[napi(object)]
pub struct DeleteNoteInput {
    pub handle: String,
}
#[napi(object)]
pub struct NoteInspectionInput {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}
#[napi(js_name = "executeDocxInsertNote")]
pub fn insert_note(
    input: Buffer,
    op: InsertNoteInput,
) -> Result<AsyncTask<SimpleTask<InsertNote>>> {
    let kind = match op.kind.as_str() {
        "footnote" => NoteKind::Footnote,
        "endnote" => NoteKind::Endnote,
        _ => return Err(napi::Error::from_reason("kind must be footnote or endnote")),
    };
    Ok(AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: InsertNote {
            kind,
            target: TextTarget {
                text: op.target.text,
                occurrence: op.target.occurrence.map(|n| n as usize),
            },
            text: op.text,
        },
        run: execute_docx_insert_note,
    }))
}
#[napi(js_name = "executeDocxUpdateNote")]
pub fn update_note(input: Buffer, op: UpdateNoteInput) -> AsyncTask<SimpleTask<UpdateNote>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: UpdateNote {
            handle: op.handle,
            text: op.text,
        },
        run: execute_docx_update_note,
    })
}
#[napi(js_name = "executeDocxDeleteNote")]
pub fn delete_note(input: Buffer, op: DeleteNoteInput) -> AsyncTask<SimpleTask<DeleteNote>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: DeleteNote { handle: op.handle },
        run: execute_docx_delete_note,
    })
}
#[napi(js_name = "inspectDocxNotes")]
pub fn inspect_notes(
    input: Buffer,
    options: Option<NoteInspectionInput>,
) -> AsyncTask<NoteInspectionTask> {
    let options = options.unwrap_or(NoteInspectionInput {
        offset: None,
        limit: None,
    });
    AsyncTask::new(NoteInspectionTask {
        input: input.to_vec(),
        offset: options.offset.unwrap_or(0) as usize,
        limit: options.limit.unwrap_or(20) as usize,
    })
}
pub struct NoteInspectionTask {
    input: Vec<u8>,
    offset: usize,
    limit: usize,
}
impl Task for NoteInspectionTask {
    type Output = String;
    type JsValue = String;
    fn compute(&mut self) -> Result<String> {
        serde_json::to_string(&inspect_docx_notes(
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
