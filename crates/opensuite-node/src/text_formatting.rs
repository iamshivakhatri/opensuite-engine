use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi_derive::napi;
use opensuite_docx::execute_docx_set_text_formatting;
use opensuite_protocol::{SetTextFormatting, TextFormattingPatch, VerticalAlignment};

use crate::{SimpleTask, TextTargetInput, text_target};

#[napi(object)]
pub struct SetTextFormattingInput {
    pub target: TextTargetInput,
    /// Existing character style ID from style inspection. Clear with clear: ["characterStyleId"].
    pub character_style_id: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    /// @schema {"minimum":1,"maximum":65535}
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
    /// @schema {"enum":["baseline","superscript","subscript"]}
    pub vertical_alignment: Option<String>,
    pub clear_vertical_alignment: Option<bool>,
    /// @schema {"description":"Remove direct properties and restore inheritance. Omitted fields stay unchanged."}
    pub clear: Option<Vec<String>>,
    pub base_revision: Option<String>,
}

/// Format exact body text or partial text spans, including supported simple table-cell runs. For known whole table cells, use set_table_cells_formatting. Apply an existing character style with characterStyleId; clear: ["characterStyleId"] removes only that reference and restores inheritance. Other run properties remain unchanged. Occurrence is zero-based when ambiguous.
#[napi(
    js_name = "executeDocxSetTextFormatting",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
pub fn execute_docx_set_text_formatting_node(
    input: Buffer,
    operation: SetTextFormattingInput,
) -> napi::Result<AsyncTask<SimpleTask<SetTextFormatting>>> {
    let mut clear = operation.clear.unwrap_or_default();
    for key in &clear {
        if !TEXT_PROPERTIES.contains(&key.as_str()) {
            return Err(napi::Error::from_reason(format!(
                "unsupported text property to clear: {key}"
            )));
        }
    }
    for (flag, name) in [
        (operation.clear_bold, "bold"),
        (operation.clear_color, "color"),
        (operation.clear_underline, "underline"),
        (operation.clear_highlight, "highlight"),
        (operation.clear_strikethrough, "strikethrough"),
        (operation.clear_vertical_alignment, "verticalAlignment"),
    ] {
        if flag == Some(true) {
            clear.push(name.into());
        }
    }
    let size = operation
        .font_size_half_points
        .map(u16::try_from)
        .transpose()
        .map_err(|_| napi::Error::from_reason("fontSizeHalfPoints exceeds 65535"))?;
    let vertical = vertical_alignment(operation.vertical_alignment)?;
    use crate::styles::property;
    Ok(AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: SetTextFormatting {
            target: text_target(operation.target),
            formatting: TextFormattingPatch {
                character_style_id: property(
                    operation.character_style_id,
                    &clear,
                    "characterStyleId",
                ),
                bold: property(operation.bold, &clear, "bold"),
                italic: property(operation.italic, &clear, "italic"),
                font_size_half_points: property(size, &clear, "fontSizeHalfPoints"),
                font_family: property(operation.font_family, &clear, "fontFamily"),
                color: property(operation.color, &clear, "color"),
                underline: property(operation.underline, &clear, "underline"),
                highlight: property(operation.highlight, &clear, "highlight"),
                strikethrough: property(operation.strikethrough, &clear, "strikethrough"),
                vertical_alignment: property(vertical, &clear, "verticalAlignment"),
            },
            base_revision: operation.base_revision,
        },
        run: execute_docx_set_text_formatting,
    }))
}

pub(crate) const TEXT_PROPERTIES: &[&str] = &[
    "characterStyleId",
    "bold",
    "italic",
    "fontSizeHalfPoints",
    "fontFamily",
    "color",
    "underline",
    "highlight",
    "strikethrough",
    "verticalAlignment",
];

pub(crate) fn vertical_alignment(value: Option<String>) -> napi::Result<Option<VerticalAlignment>> {
    value
        .map(|value| match value.as_str() {
            "baseline" => Ok(VerticalAlignment::Baseline),
            "superscript" => Ok(VerticalAlignment::Superscript),
            "subscript" => Ok(VerticalAlignment::Subscript),
            _ => Err(napi::Error::from_reason("invalid vertical alignment")),
        })
        .transpose()
}
