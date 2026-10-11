use crate::{
    ParagraphPlacementInput,
    page_composition::{
        PageCompositionTask, SetPageSetupInput, header_footer_kind, page_number_alignment,
        page_setup, paragraph_placement,
    },
};
use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
use opensuite_docx::{
    execute_docx_insert_section_break, execute_docx_set_odd_even_headers,
    execute_docx_set_section_header_footer, execute_docx_set_section_properties,
};
use opensuite_protocol::{
    HeaderFooterVariant, InsertSectionBreak, PropertyPatch, SectionBreakType,
    SectionHeaderFooterChange, SectionTarget, SetOddEvenHeaders, SetSectionHeaderFooter,
    SetSectionProperties,
};

#[napi(object)]
pub struct InsertSectionBreakInput {
    /// @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."}
    pub placement: ParagraphPlacementInput,
    /// @schema {"enum":["nextPage","continuous","oddPage","evenPage"]}
    pub break_type: String,
}
#[napi(object)]
pub struct SetSectionPropertiesInput {
    pub handle: String,
    pub page_setup: Option<SetPageSetupInput>,
    pub different_first_page: Option<bool>,
    /// @schema {"enum":["nextPage","continuous","oddPage","evenPage"]}
    pub break_type: Option<String>,
    /// @schema {"minimum":0}
    pub page_number_start: Option<u32>,
    pub continue_page_numbering: Option<bool>,
}
#[napi(object)]
pub struct SetSectionHeaderFooterInput {
    pub handle: String,
    /// @schema {"enum":["header","footer"]}
    pub kind: String,
    /// @schema {"enum":["default","first","even"]}
    pub variant: String,
    /// @schema {"enum":["text","pageNumber","inherit","unlink"]}
    pub action: String,
    pub text: Option<String>,
    /// @schema {"enum":["left","center","right"]}
    pub alignment: Option<String>,
}
#[napi(object)]
pub struct SetOddEvenHeadersInput {
    pub enabled: bool,
}

