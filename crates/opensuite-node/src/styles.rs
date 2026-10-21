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
    /// @schema {"description":"Stable Word style ID, for example OpenSuiteReportHeading"}
    pub style_id: String,
    /// @schema {"enum":["paragraph","character"]}
    pub style_type: String,
    /// @schema {"description":"Unique display name; apply paragraph styles using this name"}
    pub name: Option<String>,
    /// @schema {"description":"Existing parent style ID, for example Heading1"}
    pub based_on: Option<String>,
    /// @schema {"description":"Existing paragraph style ID"}
    pub next: Option<String>,
    /// @schema {"description":"Remove declarations and restore inheritance. Omitted fields stay unchanged. Twips: 20 = 1 pt."}
    pub clear: Option<Vec<String>>,
    pub base_revision: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    /// @schema {"minimum":1,"maximum":65535,"description":"32 = 16 pt"}
    pub font_size_half_points: Option<u32>,
    pub font_family: Option<String>,
    /// @schema {"pattern":"^([0-9a-fA-F]{6}|auto)$","description":"RGB without #, for example 124733"}
    pub color: Option<String>,
    pub underline: Option<bool>,
    pub highlight: Option<String>,
    pub strikethrough: Option<bool>,
    /// @schema {"enum":["baseline","superscript","subscript"]}
    pub vertical_alignment: Option<String>,
    /// @schema {"enum":["left","center","right","both","distribute"]}
    pub alignment: Option<String>,
    pub line_spacing: Option<LineSpacingInput>,
    pub spacing_before_twips: Option<i32>,
    pub spacing_after_twips: Option<i32>,
    pub left_indent_twips: Option<i32>,
    pub right_indent_twips: Option<i32>,
    pub first_line_indent_twips: Option<i32>,
    pub hanging_indent_twips: Option<i32>,
    /// @schema {"description":"Keep this paragraph with the following paragraph."}
    pub keep_with_next: Option<bool>,
    /// @schema {"description":"Keep all lines of this paragraph on one page."}
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
    /// @schema {"minimum":1,"description":"Auto: 240 per line, so 276 = 1.15. Exact/atLeast: twips, 20 = 1 pt."}
    pub value: u32,
    /// @schema {"enum":["auto","exact","atLeast"]}
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
        if !(["basedOn", "next"].contains(&key.as_str())
            || PARAGRAPH_PROPERTIES.contains(&key.as_str())
            || key != "characterStyleId"
                && crate::text_formatting::TEXT_PROPERTIES.contains(&key.as_str()))
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
            character_style_id: None,
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

/// Create a reusable real Word paragraph or character style. No default-style authoring. basedOn and next use IDs; apply paragraph styles with set_paragraph_style using the display name.
/// @schema {"required":["styleId","styleType","name"]}
#[napi(
    js_name = "executeDocxCreateStyle",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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

/// Patch supported properties of one Word style globally. Unspecified properties stay unchanged; clear removes a declaration and restores inheritance. Style type and default flag cannot change.
#[napi(
    js_name = "executeDocxUpdateStyle",
    ts_return_type = "Promise<ExecuteDocxReplaceTextOutput>"
)]
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
