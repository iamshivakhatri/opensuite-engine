use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi_derive::napi;
use opensuite_docx::execute_docx_set_paragraphs_list;
use opensuite_protocol::{ParagraphListKind, SetParagraphsList};

use crate::{SimpleTask, TextTargetInput, text_target};

#[napi(object)]
pub struct SetParagraphsListInput {
    pub targets: Vec<TextTargetInput>,
    /// @schema {"enum":["bullet","decimal","none"]}
    pub kind: String,
    /// @schema {"enum":[0,1,2]}
    pub level: Option<u32>,
    pub continue_from_previous: Option<bool>,
    pub base_revision: Option<String>,
}

/// Apply list formatting to one or more paragraph text targets.
#[napi(
    js_name = "executeDocxSetParagraphsList",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_paragraphs_list_node(
    input: Buffer,
    operation: SetParagraphsListInput,
) -> AsyncTask<SimpleTask<SetParagraphsList>> {
    let kind = match operation.kind.as_str() {
        "bullet" => ParagraphListKind::Bullet,
        "decimal" => ParagraphListKind::Decimal,
        _ => ParagraphListKind::None,
    };
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetParagraphsList {
            targets: operation.targets.into_iter().map(text_target).collect(),
            kind,
            level: operation.level.unwrap_or(0) as u8,
            continue_from_previous: operation.continue_from_previous.unwrap_or(false),
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_paragraphs_list,
    })
}
