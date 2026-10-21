use crate::page_composition::PageCompositionTask;
use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi_derive::napi;
use opensuite_protocol::{MoveParagraph, ParagraphMovePosition, ParagraphMoveTarget};

#[napi(object)]
pub struct ParagraphMoveTargetInput {
    /// Fresh movementHandle from inspectDocx body_blocks; ordinary bN handles are refused.
    pub handle: String,
}

#[napi(object)]
pub struct MoveParagraphInput {
    pub target: ParagraphMoveTargetInput,
    pub destination: ParagraphMoveTargetInput,
    /// @schema {"enum":["before","after"]}
    pub position: String,
    pub base_revision: Option<String>,
}

/// Move an original ordinary body paragraph before/after another in the same section. Both need fresh movementHandle identities. Preserves original XML, styles and numbering; refuses protected/dependent structures, section boundaries and stale targets. Same-position requests succeed without XML changes. baseRevision is caller metadata only.
#[napi(
    js_name = "executeDocxMoveParagraph",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_move_paragraph(
    input: Buffer,
    operation: MoveParagraphInput,
) -> AsyncTask<PageCompositionTask<MoveParagraph>> {
    let position = match operation.position.as_str() {
        "before" => Ok(ParagraphMovePosition::Before),
        "after" => Ok(ParagraphMovePosition::After),
        _ => Err("position must be before or after"),
    };
    PageCompositionTask::new(
        input,
        position.map(|position| MoveParagraph {
            target: ParagraphMoveTarget {
                handle: operation.target.handle,
            },
            destination: ParagraphMoveTarget {
                handle: operation.destination.handle,
            },
            position,
            base_revision: operation.base_revision,
        }),
        opensuite_docx::execute_docx_move_paragraph,
    )
}
