use crate::SimpleTask;
use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi_derive::napi;
use opensuite_docx::{execute_docx_create_style, execute_docx_update_style};
use opensuite_protocol::{
    CreateStyle, LineSpacing, LineSpacingRule, ParagraphAlignment, ParagraphFormattingPatch,
    PropertyPatch, TextFormattingPatch, UpdateStyle, WordStylePatch, WordStyleType,
};

/// Omitted fields are unchanged; clear names remove declarations and restore inheritance.
#[napi(object)]
pub struct StyleInput {
    pub style_id: String,
    pub style_type: String,
    pub name: Option<String>,
    pub based_on: Option<String>,
    pub next: Option<String>,
    pub clear: Option<Vec<String>>,
    pub base_revision: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub font_size_half_points: Option<u32>,
    pub font_family: Option<String>,
    pub color: Option<String>,
    pub underline: Option<bool>,
    pub highlight: Option<String>,
    pub strikethrough: Option<bool>,
    pub vertical_alignment: Option<String>,
    pub alignment: Option<String>,
    pub line_spacing: Option<LineSpacingInput>,
    pub spacing_before_twips: Option<i32>,
    pub spacing_after_twips: Option<i32>,
    pub left_indent_twips: Option<i32>,
    pub right_indent_twips: Option<i32>,
    pub first_line_indent_twips: Option<i32>,
    pub hanging_indent_twips: Option<i32>,
    pub keep_with_next: Option<bool>,
    pub keep_lines: Option<bool>,
}

pub(crate) fn property<T>(
    value: Option<T>,
    clear: &[String],
    name: &str,
) -> Option<PropertyPatch<T>> {
    if clear.iter().any(|key| key == name) {
        Some(PropertyPatch::Clear)
    } else {
        value.map(PropertyPatch::Set)
    }
}

#[napi(object)]
pub struct LineSpacingInput {
    /// Auto uses 240 units per line; exact/atLeast use twips (20 per point).
    pub value: u32,
    pub rule: Option<String>,
}

pub(crate) const PARAGRAPH_PROPERTIES: &[&str] = &[
    "alignment",
    "spacingBeforeTwips",
    "spacingAfterTwips",
    "lineSpacing",
    "leftIndentTwips",
    "rightIndentTwips",
    "firstLineIndentTwips",
    "hangingIndentTwips",
    "keepWithNext",
    "keepLines",
];

