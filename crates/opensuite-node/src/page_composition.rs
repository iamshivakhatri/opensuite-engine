use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
use opensuite_docx::{
    DocxExecutionResult, execute_docx_delete_page_break, execute_docx_insert_page_break,
    execute_docx_set_header_footer_text, execute_docx_set_page_number, execute_docx_set_page_setup,
};
use opensuite_protocol::{
    DeletePageBreak, HeaderFooterKind, InsertPageBreak, OperationResult, PageBreakTarget,
    PageMargins, PageNumberAlignment, PageOrientation, PaperSize, ParagraphPlacement,
    SetHeaderFooterText, SetPageNumber, SetPageSetup,
};

use crate::{ExecuteDocxReplaceTextOutput, ParagraphPlacementInput, operation_result_output};

#[napi(object)]
pub struct InsertPageBreakInput {
    /// @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."}
    pub placement: ParagraphPlacementInput,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct DeletePageBreakInput {
    pub handle: String,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct SetPageSetupInput {
    pub top_margin_twips: Option<i32>,
    pub right_margin_twips: Option<i32>,
    pub bottom_margin_twips: Option<i32>,
    pub left_margin_twips: Option<i32>,
    /// @schema {"enum":["letter","a4"]}
    pub paper_size: Option<String>,
    /// @schema {"enum":["portrait","landscape"]}
    pub orientation: Option<String>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct SetHeaderFooterTextInput {
    /// @schema {"enum":["header","footer"]}
    pub kind: String,
    /// @schema {"description":"Omit to clear the text."}
    pub text: Option<String>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct SetPageNumberInput {
    /// @schema {"enum":["header","footer"]}
    pub kind: String,
    /// @schema {"enum":["left","center","right"],"description":"Omit to remove the page number."}
    pub alignment: Option<String>,
    pub base_revision: Option<String>,
}

/// Insert a page break at a paragraph placement (start/end/before/after).
#[napi(
    js_name = "executeDocxInsertPageBreak",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_insert_page_break_node(
    input: Buffer,
    operation: InsertPageBreakInput,
) -> AsyncTask<PageCompositionTask<InsertPageBreak>> {
    PageCompositionTask::new(
        input,
        paragraph_placement(operation.placement).map(|placement| InsertPageBreak {
            placement,
            base_revision: operation.base_revision,
        }),
        execute_docx_insert_page_break,
    )
}

/// Delete a page break identified by an opaque body_blocks handle.
#[napi(
    js_name = "executeDocxDeletePageBreak",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_delete_page_break_node(
    input: Buffer,
    operation: DeletePageBreakInput,
) -> AsyncTask<PageCompositionTask<DeletePageBreak>> {
    PageCompositionTask::new(
        input,
        Ok(DeletePageBreak {
            target: PageBreakTarget {
                handle: operation.handle,
            },
            base_revision: operation.base_revision,
        }),
        execute_docx_delete_page_break,
    )
}

/// Update section page margins, paper size (letter/a4), or orientation when section properties exist.
#[napi(
    js_name = "executeDocxSetPageSetup",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_page_setup_node(
    input: Buffer,
    operation: SetPageSetupInput,
) -> AsyncTask<PageCompositionTask<SetPageSetup>> {
    PageCompositionTask::new(input, page_setup(operation), execute_docx_set_page_setup)
}

/// Set or clear header/footer text.
#[napi(
    js_name = "executeDocxSetHeaderFooterText",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_header_footer_text_node(
    input: Buffer,
    operation: SetHeaderFooterTextInput,
) -> AsyncTask<PageCompositionTask<SetHeaderFooterText>> {
    PageCompositionTask::new(
        input,
        header_footer_kind(&operation.kind).map(|kind| SetHeaderFooterText {
            kind,
            text: operation.text,
            base_revision: operation.base_revision,
        }),
        execute_docx_set_header_footer_text,
    )
}

/// Set or remove page-number fields in header/footer.
#[napi(
    js_name = "executeDocxSetPageNumber",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_page_number_node(
    input: Buffer,
    operation: SetPageNumberInput,
) -> AsyncTask<PageCompositionTask<SetPageNumber>> {
    PageCompositionTask::new(
        input,
        header_footer_kind(&operation.kind).and_then(|kind| {
            page_number_alignment(operation.alignment).map(|alignment| SetPageNumber {
                kind,
                alignment,
                base_revision: operation.base_revision,
            })
        }),
        execute_docx_set_page_number,
    )
}

pub struct PageCompositionTask<T> {
    input: Vec<u8>,
    operation: std::result::Result<T, &'static str>,
    run: fn(Vec<u8>, &T) -> DocxExecutionResult,
}

impl<T: Send> PageCompositionTask<T> {
    pub(crate) fn new(
        input: Buffer,
        operation: std::result::Result<T, &'static str>,
        run: fn(Vec<u8>, &T) -> DocxExecutionResult,
    ) -> AsyncTask<Self> {
        AsyncTask::new(Self {
            input: input.to_vec(),
            operation,
            run,
        })
    }
}

impl<T: Send> Task for PageCompositionTask<T> {
    type Output = DocxExecutionResult;
    type JsValue = ExecuteDocxReplaceTextOutput;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(match &self.operation {
            Ok(operation) => (self.run)(std::mem::take(&mut self.input), operation),
            Err(message) => DocxExecutionResult {
                operation: OperationResult::failed("INVALID_OPERATION", *message),
                output_artifact: None,
            },
        })
    }

    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(ExecuteDocxReplaceTextOutput {
            result: operation_result_output(result.operation),
            output: result.output_artifact.map(Buffer::from),
        })
    }
}

pub(crate) fn paragraph_placement(
    input: ParagraphPlacementInput,
) -> std::result::Result<ParagraphPlacement, &'static str> {
    match input.kind.as_str() {
        "start" => Ok(ParagraphPlacement::Start),
        "end" => Ok(ParagraphPlacement::End),
        "before" => input
            .handle
            .map(|handle| ParagraphPlacement::Before { handle })
            .ok_or("before placement requires a body block handle"),
        "after" => input
            .handle
            .map(|handle| ParagraphPlacement::After { handle })
            .ok_or("after placement requires a body block handle"),
        _ => Err("page-break placement is not supported"),
    }
}

pub(crate) fn page_setup(
    input: SetPageSetupInput,
) -> std::result::Result<SetPageSetup, &'static str> {
    let margins = [
        input.top_margin_twips,
        input.right_margin_twips,
        input.bottom_margin_twips,
        input.left_margin_twips,
    ];
    let paper_size = match input.paper_size.as_deref() {
        None => Ok(None),
        Some("letter") => Ok(Some(PaperSize::Letter)),
        Some("a4") => Ok(Some(PaperSize::A4)),
        Some(_) => Err("paper size is not supported"),
    }?;
    let orientation = match input.orientation.as_deref() {
        None => Ok(None),
        Some("portrait") => Ok(Some(PageOrientation::Portrait)),
        Some("landscape") => Ok(Some(PageOrientation::Landscape)),
        Some(_) => Err("page orientation is not supported"),
    }?;
    Ok(SetPageSetup {
        margins: margins.iter().any(Option::is_some).then_some(PageMargins {
            top_twips: margins[0],
            right_twips: margins[1],
            bottom_twips: margins[2],
            left_twips: margins[3],
        }),
        paper_size,
        orientation,
        base_revision: input.base_revision,
    })
}

pub(crate) fn header_footer_kind(
    value: &str,
) -> std::result::Result<HeaderFooterKind, &'static str> {
    match value {
        "header" => Ok(HeaderFooterKind::Header),
        "footer" => Ok(HeaderFooterKind::Footer),
        _ => Err("header/footer kind is not supported"),
    }
}

pub(crate) fn page_number_alignment(
    value: Option<String>,
) -> std::result::Result<Option<PageNumberAlignment>, &'static str> {
    match value.as_deref() {
        None => Ok(None),
        Some("left") => Ok(Some(PageNumberAlignment::Left)),
        Some("center") => Ok(Some(PageNumberAlignment::Center)),
        Some("right") => Ok(Some(PageNumberAlignment::Right)),
        Some(_) => Err("page-number alignment is not supported"),
    }
}
