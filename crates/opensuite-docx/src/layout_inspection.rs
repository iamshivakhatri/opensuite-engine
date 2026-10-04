//! Structural inputs to layout. This module never predicts rendered pages.
use crate::style_inspection::{
    ParagraphFormattingSnapshot, SectionSnapshot, StyleInspectionDiagnostic, inspect_table,
    section_snapshot,
};
use crate::{BodyBlock, DocxDocument, NodeId, SourceDocument, SourceNodeKind};
use opensuite_opc::{Package, Part};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

const MAX_ITEMS: usize = 64;
const MAX_DETAILS: usize = 100;

#[derive(Clone, Debug, Default)]
pub struct LayoutOptions {
    pub block_offset: usize,
    pub block_limit: usize,
    pub section_index: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSnapshot {
    pub ok: bool,
    pub schema_version: u32,
    pub kind: &'static str,
    pub section_count: usize,
    pub block_count: usize,
    pub paragraph_count: usize,
    pub table_count: usize,
    pub image_count: usize,
    pub explicit_page_break_count: usize,
    pub section_break_count: usize,
    pub sections: Vec<LayoutSection>,
    pub paragraph_patterns: Vec<ParagraphLayoutPattern>,
    pub tables: Vec<TableLayout>,
    pub images: Vec<ImageLayout>,
    pub blocks: Vec<BlockLayout>,
    pub matching_block_count: usize,
    pub block_offset: usize,
    pub has_more_blocks: bool,
    pub truncated: bool,
    pub rendered_page_count: Option<u32>,
    pub rendered_layout_status: &'static str,
    pub diagnostics: Vec<StyleInspectionDiagnostic>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSection {
    #[serde(flatten)]
    pub geometry: SectionSnapshot,
    pub handle: String,
    pub usable_width_twips: Option<i64>,
    pub usable_height_twips: Option<i64>,
    pub column_widths_twips: Vec<Option<u32>>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParagraphLayoutPattern {
    pub formatting: ParagraphFormattingSnapshot,
    pub count: usize,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockLayout {
    pub handle: String,
    pub kind: &'static str,
    pub section_index: Option<usize>,
    pub paragraph_formatting: Option<ParagraphFormattingSnapshot>,
    pub explicit_page_break_count: usize,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowLayout {
    pub index: usize,
    pub height_twips: Option<u32>,
    pub height_rule: Option<String>,
    pub cannot_split: Option<bool>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableLayout {
    pub handle: String,
    pub section_index: Option<usize>,
    pub preferred_width: Option<i32>,
    pub preferred_width_type: Option<String>,
    pub column_widths_twips: Vec<i32>,
    pub row_count: usize,
    pub cell_margins_twips: BTreeMap<String, i32>,
    pub cannot_split_row_count: usize,
    pub rows: Vec<RowLayout>,
    pub truncated: bool,
    pub has_complex_structure: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageLayout {
    pub kind: &'static str,
    pub block_handle: Option<String>,
    pub section_index: Option<usize>,
    pub relationship_id: Option<String>,
    pub asset_part_name: Option<String>,
    pub display_width_emu: Option<i64>,
    pub display_height_emu: Option<i64>,
    pub intrinsic_width_pixels: Option<u32>,
    pub intrinsic_height_pixels: Option<u32>,
}

pub fn inspect_docx_layout(input: Vec<u8>, options: &LayoutOptions) -> LayoutSnapshot {
    let mut snapshot = LayoutSnapshot {
        ok: false,
        schema_version: 1,
        kind: "structural",
        section_count: 0,
        block_count: 0,
        paragraph_count: 0,
        table_count: 0,
        image_count: 0,
        explicit_page_break_count: 0,
        section_break_count: 0,
        sections: vec![],
        paragraph_patterns: vec![],
        tables: vec![],
        images: vec![],
        blocks: vec![],
        matching_block_count: 0,
        block_offset: options.block_offset,
        has_more_blocks: false,
        truncated: false,
        rendered_page_count: None,
        rendered_layout_status: "unavailable",
        diagnostics: vec![],
    };
    if options.block_limit > MAX_DETAILS {
        snapshot.diagnostic(
            "INVALID_INSPECTION_BOUNDS",
            "blockLimit must be between 0 and 100",
        );
        return snapshot;
    }
    match Package::from_bytes(input)
        .map_err(|e| (e.code(), e.to_string()))
        .and_then(|package| {
            let (main, source) =
                crate::open_main_source(&package).map_err(|e| (e.code(), e.to_string()))?;
            inspect(&package, &main, &source, options, &mut snapshot)
                .map_err(|e| (e.code(), e.to_string()))
        }) {
        Ok(()) => {}
        Err((code, message)) => snapshot.diagnostic(code, &message),
    }
    snapshot.diagnostic("RENDERED_LAYOUT_UNAVAILABLE", "Structural inspection contains no rendered pages. Use an external renderer for page count; exact block-to-page mapping is unavailable.");
    snapshot
}

fn inspect(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    options: &LayoutOptions,
    out: &mut LayoutSnapshot,
) -> Result<(), crate::SemanticError> {
    let document = DocxDocument::new(source)?;
    let styles = match crate::load_styles(package, main) {
        Ok(styles) => styles,
        Err(error) => {
            out.diagnostic(error.code(), "style formatting could not be resolved");
            None
        }
    };
    let sections = document.sections().collect::<Vec<_>>();
    out.section_count = sections.len();
    if sections.is_empty() {
        out.diagnostic(
            "MISSING_SECTION_GEOMETRY",
            "no declared section geometry; implicit Word defaults are not inferred",
        );
    }
    out.section_break_count = sections
        .iter()
        .filter(|section| {
            source
                .node(section.source_id())
                .and_then(|node| node.parent())
                .is_some_and(|parent| word(source, parent, "pPr"))
        })
        .count();
    if options
        .section_index
        .is_some_and(|index| index >= sections.len())
    {
        out.diagnostic("TARGET_NOT_FOUND", "sectionIndex does not name a section");
        return Ok(());
    }
    let fingerprint = crate::mutation::section_fingerprint(package, main, source).ok();
    let mut geometry = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        match section_snapshot(section, source, index) {
            Ok(value) => {
                let width = usable(
                    value.page_width_twips,
                    &value.margins_twips,
                    "left",
                    "right",
                );
                let height = usable(
                    value.page_height_twips,
                    &value.margins_twips,
                    "top",
                    "bottom",
                );
                // A nonzero gutter may be applied on either axis by settings; avoid guessing.
                let (width, height) =
                    if value.margins_twips.get("gutter").copied().unwrap_or(0) == 0 {
                        (width, height)
                    } else {
                        out.diagnostic(
                            "UNRESOLVED_GUTTER_GEOMETRY",
                            "nonzero gutter placement requires additional layout settings",
                        );
                        (None, None)
                    };
                if width.is_some_and(|v| v <= 0) || height.is_some_and(|v| v <= 0) {
                    out.diagnostic(
                        "INVALID_PAGE_GEOMETRY",
                        "section margins leave no usable page area",
                    );
                }
                geometry.push((width, height));
                if out.sections.len() < MAX_ITEMS
                    && options.section_index.is_none_or(|v| v == index)
                {
                    let columns = child(source, section.source_id(), "cols");
                    if columns.is_some_and(|id| {
                        source
                            .children(id)
                            .filter(|id| word(source, *id, "col"))
                            .count()
                            > MAX_ITEMS
                    }) {
                        out.truncated = true;
                    }
                    let widths = columns
                        .map(|id| {
                            source
                                .children(id)
                                .filter(|id| word(source, *id, "col"))
                                .map(|id| number(source, id, "w"))
                                .take(MAX_ITEMS)
                                .collect()
                        })
                        .unwrap_or_default();
                    out.sections.push(LayoutSection {
                        geometry: value,
                        handle: fingerprint
                            .map(|v| format!("s{index}:{v:016x}"))
                            .unwrap_or_else(|| format!("section:{index}")),
                        usable_width_twips: width,
                        usable_height_twips: height,
                        column_widths_twips: widths,
                    });
                } else if options.section_index.is_none_or(|v| v == index) {
                    out.truncated = true;
                }
            }
            Err(error) => {
                out.diagnostic(error.code(), "invalid section geometry");
                geometry.push((None, None));
            }
        }
    }
    let children = source.children(document.body_id()).collect::<Vec<_>>();
    let ends = sections
        .iter()
        .map(|s| {
            children
                .get(s.end_body_child_index())
                .and_then(|id| source.node(*id))
                .map(|n| n.span().start)
                .unwrap_or(source.original_bytes().len())
        })
        .collect::<Vec<_>>();
    let mut section_index = 0;
    let mut owners = HashMap::new();
    for (index, block) in document.blocks().enumerate() {
        let (node, kind) = match &block {
            BodyBlock::Paragraph(p) => (p.source_id(), "paragraph"),
            BodyBlock::Table(t) => (t.source_id(), "table"),
        };
        while section_index < ends.len()
            && source.node(node).expect("block").span().start >= ends[section_index]
        {
            section_index += 1;
        }
        let section = (section_index < sections.len()).then_some(section_index);
        let handle = format!("b{index}");
        owners.insert(node, (handle.clone(), section));
        let selected = options.section_index.is_none_or(|v| Some(v) == section);
        out.block_count += 1;
        let breaks = descendants(source, node)
            .filter(|id| {
                word(source, *id, "br")
                    && source.node(*id).and_then(|n| n.attribute("type")) == Some("page")
            })
            .count();
        out.explicit_page_break_count += breaks;
        let formatting = match block {
            BodyBlock::Paragraph(paragraph) => {
                out.paragraph_count += 1;
                let result = if let Some(styles) = &styles {
                    paragraph.effective_formatting(styles)
                } else {
                    paragraph.direct_formatting()
                };
                let formatting = result
                    .map(|value| ParagraphFormattingSnapshot::from(&value))
                    .map_err(|error| {
                        out.diagnostic(
                            error.code(),
                            "paragraph layout controls could not be resolved",
                        )
                    })
                    .ok();
                if selected {
                    if let Some(value) = &formatting {
                        if let Some(pattern) = out
                            .paragraph_patterns
                            .iter_mut()
                            .find(|p| &p.formatting == value)
                        {
                            pattern.count += 1;
                        } else if out.paragraph_patterns.len() < MAX_ITEMS {
                            out.paragraph_patterns.push(ParagraphLayoutPattern {
                                formatting: value.clone(),
                                count: 1,
                            });
                        } else {
                            out.truncated = true;
                        }
                    }
                }
                formatting
            }
            BodyBlock::Table(table) => {
                out.table_count += 1;
                inspect_table_layout(
                    source,
                    &table,
                    &handle,
                    section,
                    section.and_then(|i| geometry[i].0),
                    selected,
                    out,
                );
                None
            }
        };
        if selected {
            let position = out.matching_block_count;
            out.matching_block_count += 1;
            if position >= options.block_offset && out.blocks.len() < options.block_limit {
                out.blocks.push(BlockLayout {
                    handle,
                    kind,
                    section_index: section,
                    paragraph_formatting: formatting,
                    explicit_page_break_count: breaks,
                });
            }
        }
    }
    inspect_images(package, main, source, &owners, &geometry, options, out);
    out.ok = true;
    out.has_more_blocks = out
        .matching_block_count
        .saturating_sub(options.block_offset)
        > out.blocks.len();
    Ok(())
}
fn inspect_table_layout(
    source: &SourceDocument,
    table: &crate::Table<'_>,
    handle: &str,
    section: Option<usize>,
    width: Option<i64>,
    selected: bool,
    out: &mut LayoutSnapshot,
) {
    // Reuse the existing source-aware table inspection once per table.
    let facts = inspect_table(source, table.source_id(), (out.table_count - 1) as u32);
    if let Some(width) = width {
        if (facts.width_type.as_deref() == Some("dxa")
            && facts.width_twips.is_some_and(|v| i64::from(v) > width))
            || facts
                .column_widths_twips
                .iter()
                .map(|v| i64::from(*v))
                .sum::<i64>()
                > width
            || (facts.width_type.as_deref() == Some("pct")
                && facts.width_twips.is_some_and(|v| v > 5000))
        {
            out.diagnostic("TABLE_WIDTH_EXCEEDS_PAGE", &format!("{handle}: preferred table or grid width exceeds usable page width; actual width requires rendering"));
        }
    }
    if facts.has_complex_structure {
        out.diagnostic("COMPLEX_TABLE_LAYOUT", &format!("{handle}: nested, merged, or irregular table; dimensions are structural inputs only"));
    }
    if selected && out.tables.len() < MAX_ITEMS {
        let mut rows = vec![];
        let mut cannot_split = 0;
        for (row_index, row) in table.rows().enumerate() {
            let properties = child(source, row.source_id(), "trPr");
            let height = properties.and_then(|id| child(source, id, "trHeight"));
            let split = properties
                .and_then(|id| child(source, id, "cantSplit"))
                .and_then(|id| on_off(source, id, out));
            cannot_split += usize::from(split == Some(true));
            if rows.len() < MAX_ITEMS {
                rows.push(RowLayout {
                    index: row_index,
                    height_twips: height.and_then(|id| number(source, id, "val")),
                    height_rule: height.and_then(|id| attribute(source, id, "hRule")),
                    cannot_split: split,
                });
            }
        }
        let truncated =
            facts.column_widths_twips.len() > MAX_ITEMS || facts.row_count as usize > MAX_ITEMS;
        out.truncated |= truncated;
        out.tables.push(TableLayout {
            handle: handle.to_owned(),
            section_index: section,
            preferred_width: facts.width_twips,
            preferred_width_type: facts.width_type,
            column_widths_twips: facts
                .column_widths_twips
                .into_iter()
                .take(MAX_ITEMS)
                .collect(),
            row_count: facts.row_count as usize,
            cell_margins_twips: facts.cell_margins_twips,
            cannot_split_row_count: cannot_split,
            rows,
            truncated,
            has_complex_structure: facts.has_complex_structure,
        });
    } else if selected {
        out.truncated = true;
    }
}
fn inspect_images(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    owners: &HashMap<NodeId, (String, Option<usize>)>,
    geometry: &[(Option<i64>, Option<i64>)],
    options: &LayoutOptions,
    out: &mut LayoutSnapshot,
) {
    let relationships = package.part_relationships(main).unwrap_or_else(|error| {
        out.diagnostic(error.code(), "image relationships could not be read");
        Vec::new()
    });
    let mut assets = HashMap::new();
    for picture in crate::picture::pictures(source) {
        out.image_count += 1;
        let mut ancestor = Some(picture.source_id());
        let mut owner = None;
        while let Some(id) = ancestor {
            if let Some(value) = owners.get(&id) {
                owner = Some(value.clone());
                break;
            }
            ancestor = source.node(id).and_then(|n| n.parent());
        }
        let section = owner.as_ref().and_then(|v| v.1);
        let extent = picture
            .extent()
            .map_err(|e| out.diagnostic(e.code(), "invalid drawing dimensions"))
            .ok()
            .flatten();
        if picture.kind() == crate::PictureKind::Anchored {
            out.diagnostic("UNSUPPORTED_FLOATING_OBJECT","anchored drawing dimensions are known; wrapping and final position require rendering");
        }
        if let (Some(size), Some(width)) = (extent, section.and_then(|i| geometry[i].0)) {
            if i128::from(size.width_emu) > i128::from(width) * 635 {
                out.diagnostic(
                    "IMAGE_WIDTH_EXCEEDS_PAGE",
                    "requested image display width exceeds usable page width",
                );
            }
        }
        if options.section_index.is_some_and(|v| Some(v) != section) {
            continue;
        }
        if out.images.len() == MAX_ITEMS {
            out.truncated = true;
            continue;
        }
        let reference = picture
            .image_reference_with_relationships(package, &relationships)
            .map_err(|e| out.diagnostic(e.code(), "image asset could not be resolved"))
            .ok();
        let part = match reference {
            Some(crate::ImageReference::Embedded(asset)) => Some(asset.part),
            Some(crate::ImageReference::LinkedInternal(part)) => Some(part),
            _ => None,
        };
        let dimensions = part.as_ref().and_then(|part| {
            let key = part.name.as_str().to_owned();
            *assets.entry(key).or_insert_with(|| {
                package
                    .read_part(part)
                    .ok()
                    .and_then(|bytes| crate::read_image_info(&bytes).ok())
                    .map(|info| info.dimensions)
            })
        });
        out.images.push(ImageLayout {
            kind: if picture.kind() == crate::PictureKind::Inline {
                "inline"
            } else {
                "anchored"
            },
            block_handle: owner.map(|v| v.0),
            section_index: section,
            relationship_id: picture.relationship_id().ok().flatten().map(str::to_owned),
            asset_part_name: part.map(|v| v.name.as_str().to_owned()),
            display_width_emu: extent.map(|v| v.width_emu),
            display_height_emu: extent.map(|v| v.height_emu),
            intrinsic_width_pixels: dimensions.map(|v| v.width_px),
            intrinsic_height_pixels: dimensions.map(|v| v.height_px),
        });
    }
}

impl LayoutSnapshot {
    fn diagnostic(&mut self, code: &str, message: &str) {
        if self.diagnostics.len() < MAX_ITEMS {
            self.diagnostics.push(StyleInspectionDiagnostic {
                code: code.into(),
                message: message.into(),
            });
        } else {
            self.truncated = true;
        }
    }
}
fn usable(
    size: Option<u32>,
    margins: &BTreeMap<String, i32>,
    first: &str,
    second: &str,
) -> Option<i64> {
    let first = *margins.get(first)?;
    let second = *margins.get(second)?;
    // Negative margins have Word-specific header/footer overlap behavior.
    if first < 0 || second < 0 {
        return None;
    }
    Some(i64::from(size?) - i64::from(first) - i64::from(second))
}
fn word(source: &SourceDocument, id: NodeId, local: &str) -> bool {
    matches!(source.node(id).map(|n|n.kind()),Some(SourceNodeKind::Element{name,..}) if name.local_name()==local && name.namespace_uri().is_some_and(|v|v=="http://schemas.openxmlformats.org/wordprocessingml/2006/main"||v=="http://purl.oclc.org/ooxml/wordprocessingml/main"))
}
fn child(source: &SourceDocument, id: NodeId, local: &str) -> Option<NodeId> {
    source.children(id).find(|id| word(source, *id, local))
}
fn attribute(source: &SourceDocument, id: NodeId, key: &str) -> Option<String> {
    source.node(id)?.attribute(key).map(str::to_owned)
}
fn number(source: &SourceDocument, id: NodeId, key: &str) -> Option<u32> {
    source.node(id)?.attribute(key)?.parse().ok()
}
fn on_off(source: &SourceDocument, id: NodeId, out: &mut LayoutSnapshot) -> Option<bool> {
    match source.node(id)?.attribute("val") {
        None | Some("1" | "true" | "on") => Some(true),
        Some("0" | "false" | "off") => Some(false),
        _ => {
            out.diagnostic("INVALID_FORMATTING_VALUE", "invalid row cannot-split value");
            None
        }
    }
}
fn descendants(source: &SourceDocument, root: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    let mut stack = vec![root];
    std::iter::from_fn(move || {
        let id = stack.pop()?;
        stack.extend(source.children(id));
        Some(id)
    })
}
#[cfg(test)]
mod tests;