pub(crate) fn validate_paragraph_clear(clear: &[String]) -> napi::Result<()> {
    for key in clear {
        if !PARAGRAPH_PROPERTIES.contains(&key.as_str()) {
            return Err(napi::Error::from_reason(format!(
                "unsupported paragraph property to clear: {key}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn alignment(value: Option<String>) -> napi::Result<Option<ParagraphAlignment>> {
    value
        .map(|value| match value.as_str() {
            "left" => Ok(ParagraphAlignment::Left),
            "center" => Ok(ParagraphAlignment::Center),
            "right" => Ok(ParagraphAlignment::Right),
            "both" => Ok(ParagraphAlignment::Both),
            "distribute" => Ok(ParagraphAlignment::Distribute),
            _ => Err(napi::Error::from_reason("invalid paragraph alignment")),
        })
        .transpose()
}

pub(crate) fn line_spacing(value: Option<LineSpacingInput>) -> napi::Result<Option<LineSpacing>> {
    value
        .map(|value| {
            Ok(LineSpacing {
                value: value.value,
                rule: value
                    .rule
                    .map(|rule| match rule.as_str() {
                        "auto" => Ok(LineSpacingRule::Auto),
                        "exact" => Ok(LineSpacingRule::Exact),
                        "atLeast" => Ok(LineSpacingRule::AtLeast),
                        _ => Err(napi::Error::from_reason("invalid line spacing rule")),
                    })
                    .transpose()?,
            })
        })
        .transpose()
}

fn properties(
    input: StyleInput,
) -> napi::Result<(String, WordStyleType, WordStylePatch, Option<String>)> {
    let kind = match input.style_type.as_str() {
        "paragraph" => WordStyleType::Paragraph,
        "character" => WordStyleType::Character,
        _ => {
            return Err(napi::Error::from_reason(
                "styleType must be paragraph or character",
            ));
        }
    };
    let clear = input.clear.unwrap_or_default();
    for key in &clear {
        if !["basedOn", "next"].contains(&key.as_str())
            && !crate::text_formatting::TEXT_PROPERTIES.contains(&key.as_str())
            && !PARAGRAPH_PROPERTIES.contains(&key.as_str())
        {
            return Err(napi::Error::from_reason(format!(
                "unsupported style property to clear: {key}"
            )));
        }
    }
    let size = input
        .font_size_half_points
        .map(u16::try_from)
        .transpose()
        .map_err(|_| napi::Error::from_reason("fontSizeHalfPoints exceeds 65535"))?;
    let alignment = alignment(input.alignment)?;
    let line_spacing = line_spacing(input.line_spacing)?;
    let vertical = crate::text_formatting::vertical_alignment(input.vertical_alignment)?;
    let patch = WordStylePatch {
        name: input.name,
        based_on: property(input.based_on, &clear, "basedOn"),
        next: property(input.next, &clear, "next"),
        run: TextFormattingPatch {
            bold: property(input.bold, &clear, "bold"),
            italic: property(input.italic, &clear, "italic"),
            font_family: property(input.font_family, &clear, "fontFamily"),
            color: property(input.color, &clear, "color"),
            underline: property(input.underline, &clear, "underline"),
            font_size_half_points: property(size, &clear, "fontSizeHalfPoints"),
            highlight: property(input.highlight, &clear, "highlight"),
            strikethrough: property(input.strikethrough, &clear, "strikethrough"),
            vertical_alignment: property(vertical, &clear, "verticalAlignment"),
        },
        paragraph: ParagraphFormattingPatch {
            alignment: property(alignment, &clear, "alignment"),
            line_spacing: property(line_spacing, &clear, "lineSpacing"),
            spacing_before_twips: property(
                input.spacing_before_twips,
                &clear,
                "spacingBeforeTwips",
            ),
            spacing_after_twips: property(input.spacing_after_twips, &clear, "spacingAfterTwips"),
            left_indent_twips: property(input.left_indent_twips, &clear, "leftIndentTwips"),
            right_indent_twips: property(input.right_indent_twips, &clear, "rightIndentTwips"),
            first_line_indent_twips: property(
                input.first_line_indent_twips,
                &clear,
                "firstLineIndentTwips",
            ),
            hanging_indent_twips: property(
                input.hanging_indent_twips,
                &clear,
                "hangingIndentTwips",
            ),
            keep_with_next: property(input.keep_with_next, &clear, "keepWithNext"),
            keep_lines: property(input.keep_lines, &clear, "keepLines"),
        },
    };
    Ok((input.style_id, kind, patch, input.base_revision))
}

#[napi(js_name = "executeDocxCreateStyle")]
pub fn create_style(
    input: Buffer,
    operation: StyleInput,
) -> napi::Result<AsyncTask<SimpleTask<CreateStyle>>> {
    let (style_id, style_type, properties, base_revision) = properties(operation)?;
    Ok(AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: CreateStyle {
            style_id,
            style_type,
            properties,
            base_revision,
        },
        run: execute_docx_create_style,
    }))
}

#[napi(js_name = "executeDocxUpdateStyle")]
pub fn update_style(
    input: Buffer,
    operation: StyleInput,
) -> napi::Result<AsyncTask<SimpleTask<UpdateStyle>>> {
    let (style_id, style_type, properties, base_revision) = properties(operation)?;
    Ok(AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: UpdateStyle {
            style_id,
            style_type,
            properties,
            base_revision,
        },
        run: execute_docx_update_style,
    }))
}
