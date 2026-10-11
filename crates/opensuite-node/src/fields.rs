use crate::{ParagraphPlacementInput, SimpleTask};
use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
use opensuite_protocol::{FieldContent, InsertFields, InsertToc, ParagraphPlacement};

#[napi(object)]
pub struct FieldInspectionInput {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}
#[napi(js_name = "inspectDocxFields", ts_return_type = "Promise<string>")]
pub fn inspect_fields(
    input: Buffer,
    options: Option<FieldInspectionInput>,
) -> AsyncTask<FieldInspectionTask> {
    let options = options.unwrap_or(FieldInspectionInput {
        offset: None,
        limit: None,
    });
    AsyncTask::new(FieldInspectionTask {
        input: input.to_vec(),
        offset: options.offset.unwrap_or(0) as usize,
        limit: options.limit.unwrap_or(20) as usize,
    })
}
pub struct FieldInspectionTask {
    input: Vec<u8>,
    offset: usize,
    limit: usize,
}
impl Task for FieldInspectionTask {
    type Output = String;
    type JsValue = String;
    fn compute(&mut self) -> Result<String> {
        serde_json::to_string(&opensuite_docx::inspect_docx_fields(
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
#[napi(object)]
/// @schema {"oneOf":[{"type":"object","properties":{"kind":{"const":"text"},"text":{"type":"string","maxLength":32000}},"required":["kind","text"],"additionalProperties":false},{"type":"object","properties":{"kind":{"type":"string","enum":["page","numPages"]}},"required":["kind"],"additionalProperties":false}]}
pub struct FieldContentInput {
    pub kind: String,
    pub text: Option<String>,
}
#[napi(object)]
pub struct InsertFieldsInput {
    /// @schema {"minItems":1,"maxItems":20,"items":{"oneOf":[{"type":"object","properties":{"kind":{"const":"text"},"text":{"type":"string","maxLength":32000}},"required":["kind","text"],"additionalProperties":false},{"type":"object","properties":{"kind":{"type":"string","enum":["page","numPages"]}},"required":["kind"],"additionalProperties":false}]}}
    pub content: Vec<FieldContentInput>,
    /// Body (default) requires placement. Footer appends to a single-section default footer.
    /// @schema {"enum":["body","footer"]}
    pub location: Option<String>,
    /// @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."}
    pub placement: Option<ParagraphPlacementInput>,
}
#[napi(object)]
pub struct InsertTocInput {
    /// @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."}
    pub placement: ParagraphPlacementInput,
    /// @schema {"minimum":1,"maximum":9}
    pub max_heading_level: Option<u32>,
    /// @schema {"maxLength":32000}
    pub title: Option<String>,
}
fn placement(input: ParagraphPlacementInput) -> Result<ParagraphPlacement> {
    match (input.kind.as_str(), input.handle) {
        ("start", None) => Ok(ParagraphPlacement::Start),
        ("end", None) => Ok(ParagraphPlacement::End),
        ("before", Some(handle)) => Ok(ParagraphPlacement::Before { handle }),
        ("after", Some(handle)) => Ok(ParagraphPlacement::After { handle }),
        _ => Err(napi::Error::from_reason(
            "placement must be start/end or before/after with a body block handle",
        )),
    }
}
/// Insert a new paragraph of plain text and real PAGE/NUMPAGES fields. content items use kind text with text, or page/numPages without text. Example: text Page , page, text of , numPages. Body requires placement; footer appends to the single-section default footer and omits placement. Cached values are ? and marked dirty; Word calculates them. Imported fields remain unchanged.
#[napi(
    js_name = "executeDocxInsertFields",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn insert_fields(
    input: Buffer,
    op: InsertFieldsInput,
) -> Result<AsyncTask<SimpleTask<InsertFields>>> {
    let content: Result<Vec<_>> = op
        .content
        .into_iter()
        .map(|item| match (item.kind.as_str(), item.text) {
            ("text", Some(text)) => Ok(FieldContent::Text(text)),
            ("page", None) => Ok(FieldContent::Page),
            ("numPages", None) => Ok(FieldContent::NumPages),
            _ => Err(napi::Error::from_reason(
                "content requires text with a text value, or page/numPages without a value",
            )),
        })
        .collect();
    let placement = match (op.location.as_deref().unwrap_or("body"), op.placement) {
        ("body", Some(value)) => Some(placement(value)?),
        ("footer", None) => None,
        _ => {
            return Err(napi::Error::from_reason(
                "body requires placement; footer cannot include placement",
            ));
        }
    };
    Ok(AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: InsertFields {
            content: content?,
            placement,
        },
        run: opensuite_docx::execute_docx_insert_fields,
    }))
}
/// Insert a real refreshable Word Table of Contents in a new body paragraph, with an optional title paragraph. Heading levels 1 through maxHeadingLevel (default 3, range 1–9). Marked dirty with a refresh placeholder. Inspect fields first to avoid an unintended duplicate TOC. No in-engine field evaluation.
#[napi(
    js_name = "executeDocxInsertToc",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn insert_toc(input: Buffer, op: InsertTocInput) -> Result<AsyncTask<SimpleTask<InsertToc>>> {
    let level = op.max_heading_level.unwrap_or(3);
    if !(1..=9).contains(&level) {
        return Err(napi::Error::from_reason("maxHeadingLevel must be 1–9"));
    }
    Ok(AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: InsertToc {
            placement: placement(op.placement)?,
            max_heading_level: level as u8,
            title: op.title,
        },
        run: opensuite_docx::execute_docx_insert_toc,
    }))
}
