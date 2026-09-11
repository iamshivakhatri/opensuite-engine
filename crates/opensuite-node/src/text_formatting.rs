use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi_derive::napi;
use opensuite_docx::execute_docx_set_text_formatting;
use opensuite_protocol::{
    PropertyPatch, SetTextFormatting, TextFormattingPatch, VerticalAlignment,
};

use crate::{SimpleTask, TextTargetInput, text_target};

#[napi(object)]
pub struct SetTextFormattingInput {
    pub target: TextTargetInput,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub font_size_half_points: Option<u32>,
    pub font_family: Option<String>,
    pub clear_bold: Option<bool>,
    pub color: Option<String>,
    pub clear_color: Option<bool>,
    pub underline: Option<bool>,
    pub clear_underline: Option<bool>,
    pub highlight: Option<String>,
    pub clear_highlight: Option<bool>,
    pub strikethrough: Option<bool>,
    pub clear_strikethrough: Option<bool>,
    pub vertical_alignment: Option<String>,
    pub clear_vertical_alignment: Option<bool>,
    pub base_revision: Option<String>,
}

#[napi(js_name = "executeDocxSetTextFormatting")]
pub fn execute_docx_set_text_formatting_node(
    input: Buffer,
    operation: SetTextFormattingInput,
) -> AsyncTask<SimpleTask<SetTextFormatting>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetTextFormatting {
            target: text_target(operation.target),
            formatting: TextFormattingPatch {
                bold: operation
                    .clear_bold
                    .filter(|value| *value)
                    .map(|_| PropertyPatch::Clear)
                    .or_else(|| operation.bold.map(PropertyPatch::Set)),
                italic: operation.italic.map(PropertyPatch::Set),
                font_size_half_points: operation
                    .font_size_half_points
                    .map(|value| PropertyPatch::Set(value as u16)),
                font_family: operation.font_family.map(PropertyPatch::Set),
                color: operation
                    .clear_color
                    .filter(|value| *value)
                    .map(|_| PropertyPatch::Clear)
                    .or_else(|| operation.color.map(PropertyPatch::Set)),
                underline: operation
                    .clear_underline
                    .filter(|value| *value)
                    .map(|_| PropertyPatch::Clear)
                    .or_else(|| operation.underline.map(PropertyPatch::Set)),
                highlight: operation
                    .clear_highlight
                    .filter(|value| *value)
                    .map(|_| PropertyPatch::Clear)
                    .or_else(|| operation.highlight.map(PropertyPatch::Set)),
                strikethrough: operation
                    .clear_strikethrough
                    .filter(|value| *value)
                    .map(|_| PropertyPatch::Clear)
                    .or_else(|| operation.strikethrough.map(PropertyPatch::Set)),
                vertical_alignment: operation
                    .clear_vertical_alignment
                    .filter(|value| *value)
                    .map(|_| PropertyPatch::Clear)
                    .or_else(|| vertical_alignment(operation.vertical_alignment)),
            },
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_text_formatting,
    })
}

fn vertical_alignment(value: Option<String>) -> Option<PropertyPatch<VerticalAlignment>> {
    match value.as_deref() {
        Some("baseline") => Some(PropertyPatch::Set(VerticalAlignment::Baseline)),
        Some("superscript") => Some(PropertyPatch::Set(VerticalAlignment::Superscript)),
        Some("subscript") => Some(PropertyPatch::Set(VerticalAlignment::Subscript)),
        _ => None,
    }
}
