use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
use opensuite_docx::{LayoutOptions, inspect_docx_layout};

#[napi(object)]
pub struct LayoutInput {
    pub block_offset: Option<u32>,
    pub block_limit: Option<u32>,
    pub section_index: Option<u32>,
}
#[napi(js_name = "inspectDocxLayout")]
pub fn inspect_layout(input: Buffer, options: Option<LayoutInput>) -> AsyncTask<LayoutTask> {
    let options = options
        .map(|v| LayoutOptions {
            block_offset: v.block_offset.unwrap_or(0) as usize,
            block_limit: v.block_limit.unwrap_or(0) as usize,
            section_index: v.section_index.map(|v| v as usize),
        })
        .unwrap_or_default();
    AsyncTask::new(LayoutTask {
        input: input.to_vec(),
        options,
    })
}
pub struct LayoutTask {
    input: Vec<u8>,
    options: LayoutOptions,
}
impl Task for LayoutTask {
    type Output = String;
    type JsValue = String;
    fn compute(&mut self) -> Result<String> {
        serde_json::to_string(&inspect_docx_layout(
            std::mem::take(&mut self.input),
            &self.options,
        ))
        .map_err(|e| napi::Error::from_reason(e.to_string()))
    }
    fn resolve(&mut self, _env: Env, output: String) -> Result<String> {
        Ok(output)
    }
}
