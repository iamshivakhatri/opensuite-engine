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
    /// @schema {"enum":["footnote","endnote"]}
    pub kind: String,
    pub target: TextTargetInput,
    /// @schema {"minLength":1,"maxLength":32000}
    pub text: String,
}
#[napi(object)]
pub struct UpdateNoteInput {
    /// @schema {"minLength":1}
    pub handle: String,
    /// @schema {"minLength":1,"maxLength":32000}
    pub text: String,
}
#[napi(object)]
pub struct DeleteNoteInput {
    /// @schema {"minLength":1}
    pub handle: String,
}
#[napi(object)]
pub struct NoteInspectionInput {
    pub offset: Option<u32>,
    /// @schema {"minimum":1,"maximum":100}
    pub limit: Option<u32>,
}
/// Insert a real footnote or endnote immediately after exact ordinary text in one body paragraph. Plain one-paragraph note text. Independent comments/revisions are preserved; targets inside or crossing protected ranges, fields, wrappers, and tables refuse. Word controls visible numbering; IDs are not displayed note numbers.
#[napi(
    js_name = "executeDocxInsertNote",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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
/// Update a simple one-paragraph note using a fresh inspect_notes handle. Keeps its ID, reference, formatting, and other notes. Complex note bodies are preserved and cannot be edited. Re-inspect after every mutation.
#[napi(
    js_name = "executeDocxUpdateNote",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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
/// Delete an ordinary simple footnote/endnote and its body reference using a fresh inspect_notes handle. Remaining IDs are preserved. Protected references and complex/malformed notes refuse. Re-inspect after every mutation.
#[napi(
    js_name = "executeDocxDeleteNote",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn delete_note(input: Buffer, op: DeleteNoteInput) -> AsyncTask<SimpleTask<DeleteNote>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: DeleteNote { handle: op.handle },
        run: execute_docx_delete_note,
    })
}
/// Inspect ordinary footnotes and endnotes, fresh handles, note IDs, text, zero-based body paragraph and source reference indexes, reference counts, structural status, and imported numbering settings. Separators are excluded and protected. IDs differ from visible note numbers. Default 20, maximum 100 records per page; note text capped at 2000 characters. Read-only.
/// @jsonOutput
#[napi(js_name = "inspectDocxNotes", ts_return_type = "Promise<string>")]
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
