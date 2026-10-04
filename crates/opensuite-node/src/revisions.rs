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
