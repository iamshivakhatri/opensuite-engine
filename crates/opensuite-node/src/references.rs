use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;

#[napi(object)]
pub struct ReferenceInspectionInput {
    pub offset: Option<u32>,
    /// @schema {"minimum":1,"maximum":100}
    pub limit: Option<u32>,
}
/// Inspect main-document bookmarks and internal/external hyperlinks in source order, including part/paragraph locations, bookmarkName destinations, exact text targets and malformed/ambiguous reasons. Default 20/max 100 records; strings capped at 2000 characters. Does not change XML. Copy bookmarkName into set_hyperlink; refresh body-block movementHandle values before moving paragraphs.
/// @jsonOutput
#[napi(js_name = "inspectDocxReferences", ts_return_type = "Promise<string>")]
pub fn inspect_references(
    input: Buffer,
    options: Option<ReferenceInspectionInput>,
) -> AsyncTask<ReferenceInspectionTask> {
    let options = options.unwrap_or(ReferenceInspectionInput {
        offset: None,
        limit: None,
    });
    AsyncTask::new(ReferenceInspectionTask {
        input: input.to_vec(),
        offset: options.offset.unwrap_or(0) as usize,
        limit: options.limit.unwrap_or(20) as usize,
    })
}
pub struct ReferenceInspectionTask {
    input: Vec<u8>,
    offset: usize,
    limit: usize,
}
impl Task for ReferenceInspectionTask {
    type Output = String;
    type JsValue = String;
    fn compute(&mut self) -> Result<String> {
        serde_json::to_string(&opensuite_docx::inspect_docx_references(
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
