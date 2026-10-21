use super::*;

pub(super) fn picture_fragment(
    source: &SourceDocument,
    container: NodeId,
    extent: (i64, i64),
    picture_id: u32,
    relationship_id: &str,
    alt_text: Option<&str>,
    alignment: Option<opensuite_protocol::ParagraphAlignment>,
) -> Result<Vec<u8>, OperationResult> {
    let (width_emu, height_emu) = extent;
    let prefix = word_prefix_for(
        source,
        container,
        if word(source, container, "tc") {
            "tc"
        } else if word(source, container, "hdr") {
            "hdr"
        } else if word(source, container, "ftr") {
            "ftr"
        } else {
            "body"
        },
    )?;
    let name = |local: &str| qualify(&prefix, local);
    let description = alt_text
        .filter(|value| !value.is_empty())
        .map(|value| format!(r#" descr="{}""#, escape(value)))
        .unwrap_or_default();
    let paragraph_properties = match alignment {
        None => String::new(),
        Some(
            value @ (opensuite_protocol::ParagraphAlignment::Left
            | opensuite_protocol::ParagraphAlignment::Center
            | opensuite_protocol::ParagraphAlignment::Right),
        ) => format!(
            r#"<{}><{} {}="{}"/></{}>"#,
            name("pPr"),
            name("jc"),
            name("val"),
            alignment_value(value),
            name("pPr")
        ),
        Some(_) => {
            return Err(OperationResult::failed(
                "INVALID_OPERATION",
                "picture alignment must be left, center, or right",
            ));
        }
    };
    let picture_name = format!("Picture {picture_id}");
    Ok(format!(
        r#"<{}>{paragraph_properties}<{}><{}><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{width_emu}" cy="{height_emu}"/><wp:docPr id="{picture_id}" name="{picture_name}"{description}/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="{picture_id}" name="{picture_name}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:embed="{relationship_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{width_emu}" cy="{height_emu}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></{}></{}></{}>"#,
        name("p"), name("r"), name("drawing"), name("drawing"), name("r"), name("p")
    ).into_bytes())
}

/// Fit to declared bounds only; remaining space on a rendered page is unknown.
pub(super) fn automatic_extent(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    insertion: usize,
    original: (i64, i64),
    cell: Option<&cell::CellInsertion>,
) -> Result<(i64, i64), OperationResult> {
    let invalid = || {
        OperationResult::failed(
            "INVALID_PAGE_GEOMETRY",
            "automatic image fit requires positive declared page, margin, and column dimensions",
        )
    };
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    let section = document
        .sections()
        .find(|section| {
            let node = source.node(section.source_id()).expect("section");
            if node.parent() == Some(document.body_id()) {
                insertion <= node.span().start
            } else {
                let paragraph = node
                    .parent()
                    .and_then(|id| source.node(id))
                    .and_then(|node| node.parent())
                    .and_then(|id| source.node(id));
                paragraph.is_some_and(|node| insertion < node.span().end)
            }
        })
        .ok_or_else(invalid)?;
    // Reject ambiguous property children instead of choosing one silently.
    for local in ["pgSz", "pgMar", "cols"] {
        sections::section_child(source, section.source_id(), local)?;
    }
    let properties = section.properties();
    let page = properties
        .page_size()
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    if matches!(page.orientation, Some(crate::PageOrientation::Unknown(_))) {
        return Err(invalid());
    }
    let margins = properties
        .page_margins()
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    let margin = |value: Option<i32>| value.filter(|v| *v >= 0).map(i64::from).ok_or_else(invalid);
    // A gutter may be placed horizontally or vertically by settings. Deduct it on
    // both axes for a safe bound without guessing which setting Word will use.
    let gutter = margin(Some(margins.gutter_twips.unwrap_or(0)))?;
    let mut width = i64::from(page.width_twips.ok_or_else(invalid)?)
        - margin(margins.left_twips)?
        - margin(margins.right_twips)?
        - gutter;
    let mut height = i64::from(page.height_twips.ok_or_else(invalid)?)
        - margin(margins.top_twips)?
        - margin(margins.bottom_twips)?
        - gutter;
    if width <= 0 || height <= 0 {
        return Err(invalid());
    }
    if let Some(columns) = properties.columns().map_err(|_| invalid())? {
        let id = sections::section_child(source, section.source_id(), "cols")?.expect("columns");
        let children = source
            .children(id)
            .filter(|id| {
                !matches!(
                    source.node(*id).map(|node| node.kind()),
                    Some(SourceNodeKind::Text)
                )
            })
            .collect::<Vec<_>>();
        if children.iter().any(|id| !word(source, *id, "col")) {
            return Err(invalid());
        }
        let count = i64::from(columns.count.unwrap_or(1));
        if count < 1 {
            return Err(invalid());
        }
        if columns.equal_width == Some(false) {
            if children.is_empty() {
                return Err(invalid());
            }
            let mut total = 0;
            let mut narrowest = width;
            for (index, id) in children.iter().enumerate() {
                let node = source.node(*id).expect("column");
                let col_width = node
                    .attribute("w")
                    .and_then(|v| v.parse::<u32>().ok())
                    .filter(|v| *v > 0)
                    .ok_or_else(invalid)?;
                total += i64::from(col_width);
                narrowest = narrowest.min(i64::from(col_width));
                if index + 1 < children.len() {
                    total += i64::from(
                        node.attribute("space")
                            .and_then(|v| v.parse::<u32>().ok())
                            .ok_or_else(invalid)?,
                    );
                }
            }
            if total > width {
                return Err(invalid());
            }
            // Column ownership needs pagination; use the narrowest declared column.
            width = narrowest;
        } else {
            if !children.is_empty() {
                return Err(invalid());
            }
            let spacing = if count == 1 {
                0
            } else {
                margin(columns.spacing_twips)?
            };
            width = (width - (count - 1) * spacing) / count;
        }
    }
    if let Some(cell) = cell {
        (width, height) = cell::fit_bounds(package, main, source, cell, (width, height))?;
    }
    fit_to_bounds(package, main, original, (width, height))
}

pub(super) fn fit_to_bounds(
    package: &Package,
    main: &Part,
    original: (i64, i64),
    bounds_twips: (i64, i64),
) -> Result<(i64, i64), OperationResult> {
    let (mut width, mut height) = bounds_twips;
    // New image paragraphs inherit document defaults, not the adjacent paragraph.
    // Deduct only reliably resolved indents; negative/hanging indents never widen the bound.
    if let Some(styles) = crate::load_styles(package, main).map_err(document_invalid)? {
        let formatting = styles
            .effective_paragraph_formatting(None, &Default::default())
            .map_err(document_invalid)?;
        if matches!(
            formatting.line_spacing.map(|value| value.rule),
            Some(Some(crate::styles::LineSpacingRule::Exact))
        ) {
            return Err(unsupported(
                "automatic image fit cannot use a paragraph with exact line height",
            ));
        }
        height -= i64::from(formatting.spacing_before_twips.unwrap_or(0).max(0))
            + i64::from(formatting.spacing_after_twips.unwrap_or(0).max(0));
        width -= i64::from(formatting.left_indent_twips.unwrap_or(0).max(0))
            + i64::from(formatting.right_indent_twips.unwrap_or(0).max(0))
            + i64::from(formatting.first_line_indent_twips.unwrap_or(0).max(0));
    }
    if width <= 0 || height <= 0 {
        return Err(OperationResult::failed(
            "INVALID_PAGE_GEOMETRY",
            "picture bounds leave no usable space",
        ));
    }
    let bounds = (width * 635, height * 635);
    // Choose the limiting axis once so rounding cannot progressively change ratio.
    let size = if original.0 <= bounds.0 && original.1 <= bounds.1 {
        None
    } else if i128::from(bounds.0) * i128::from(original.1)
        <= i128::from(bounds.1) * i128::from(original.0)
    {
        Some(opensuite_protocol::PictureSizeChange::WidthEmu(
            bounds.0.min(original.0),
        ))
    } else {
        Some(opensuite_protocol::PictureSizeChange::HeightEmu(
            bounds.1.min(original.1),
        ))
    };
    sized_extent(original, size)
}

#[cfg(test)]
mod tests;