/// Insert a real Word section boundary at a body placement. Re-inspect sections and body_blocks after edits.
#[napi(
    js_name = "executeDocxInsertSectionBreak",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn insert_section_break(
    input: Buffer,
    operation: InsertSectionBreakInput,
) -> AsyncTask<PageCompositionTask<InsertSectionBreak>> {
    PageCompositionTask::new(
        input,
        paragraph_placement(operation.placement).and_then(|placement| {
            Ok(InsertSectionBreak {
                placement,
                break_type: break_type(&operation.break_type)?,
            })
        }),
        execute_docx_insert_section_break,
    )
}
/// Edit one section using its latest inspected handle. Margins use twips (1440 = one inch). Restart numbering with pageNumberStart or continuePageNumbering=true.
#[napi(
    js_name = "executeDocxSetSectionProperties",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn set_section_properties(
    input: Buffer,
    operation: SetSectionPropertiesInput,
) -> AsyncTask<PageCompositionTask<SetSectionProperties>> {
    let parsed = (|| {
        if operation.page_number_start.is_some() && operation.continue_page_numbering == Some(true)
        {
            return Err("choose restart or continue page numbering");
        }
        Ok(SetSectionProperties {
            target: SectionTarget {
                handle: operation.handle,
            },
            page_setup: operation.page_setup.map(page_setup).transpose()?,
            different_first_page: operation.different_first_page,
            break_type: operation
                .break_type
                .as_deref()
                .map(break_type)
                .transpose()?,
            page_number_start: operation
                .page_number_start
                .map(PropertyPatch::Set)
                .or_else(|| {
                    (operation.continue_page_numbering == Some(true))
                        .then_some(PropertyPatch::Clear)
                }),
        })
    })();
    PageCompositionTask::new(input, parsed, execute_docx_set_section_properties)
}
/// Edit one simple section header/footer variant. text creates independent content; empty text clears it. inherit links to previous; unlink preserves visible content in an independent part. PAGE fields use pageNumber. Other sections retain their content. Enable odd/even explicitly before editing even variants; use differentFirstPage to activate first variants.
#[napi(
    js_name = "executeDocxSetSectionHeaderFooter",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn set_section_header_footer(
    input: Buffer,
    operation: SetSectionHeaderFooterInput,
) -> AsyncTask<PageCompositionTask<SetSectionHeaderFooter>> {
    let parsed = (|| {
        let variant = match operation.variant.as_str() {
            "default" => HeaderFooterVariant::Default,
            "first" => HeaderFooterVariant::First,
            "even" => HeaderFooterVariant::Even,
            _ => return Err("header/footer variant is not supported"),
        };
        let change = match operation.action.as_str() {
            "inherit" | "unlink" if operation.text.is_some() || operation.alignment.is_some() => {
                return Err("inherit/unlink cannot include text or alignment");
            }
            "inherit" => SectionHeaderFooterChange::Inherit,
            "unlink" => SectionHeaderFooterChange::Unlink,
            "text" if operation.alignment.is_some() => {
                return Err("text action cannot include alignment");
            }
            "text" => SectionHeaderFooterChange::SetText(
                operation
                    .text
                    .ok_or("text action requires text (use empty text to clear)")?,
            ),
            "pageNumber" if operation.text.is_some() => {
                return Err("pageNumber action cannot include text");
            }
            "pageNumber" => SectionHeaderFooterChange::SetPageNumber(page_number_alignment(
                operation.alignment,
            )?),
            _ => return Err("header/footer action is not supported"),
        };
        Ok(SetSectionHeaderFooter {
            target: SectionTarget {
                handle: operation.handle,
            },
            kind: header_footer_kind(&operation.kind)?,
            variant,
            change,
        })
    })();
    PageCompositionTask::new(input, parsed, execute_docx_set_section_header_footer)
}
/// Explicitly enable or disable different odd/even headers and footers for the ENTIRE document. This affects every section; it preserves all variant parts.
#[napi(
    js_name = "executeDocxSetOddEvenHeaders",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn set_odd_even_headers(
    input: Buffer,
    operation: SetOddEvenHeadersInput,
) -> AsyncTask<PageCompositionTask<SetOddEvenHeaders>> {
    PageCompositionTask::new(
        input,
        Ok(SetOddEvenHeaders {
            enabled: operation.enabled,
        }),
        execute_docx_set_odd_even_headers,
    )
}

fn break_type(value: &str) -> std::result::Result<SectionBreakType, &'static str> {
    match value {
        "nextPage" => Ok(SectionBreakType::NextPage),
        "continuous" => Ok(SectionBreakType::Continuous),
        "oddPage" => Ok(SectionBreakType::OddPage),
        "evenPage" => Ok(SectionBreakType::EvenPage),
        _ => Err("section break type is not supported"),
    }
}

#[napi(js_name = "inspectDocxSections", ts_return_type = "Promise<string>")]
pub fn inspect_docx_sections(input: Buffer) -> AsyncTask<InspectSectionsTask> {
    AsyncTask::new(InspectSectionsTask {
        input: input.to_vec(),
    })
}
pub struct InspectSectionsTask {
    input: Vec<u8>,
}
impl Task for InspectSectionsTask {
    type Output = String;
    type JsValue = String;
    fn compute(&mut self) -> Result<String> {
        let inspect = opensuite_docx::inspect_docx_sections(std::mem::take(&mut self.input));
        let value = match inspect {
            Ok(sections) => serde_json::json!({"ok":true,"sections":sections,"diagnostics":[]}),
            Err(error) => {
                serde_json::json!({"ok":false,"sections":[],"diagnostics":error.diagnostics.iter().map(|d|serde_json::json!({"code":d.code,"message":d.message,"severity":"error"})).collect::<Vec<_>>()})
            }
        };
        Ok(value.to_string())
    }
    fn resolve(&mut self, _env: Env, output: String) -> Result<String> {
        Ok(output)
    }
}
