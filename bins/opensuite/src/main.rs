fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let result = match (arguments.next(), arguments.next(), arguments.next()) {
        (Some(command), Some(path), None) if command == "inspect" => inspect(path),
        (Some(command), Some(path), None) if command == "inspect-source" => inspect_source(path),
        (Some(command), Some(path), None) if command == "inspect-docx" => inspect_docx(path),
        (Some(command), Some(path), None) if command == "inspect-styles" => inspect_styles(path),
        (Some(command), Some(path), None) if command == "inspect-paragraphs" => inspect_paragraphs(path),
        (Some(command), Some(path), None) if command == "inspect-numbering" => inspect_numbering(path),
        (Some(command), Some(path), None) if command == "inspect-sections" => inspect_sections(path),
        (Some(command), Some(path), None) if command == "inspect-headers-footers" => inspect_headers_footers(path),
        _ => Err((
            "INVALID_ARGUMENTS",
            "usage: opensuite <inspect|inspect-source|inspect-docx|inspect-styles|inspect-paragraphs|inspect-numbering|inspect-sections|inspect-headers-footers> <path-to-office-file>"
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

fn inspect_headers_footers(
    path: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let main = package
        .main_office_document()
        .map_err(|error| (error.code(), error.to_string()))?;
    let source = opensuite_docx::SourceDocument::parse(
        package
            .read_part(&main)
            .map_err(|error| (error.code(), error.to_string()))?,
    )
    .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let sections = document
        .sections()
        .map(|section| {
            Ok(serde_json::json!({
                "headers": section.header_references().map(|reference| inspect_header_footer(&package, &main, &reference)).collect::<Result<Vec<_>, _>>()?,
                "footers": section.footer_references().map(|reference| inspect_header_footer(&package, &main, &reference)).collect::<Result<Vec<_>, _>>()?,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({ "ok": true, "sections": sections }))
}

fn inspect_header_footer(
    package: &opensuite_opc::Package,
    main: &opensuite_opc::Part,
    reference: &opensuite_docx::HeaderFooterReference,
) -> Result<serde_json::Value, (&'static str, String)> {
    let content = match reference.kind() {
        opensuite_docx::HeaderFooterKind::Header => {
            opensuite_docx::load_header(package, main, reference)
        }
        opensuite_docx::HeaderFooterKind::Footer => {
            opensuite_docx::load_footer(package, main, reference)
        }
    }
    .map_err(|error| (error.code(), error.to_string()))?;
    let paragraphs = content
        .blocks()
        .filter_map(|block| match block {
            opensuite_docx::BodyBlock::Paragraph(paragraph) => Some(paragraph.text()),
            opensuite_docx::BodyBlock::Table(_) => None,
        })
        .map(|text| {
            text.map(|text| serde_json::json!({ "text": text }))
                .map_err(|error| (error.code(), error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        serde_json::json!({ "type": header_footer_type_name(reference.reference_type()), "part_name": content.part().name.as_str(), "paragraphs": paragraphs }),
    )
}

fn header_footer_type_name(value: &opensuite_docx::HeaderFooterType) -> String {
    match value {
        opensuite_docx::HeaderFooterType::Default => "default".to_owned(),
        opensuite_docx::HeaderFooterType::First => "first".to_owned(),
        opensuite_docx::HeaderFooterType::Even => "even".to_owned(),
        opensuite_docx::HeaderFooterType::Unknown(value) => value.clone(),
    }
}

fn inspect_sections(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (_, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let sections = document
        .sections()
        .map(|section| {
            let properties = section.properties();
            Ok(serde_json::json!({
                "type": properties.section_type().map(section_type_name),
                "page_size": properties.page_size().map_err(|error| (error.code(), error.to_string()))?.map(page_size_json),
                "margins": properties.page_margins().map_err(|error| (error.code(), error.to_string()))?.map(page_margins_json),
                "columns": properties.columns().map_err(|error| (error.code(), error.to_string()))?.map(columns_json),
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({ "ok": true, "section_count": sections.len(), "sections": sections }))
}

fn section_type_name(value: opensuite_docx::SectionType) -> String {
    match value {
        opensuite_docx::SectionType::NextPage => "nextPage".to_owned(),
        opensuite_docx::SectionType::Continuous => "continuous".to_owned(),
        opensuite_docx::SectionType::EvenPage => "evenPage".to_owned(),
        opensuite_docx::SectionType::OddPage => "oddPage".to_owned(),
        opensuite_docx::SectionType::Unknown(value) => value,
    }
}

fn page_size_json(value: opensuite_docx::PageSize) -> serde_json::Value {
    serde_json::json!({ "width_twips": value.width_twips, "height_twips": value.height_twips, "orientation": value.orientation.map(orientation_name) })
}
fn orientation_name(value: opensuite_docx::PageOrientation) -> String {
    match value {
        opensuite_docx::PageOrientation::Portrait => "portrait".to_owned(),
        opensuite_docx::PageOrientation::Landscape => "landscape".to_owned(),
        opensuite_docx::PageOrientation::Unknown(value) => value,
    }
}
fn page_margins_json(value: opensuite_docx::PageMargins) -> serde_json::Value {
    serde_json::json!({ "top_twips": value.top_twips, "bottom_twips": value.bottom_twips, "left_twips": value.left_twips, "right_twips": value.right_twips, "header_twips": value.header_twips, "footer_twips": value.footer_twips, "gutter_twips": value.gutter_twips })
}
fn columns_json(value: opensuite_docx::Columns) -> serde_json::Value {
    serde_json::json!({ "count": value.count, "spacing_twips": value.spacing_twips, "equal_width": value.equal_width })
}

fn inspect_numbering(
    path: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let main = package
        .main_office_document()
        .map_err(|error| (error.code(), error.to_string()))?;
    let Some(numbering) = opensuite_docx::load_numbering(&package, &main)
        .map_err(|error| (error.code(), error.to_string()))?
    else {
        return Ok(serde_json::json!({"ok":true,"abstract_count":0,"instance_count":0,"lists":[]}));
    };
    let mut instances: Vec<_> = numbering.instances().collect();
    instances.sort_by_key(|instance| instance.num_id.0);
    let lists = instances.into_iter().map(|instance| {
        let levels = (0_u8..=8).filter_map(|level| numbering.resolve(opensuite_docx::ListReference { num_id: instance.num_id, level }).ok()).map(|level| serde_json::json!({"level":level.level,"format":number_format_name(&level.format),"text":level.text,"start":level.start,"suffix":level.suffix})).collect::<Vec<_>>();
        serde_json::json!({"num_id":instance.num_id.0,"abstract_num_id":instance.abstract_num_id.0,"levels":levels})
    }).collect::<Vec<_>>();
    Ok(
        serde_json::json!({"ok":true,"abstract_count":numbering.abstract_count(),"instance_count":numbering.instance_count(),"lists":lists}),
    )
}

fn number_format_name(format: &opensuite_docx::NumberFormat) -> &str {
    match format {
        opensuite_docx::NumberFormat::Decimal => "decimal",
        opensuite_docx::NumberFormat::UpperRoman => "upperRoman",
        opensuite_docx::NumberFormat::LowerRoman => "lowerRoman",
        opensuite_docx::NumberFormat::UpperLetter => "upperLetter",
        opensuite_docx::NumberFormat::LowerLetter => "lowerLetter",
        opensuite_docx::NumberFormat::Bullet => "bullet",
        opensuite_docx::NumberFormat::None => "none",
        opensuite_docx::NumberFormat::Unknown(value) => value,
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
