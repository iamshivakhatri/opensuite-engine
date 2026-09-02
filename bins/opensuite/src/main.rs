fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let result = match (arguments.next(), arguments.next(), arguments.next()) {
        (Some(command), Some(path), None) if command == "inspect" => inspect(path),
        (Some(command), Some(path), None) if command == "inspect-source" => inspect_source(path),
        (Some(command), Some(path), None) if command == "inspect-docx" => inspect_docx(path),
        (Some(command), Some(path), None) if command == "inspect-styles" => inspect_styles(path),
        (Some(command), Some(path), None) if command == "inspect-paragraphs" => inspect_paragraphs(path),
        _ => Err((
            "INVALID_ARGUMENTS",
            "usage: opensuite <inspect|inspect-source|inspect-docx|inspect-styles|inspect-paragraphs> <path-to-office-file>"
                .to_owned(),
        )),
    };

    match result {
        Ok(value) => println!("{value}"),
        Err((code, message)) => {
            println!(
                "{}",
                serde_json::json!({ "ok": false, "error": { "code": code, "message": message } })
            );
            std::process::exit(1);
        }
    }
}

fn inspect_paragraphs(
    path: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (part, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let styles = opensuite_docx::load_styles(&package, &part)
        .map_err(|error| (error.code(), error.to_string()))?;
    let paragraphs = document
        .paragraphs()
        .enumerate()
        .map(|(index, paragraph)| {
            let mut item = serde_json::Map::new();
            item.insert("index".to_owned(), serde_json::json!(index));
            item.insert(
                "text".to_owned(),
                serde_json::json!(
                    paragraph
                        .text()
                        .map_err(|error| (error.code(), error.to_string()))?
                ),
            );
            if let Some(style_id) = paragraph.style_id() {
                item.insert("style_id".to_owned(), serde_json::json!(style_id.as_str()));
            }
            if let Some(styles) = styles.as_ref() {
                item.insert(
                    "formatting".to_owned(),
                    paragraph_formatting_json(
                        paragraph
                            .effective_formatting(styles)
                            .map_err(|error| (error.code(), error.to_string()))?,
                    ),
                );
            }
            Ok(serde_json::Value::Object(item))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({ "ok": true, "paragraphs": paragraphs }))
}

fn paragraph_formatting_json(formatting: opensuite_docx::ParagraphFormatting) -> serde_json::Value {
    let mut fields = serde_json::Map::new();
    if let Some(alignment) = formatting.alignment {
        fields.insert(
            "alignment".to_owned(),
            serde_json::json!(match alignment {
                opensuite_docx::ParagraphAlignment::Left => "left",
                opensuite_docx::ParagraphAlignment::Center => "center",
                opensuite_docx::ParagraphAlignment::Right => "right",
                opensuite_docx::ParagraphAlignment::Both => "both",
                opensuite_docx::ParagraphAlignment::Distribute => "distribute",
            }),
        );
    }
    if let Some(value) = formatting.spacing_before_twips {
        fields.insert("spacing_before_twips".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.spacing_after_twips {
        fields.insert("spacing_after_twips".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.line_spacing {
        let rule = match value.rule {
            Some(opensuite_docx::LineSpacingRule::Auto) => "auto",
            Some(opensuite_docx::LineSpacingRule::Exact) => "exact",
            Some(opensuite_docx::LineSpacingRule::AtLeast) => "atLeast",
            None => "unspecified",
        };
        fields.insert(
            "line_spacing".to_owned(),
            serde_json::json!({ "value": value.value, "rule": rule }),
        );
    }
    if let Some(value) = formatting.left_indent_twips {
        fields.insert("left_indent_twips".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.right_indent_twips {
        fields.insert("right_indent_twips".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.first_line_indent_twips {
        fields.insert(
            "first_line_indent_twips".to_owned(),
            serde_json::json!(value),
        );
    }
    if let Some(value) = formatting.hanging_indent_twips {
        fields.insert("hanging_indent_twips".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.keep_with_next {
        fields.insert("keep_with_next".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.keep_lines {
        fields.insert("keep_lines".to_owned(), serde_json::json!(value));
    }
    serde_json::Value::Object(fields)
}

fn inspect_styles(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let main = package
        .main_office_document()
        .map_err(|error| (error.code(), error.to_string()))?;
    let Some(styles) = opensuite_docx::load_styles(&package, &main)
        .map_err(|error| (error.code(), error.to_string()))?
    else {
        return Ok(serde_json::json!({ "ok": true, "style_count": 0, "styles": [] }));
    };
    let mut styles: Vec<_> = styles.styles().collect();
    styles.sort_by_key(|style| style.id().as_str());
    let styles = styles
        .into_iter()
        .map(|style| serde_json::json!({
            "id": style.id().as_str(),
            "type": match style.style_type() { opensuite_docx::StyleType::Paragraph => "paragraph", opensuite_docx::StyleType::Character => "character" },
            "based_on": style.based_on().map(opensuite_docx::StyleId::as_str),
        }))
        .collect::<Vec<_>>();
    Ok(serde_json::json!({ "ok": true, "style_count": styles.len(), "styles": styles }))
}

fn inspect_docx(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (part, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let blocks = document
        .blocks()
        .map(|block| inspect_block(block))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(serde_json::json!({
        "ok": true,
        "part_name": part.name.as_str(),
        "block_count": blocks.len(),
        "blocks": blocks,
    }))
}

fn inspect_block(
    block: opensuite_docx::BodyBlock<'_>,
) -> Result<serde_json::Value, (&'static str, String)> {
    match block {
        opensuite_docx::BodyBlock::Paragraph(paragraph) => Ok(serde_json::json!({
            "type": "paragraph",
            "text": paragraph.text().map_err(|error| (error.code(), error.to_string()))?,
            "run_count": paragraph.runs().count(),
        })),
        opensuite_docx::BodyBlock::Table(table) => {
            let rows = table
                .rows()
                .map(|row| {
                    let cells = row
                        .cells()
                        .map(|cell| cell.text().map(|text| serde_json::json!({ "text": text })))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|error| (error.code(), error.to_string()))?;
                    Ok(serde_json::json!({ "cells": cells }))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(serde_json::json!({ "type": "table", "row_count": rows.len(), "rows": rows }))
        }
    }
}

fn inspect_source(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (part, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let root = source.node(source.root()).ok_or((
        "SOURCE_SPAN_INCONSISTENCY",
        "source root is missing".to_owned(),
    ))?;
    let opensuite_docx::SourceNodeKind::Element { name, .. } = root.kind() else {
        return Err((
            "SOURCE_SPAN_INCONSISTENCY",
            "source root is not an element".to_owned(),
        ));
    };

    Ok(serde_json::json!({
        "ok": true,
        "part_name": part.name.as_str(),
        "source_bytes": source.original_bytes().len(),
        "node_count": source.node_count(),
        "element_count": source.element_count(),
        "text_count": source.text_count(),
        "root": {
            "local_name": name.local_name(),
            "namespace_uri": name.namespace_uri(),
        },
    }))
}

fn inspect(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let main_document = package
        .main_office_document()
        .map_err(|error| (error.code(), error.to_string()))?;

    Ok(serde_json::json!({
        "ok": true,
        "entry_count": package.entry_count(),
        "part_count": package.part_count(),
        "main_document": {
            "part_name": main_document.name.as_str(),
            "content_type": main_document.content_type.as_str(),
        },
    }))
}
