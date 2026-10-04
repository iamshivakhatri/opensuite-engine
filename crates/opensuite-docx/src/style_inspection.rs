use std::collections::{BTreeMap, HashMap};

use opensuite_opc::Package;
use serde::Serialize;

use crate::{
    BodyBlock, DocxDocument, HeaderFooterKind, HeaderFooterType, LineSpacingRule, NumberFormat,
    PageOrientation, Paragraph, ParagraphAlignment, ParagraphFormatting, RunFormatting,
    SectionType, SourceDocument, SourceNodeKind, StyleSheet, StyleType, load_footer, load_header,
    load_numbering, load_styles, open_main_source,
};

const MAX_ITEMS: usize = 128;
const MAX_DIAGNOSTICS: usize = 50;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleInspectionDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CountedValue {
    pub value: String,
    pub count: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunFormattingSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size_half_points: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_alignment: Option<String>,
}

impl From<&RunFormatting> for RunFormattingSnapshot {
    fn from(value: &RunFormatting) -> Self {
        Self {
            bold: value.bold,
            italic: value.italic,
            font_size_half_points: value.font_size_half_points,
            font_family: value.font_family.clone(),
            color: value.color.clone(),
            underline: value.underline,
            highlight: value.highlight.clone(),
            strikethrough: value.strikethrough,
            vertical_alignment: value
                .vertical_alignment
                .map(|value| format!("{value:?}").to_lowercase()),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParagraphFormattingSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alignment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing_before_twips: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing_after_twips: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing: Option<LineSpacingSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left_indent_twips: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right_indent_twips: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_line_indent_twips: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hanging_indent_twips: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_with_next: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_lines: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_break_before: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widow_control: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineSpacingSnapshot {
    pub value: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

impl From<&ParagraphFormatting> for ParagraphFormattingSnapshot {
    fn from(value: &ParagraphFormatting) -> Self {
        Self {
            alignment: value.alignment.map(alignment_name),
            spacing_before_twips: value.spacing_before_twips,
            spacing_after_twips: value.spacing_after_twips,
            line_spacing: value.line_spacing.map(|line| LineSpacingSnapshot {
                value: line.value,
                rule: line.rule.map(line_spacing_rule_name),
            }),
            left_indent_twips: value.left_indent_twips,
            right_indent_twips: value.right_indent_twips,
            first_line_indent_twips: value.first_line_indent_twips,
            hanging_indent_twips: value.hanging_indent_twips,
            keep_with_next: value.keep_with_next,
            keep_lines: value.keep_lines,
            page_break_before: value.page_break_before,
            widow_control: value.widow_control,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleUsageSnapshot {
    pub style_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub style_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub based_on_style_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_style_id: Option<String>,
    pub paragraph_usage_count: u32,
    pub run_usage_count: u32,
    pub declared_run_formatting: RunFormattingSnapshot,
    pub declared_paragraph_formatting: ParagraphFormattingSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_run_formatting: Option<RunFormattingSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_paragraph_formatting: Option<ParagraphFormattingSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParagraphPatternSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_name: Option<String>,
    pub direct_formatting: ParagraphFormattingSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_formatting: Option<ParagraphFormattingSnapshot>,
    pub usage_count: u32,
    pub locations: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPatternSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraph_style_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character_style_id: Option<String>,
    pub direct_formatting: RunFormattingSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_formatting: Option<RunFormattingSnapshot>,
    pub usage_count: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypographySnapshot {
    pub fonts: Vec<CountedValue>,
    pub font_sizes_half_points: Vec<CountedValue>,
    pub text_colors: Vec<CountedValue>,
    pub highlights: Vec<CountedValue>,
    pub bold_run_count: u32,
    pub italic_run_count: u32,
    pub underline_run_count: u32,
    pub run_patterns: Vec<RunPatternSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPatternSnapshot {
    pub numbering_id: u32,
    pub abstract_numbering_id: Option<u32>,
    pub level: u8,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_after_level: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left_indent_twips: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hanging_indent_twips: Option<i32>,
    pub usage_count: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BorderSnapshot {
    pub side: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_eighth_points: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSnapshot {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_twips: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_type: Option<String>,
    pub column_widths_twips: Vec<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alignment: Option<String>,
    pub borders: Vec<BorderSnapshot>,
    pub cell_margins_twips: BTreeMap<String, i32>,
    pub shading_colors: Vec<CountedValue>,
    pub border_colors: Vec<CountedValue>,
    pub merged_cell_count: u32,
    pub row_count: u32,
    pub column_count: u32,
    pub first_row_shading_colors: Vec<String>,
    pub first_row_bold_run_count: u32,
    pub has_complex_structure: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionSnapshot {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_width_twips: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_height_twips: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    pub margins_twips: BTreeMap<String, i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_count: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_spacing_twips: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equal_column_width: Option<bool>,
    pub different_first_page: bool,
    pub odd_even_headers: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number_start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number_format: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderFooterSnapshot {
    pub section_index: u32,
    pub kind: String,
    pub variant: String,
    pub paragraph_count: u32,
    pub table_count: u32,
    pub picture_count: u32,
    pub has_page_number: bool,
    pub style_ids: Vec<String>,
    pub fonts: Vec<String>,
    pub text_colors: Vec<String>,
    pub has_complex_content: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeReferenceSnapshot {
    pub property: String,
    pub value: String,
    pub count: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDefaultsSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_paragraph_style_id: Option<String>,
    pub run_formatting: RunFormattingSnapshot,
    pub paragraph_formatting: ParagraphFormattingSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocxStyleSnapshot {
    pub ok: bool,
    pub schema_version: u32,
    pub paragraph_count: u32,
    pub run_count: u32,
    pub table_count: u32,
    pub section_count: u32,
    pub defaults: DocumentDefaultsSnapshot,
    pub styles: Vec<StyleUsageSnapshot>,
    pub typography: TypographySnapshot,
    pub paragraph_patterns: Vec<ParagraphPatternSnapshot>,
    pub lists: Vec<ListPatternSnapshot>,
    pub tables: Vec<TableSnapshot>,
    pub sections: Vec<SectionSnapshot>,
    pub headers_footers: Vec<HeaderFooterSnapshot>,
    pub theme_references: Vec<ThemeReferenceSnapshot>,
    pub truncated: bool,
    pub diagnostics: Vec<StyleInspectionDiagnostic>,
}

impl DocxStyleSnapshot {
    fn failed(code: &str, message: &str) -> Self {
        Self {
            ok: false,
            schema_version: 1,
            paragraph_count: 0,
            run_count: 0,
            table_count: 0,
            section_count: 0,
            defaults: DocumentDefaultsSnapshot::default(),
            styles: Vec::new(),
            typography: TypographySnapshot::default(),
            paragraph_patterns: Vec::new(),
            lists: Vec::new(),
            tables: Vec::new(),
            sections: Vec::new(),
            headers_footers: Vec::new(),
            theme_references: Vec::new(),
            truncated: false,
            diagnostics: vec![StyleInspectionDiagnostic {
                code: code.to_owned(),
                message: message.to_owned(),
            }],
        }
    }
}

#[derive(Default)]
struct Collector {
    paragraph_count: u32,
    run_count: u32,
    style_paragraph_counts: HashMap<String, u32>,
    style_run_counts: HashMap<String, u32>,
    paragraph_patterns: Vec<ParagraphPatternSnapshot>,
    paragraph_pattern_indexes: HashMap<String, usize>,
    run_patterns: Vec<RunPatternSnapshot>,
    run_pattern_indexes: HashMap<String, usize>,
    fonts: BTreeMap<String, u32>,
    sizes: BTreeMap<String, u32>,
    colors: BTreeMap<String, u32>,
    highlights: BTreeMap<String, u32>,
    bold: u32,
    italic: u32,
    underline: u32,
    diagnostics: Vec<StyleInspectionDiagnostic>,
    truncated: bool,
}

/// Reads style facts from immutable DOCX bytes. It never serializes or mutates the package.
pub fn inspect_docx_style_snapshot(input: Vec<u8>) -> DocxStyleSnapshot {
    let package = match Package::from_bytes(input) {
        Ok(value) => value,
        Err(error) => {
            return DocxStyleSnapshot::failed(error.code(), "could not load DOCX artifact");
        }
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => {
            return DocxStyleSnapshot::failed(error.code(), "could not load DOCX main document");
        }
    };
    let document = match DocxDocument::new(&source) {
        Ok(value) => value,
        Err(error) => {
            return DocxStyleSnapshot::failed(error.code(), "could not inspect DOCX document");
        }
    };
    let mut collector = Collector::default();
    let styles = match load_styles(&package, &main) {
        Ok(value) => value,
        Err(error) => {
            collector.diagnostic(error.code(), "could not resolve DOCX styles");
            None
        }
    };
    let numbering = match load_numbering(&package, &main) {
        Ok(value) => value,
        Err(error) => {
            collector.diagnostic(error.code(), "could not resolve DOCX numbering");
            None
        }
    };
    if numbering.as_ref().is_some_and(|numbering| {
        numbering.source().node_ids().any(|id| {
            is_word(numbering.source(), id, "lvlOverride")
                || is_word(numbering.source(), id, "startOverride")
        })
    }) {
        collector.diagnostic(
            "UNSUPPORTED_NUMBERING_OVERRIDE",
            "numbering restart overrides are present but not resolved",
        );
    }

    let mut list_patterns: Vec<ListPatternSnapshot> = Vec::new();
    let mut list_indexes = HashMap::new();
    let mut tables = Vec::new();
    let mut table_count = 0_u32;
    for block in document.blocks() {
        match block {
            BodyBlock::Paragraph(paragraph) => collect_paragraph(
                &paragraph,
                "body",
                styles.as_ref(),
                numbering.as_ref(),
                &mut collector,
                &mut list_patterns,
                &mut list_indexes,
            ),
            BodyBlock::Table(table) => {
                table_count += 1;
                if tables.len() < MAX_ITEMS {
                    let table = inspect_table(&source, table.source_id(), tables.len() as u32);
                    if table.has_complex_structure {
                        collector.diagnostic(
                            "COMPLEX_TABLE_STRUCTURE",
                            "a table contains merged, uneven, or nested structure",
                        );
                    }
                    if table.style_id.is_some() {
                        collector.diagnostic(
                            "UNRESOLVED_TABLE_STYLE_FORMATTING",
                            "a named table style is preserved by ID but its inherited appearance is not resolved",
                        );
                    }
                    tables.push(table);
                } else {
                    collector.truncated = true;
                }
                for row in table.rows() {
                    for cell in row.cells() {
                        for paragraph in cell.paragraphs() {
                            collect_paragraph(
                                &paragraph,
                                "table_cell",
                                styles.as_ref(),
                                numbering.as_ref(),
                                &mut collector,
                                &mut list_patterns,
                                &mut list_indexes,
                            );
                        }
                    }
                }
            }
        }
    }

    let sections = inspect_sections(&document, &source, &mut collector);
    let headers_footers =
        inspect_headers_footers(&package, &main, &document, styles.as_ref(), &mut collector);
    let style_usage = inspect_styles(styles.as_ref(), &mut collector);
    let defaults = styles
        .as_ref()
        .map_or_else(DocumentDefaultsSnapshot::default, |styles| {
            DocumentDefaultsSnapshot {
                default_paragraph_style_id: styles
                    .default_paragraph_style_id()
                    .map(|id| id.as_str().to_owned()),
                run_formatting: styles.run_defaults().into(),
                paragraph_formatting: styles.paragraph_defaults().into(),
            }
        });
    let mut theme_counts = BTreeMap::new();
    collect_theme_references(&source, &mut theme_counts);
    if let Some(styles) = &styles {
        collect_theme_references(styles.source(), &mut theme_counts);
    }
    if !theme_counts.is_empty() {
        collector.diagnostic(
            "UNRESOLVED_THEME_FORMATTING",
            "theme references are preserved but not resolved to concrete fonts or colors",
        );
    }
    let diagnostics = collector.diagnostics;
    DocxStyleSnapshot {
        ok: true,
        schema_version: 1,
        paragraph_count: collector.paragraph_count,
        run_count: collector.run_count,
        table_count,
        section_count: document.sections().count() as u32,
        defaults,
        styles: style_usage,
        typography: TypographySnapshot {
            fonts: counted_values(collector.fonts),
            font_sizes_half_points: counted_values(collector.sizes),
            text_colors: counted_values(collector.colors),
            highlights: counted_values(collector.highlights),
            bold_run_count: collector.bold,
            italic_run_count: collector.italic,
            underline_run_count: collector.underline,
            run_patterns: collector.run_patterns,
        },
        paragraph_patterns: collector.paragraph_patterns,
        lists: list_patterns,
        tables,
        sections,
        headers_footers,
        theme_references: theme_counts
            .into_iter()
            .take(MAX_ITEMS)
            .map(|((property, value), count)| ThemeReferenceSnapshot {
                property,
                value,
                count,
            })
            .collect(),
        truncated: collector.truncated,
        diagnostics,
    }
}

fn collect_paragraph(
    paragraph: &Paragraph<'_>,
    location: &str,
    styles: Option<&StyleSheet>,
    numbering: Option<&crate::Numbering>,
    collector: &mut Collector,
    list_patterns: &mut Vec<ListPatternSnapshot>,
    list_indexes: &mut HashMap<String, usize>,
) {
    collector.paragraph_count += 1;
    let style_id = paragraph.style_id();
    if let Some(id) = &style_id {
        *collector
            .style_paragraph_counts
            .entry(id.as_str().to_owned())
            .or_default() += 1;
    }
    let direct = paragraph.direct_formatting().unwrap_or_else(|error| {
        collector.diagnostic(error.code(), "could not read direct paragraph formatting");
        ParagraphFormatting::default()
    });
    let effective = styles.and_then(|styles| match paragraph.effective_formatting(styles) {
        Ok(value) => Some(value),
        Err(error) => {
            collector.diagnostic(
                error.code(),
                "could not resolve paragraph style inheritance",
            );
            None
        }
    });
    let style_name = styles
        .and_then(|styles| style_id.as_ref().and_then(|id| styles.style(id)))
        .and_then(|style| style.name())
        .map(str::to_owned);
    let direct_snapshot = ParagraphFormattingSnapshot::from(&direct);
    let effective_snapshot = effective.as_ref().map(ParagraphFormattingSnapshot::from);
    let key = format!(
        "{:?}|{:?}|{:?}",
        style_id.as_ref().map(|id| id.as_str()),
        direct_snapshot,
        effective_snapshot
    );
    if let Some(index) = collector.paragraph_pattern_indexes.get(&key).copied() {
        let pattern = &mut collector.paragraph_patterns[index];
        pattern.usage_count += 1;
        if pattern.locations.len() < 4 && !pattern.locations.iter().any(|item| item == location) {
            pattern.locations.push(location.to_owned());
        }
    } else if collector.paragraph_patterns.len() < MAX_ITEMS {
        collector
            .paragraph_pattern_indexes
            .insert(key, collector.paragraph_patterns.len());
        collector.paragraph_patterns.push(ParagraphPatternSnapshot {
            style_id: style_id.as_ref().map(|id| id.as_str().to_owned()),
            style_name,
            direct_formatting: direct_snapshot,
            effective_formatting: effective_snapshot,
            usage_count: 1,
            locations: vec![location.to_owned()],
        });
    } else {
        collector.truncated = true;
    }

    if let (Some(styles), Some(numbering)) = (styles, numbering) {
        if let Ok(Some(reference)) = paragraph.list_reference(styles) {
            if let Ok(level) = numbering.resolve(reference) {
                let abstract_id = numbering
                    .instances()
                    .find(|item| item.num_id == reference.num_id)
                    .map(|item| item.abstract_num_id.0);
                let key = format!("{}:{}", reference.num_id.0, reference.level);
                if let Some(index) = list_indexes.get(&key).copied() {
                    list_patterns[index].usage_count += 1;
                } else if list_patterns.len() < MAX_ITEMS {
                    list_indexes.insert(key, list_patterns.len());
                    list_patterns.push(ListPatternSnapshot {
                        numbering_id: reference.num_id.0,
                        abstract_numbering_id: abstract_id,
                        level: reference.level,
                        format: number_format_name(&level.format),
                        start: level.start,
                        restart_after_level: level.restart_after_level,
                        text: level.text.clone(),
                        suffix: level.suffix.clone(),
                        left_indent_twips: level.left_indent_twips,
                        hanging_indent_twips: level.hanging_indent_twips,
                        usage_count: 1,
                    });
                }
            }
        }
    }

    for run in paragraph.runs() {
        collector.run_count += 1;
        let character_style = run.character_style_id();
        if let Some(id) = &character_style {
            *collector
                .style_run_counts
                .entry(id.as_str().to_owned())
                .or_default() += 1;
        }
        let direct = run.direct_formatting().unwrap_or_else(|error| {
            collector.diagnostic(error.code(), "could not read direct run formatting");
            RunFormatting::default()
        });
        let effective = styles.and_then(|styles| {
            match styles.effective_run_formatting(
                style_id.as_ref(),
                character_style.as_ref(),
                &direct,
            ) {
                Ok(value) => Some(value),
                Err(error) => {
                    collector.diagnostic(error.code(), "could not resolve run style inheritance");
                    None
                }
            }
        });
        let counted = effective.as_ref().unwrap_or(&direct);
        count_run_formatting(counted, collector);
        let direct_snapshot = RunFormattingSnapshot::from(&direct);
        let effective_snapshot = effective.as_ref().map(RunFormattingSnapshot::from);
        let key = format!(
            "{:?}|{:?}|{:?}|{:?}",
            style_id.as_ref().map(|id| id.as_str()),
            character_style.as_ref().map(|id| id.as_str()),
            direct_snapshot,
            effective_snapshot
        );
        if let Some(index) = collector.run_pattern_indexes.get(&key).copied() {
            collector.run_patterns[index].usage_count += 1;
        } else if collector.run_patterns.len() < MAX_ITEMS {
            collector
                .run_pattern_indexes
                .insert(key, collector.run_patterns.len());
            collector.run_patterns.push(RunPatternSnapshot {
                paragraph_style_id: style_id.as_ref().map(|id| id.as_str().to_owned()),
                character_style_id: character_style.as_ref().map(|id| id.as_str().to_owned()),
                direct_formatting: direct_snapshot,
                effective_formatting: effective_snapshot,
                usage_count: 1,
            });
        } else {
            collector.truncated = true;
        }
    }
}

fn inspect_styles(
    styles: Option<&StyleSheet>,
    collector: &mut Collector,
) -> Vec<StyleUsageSnapshot> {
    let Some(styles) = styles else {
        return Vec::new();
    };
    let mut selected = styles.styles().collect::<Vec<_>>();
    selected.sort_by(|a, b| a.id().as_str().cmp(b.id().as_str()));
    collector.truncated |= selected.len() > MAX_ITEMS;
    let result = selected
        .into_iter()
        .take(MAX_ITEMS)
        .map(|style| StyleUsageSnapshot {
            style_id: style.id().as_str().to_owned(),
            name: style.name().map(str::to_owned),
            style_type: match style.style_type() {
                StyleType::Paragraph => "paragraph",
                StyleType::Character => "character",
            }
            .to_owned(),
            based_on_style_id: style.based_on().map(|id| id.as_str().to_owned()),
            next_style_id: style.next().map(|id| id.as_str().to_owned()),
            paragraph_usage_count: collector
                .style_paragraph_counts
                .get(style.id().as_str())
                .copied()
                .unwrap_or_default(),
            run_usage_count: collector
                .style_run_counts
                .get(style.id().as_str())
                .copied()
                .unwrap_or_default(),
            declared_run_formatting: style.run_formatting().into(),
            declared_paragraph_formatting: style.paragraph_formatting().into(),
            effective_run_formatting: styles
                .effective_run_formatting(
                    (style.style_type() == StyleType::Paragraph).then_some(style.id()),
                    (style.style_type() == StyleType::Character).then_some(style.id()),
                    &crate::RunFormatting::default(),
                )
                .ok()
                .as_ref()
                .map(RunFormattingSnapshot::from),
            effective_paragraph_formatting: (style.style_type() == StyleType::Paragraph)
                .then(|| {
                    styles
                        .effective_paragraph_formatting(
                            Some(style.id()),
                            &crate::ParagraphFormatting::default(),
                        )
                        .ok()
                })
                .flatten()
                .as_ref()
                .map(ParagraphFormattingSnapshot::from),
        })
        .collect::<Vec<_>>();
    result
}

fn inspect_sections(
    document: &DocxDocument<'_>,
    source: &SourceDocument,
    collector: &mut Collector,
) -> Vec<SectionSnapshot> {
    document
        .sections()
        .enumerate()
        .take(MAX_ITEMS)
        .filter_map(|(index, section)| {
            section_snapshot(&section, source, index)
                .map_err(|error| {
                    collector.diagnostic(error.code(), "could not read section geometry")
                })
                .ok()
        })
        .collect()
}

pub(crate) fn section_snapshot(
    section: &crate::Section<'_>,
    source: &SourceDocument,
    index: usize,
) -> Result<SectionSnapshot, crate::SectionError> {
    let properties = section.properties();
    let page_size = properties.page_size()?;
    let margins = properties.page_margins()?;
    let columns = properties.columns()?;
    let page_number =
        child(source, properties.source_id(), "pgNumType").and_then(|id| source.node(id));
    let mut margin_values = BTreeMap::new();
    if let Some(value) = margins {
        for (name, item) in [
            ("top", value.top_twips),
            ("right", value.right_twips),
            ("bottom", value.bottom_twips),
            ("left", value.left_twips),
            ("header", value.header_twips),
            ("footer", value.footer_twips),
            ("gutter", value.gutter_twips),
        ] {
            if let Some(item) = item {
                margin_values.insert(name.to_owned(), item);
            }
        }
    }
    Ok(SectionSnapshot {
        index: index as u32,
        section_type: properties.section_type().map(section_type_name),
        page_width_twips: page_size.as_ref().and_then(|value| value.width_twips),
        page_height_twips: page_size.as_ref().and_then(|value| value.height_twips),
        orientation: page_size
            .and_then(|value| value.orientation)
            .map(orientation_name),
        margins_twips: margin_values,
        column_count: columns.as_ref().and_then(|value| value.count),
        column_spacing_twips: columns.as_ref().and_then(|value| value.spacing_twips),
        equal_column_width: columns.and_then(|value| value.equal_width),
        different_first_page: has_child(source, properties.source_id(), "titlePg"),
        odd_even_headers: section
            .header_references()
            .chain(section.footer_references())
            .any(|reference| reference.reference_type() == &HeaderFooterType::Even),
        page_number_start: page_number
            .and_then(|node| node.attribute("start"))
            .and_then(|value| value.parse().ok()),
        page_number_format: page_number
            .and_then(|node| node.attribute("fmt"))
            .map(str::to_owned),
    })
}

fn inspect_headers_footers(
    package: &Package,
    main: &opensuite_opc::Part,
    document: &DocxDocument<'_>,
    styles: Option<&StyleSheet>,
    collector: &mut Collector,
) -> Vec<HeaderFooterSnapshot> {
    let mut result = Vec::new();
    for (section_index, section) in document.sections().enumerate() {
        let references = section
            .header_references()
            .chain(section.footer_references());
        for reference in references {
            if result.len() >= MAX_ITEMS {
                collector.truncated = true;
                return result;
            }
            let loaded = match reference.kind() {
                HeaderFooterKind::Header => load_header(package, main, &reference),
                HeaderFooterKind::Footer => load_footer(package, main, &reference),
            };
            let header_footer = match loaded {
                Ok(value) => value,
                Err(error) => {
                    collector.diagnostic(error.code(), "could not load header or footer");
                    continue;
                }
            };
            let mut paragraph_count = 0;
            let mut table_count = 0;
            let mut style_ids = Vec::new();
            let mut fonts = Vec::new();
            let mut colors = Vec::new();
            for block in header_footer.blocks() {
                match block {
                    BodyBlock::Paragraph(paragraph) => {
                        paragraph_count += 1;
                        if let Some(id) = paragraph.style_id().map(|id| id.as_str().to_owned()) {
                            push_unique(&mut style_ids, id);
                        }
                        for run in paragraph.runs() {
                            let direct = run.direct_formatting().unwrap_or_default();
                            let effective = styles.and_then(|sheet| {
                                sheet
                                    .effective_run_formatting(
                                        paragraph.style_id().as_ref(),
                                        run.character_style_id().as_ref(),
                                        &direct,
                                    )
                                    .ok()
                            });
                            let format = effective.as_ref().unwrap_or(&direct);
                            if let Some(value) = &format.font_family {
                                push_unique(&mut fonts, value.clone());
                            }
                            if let Some(value) = &format.color {
                                push_unique(&mut colors, value.clone());
                            }
                        }
                    }
                    BodyBlock::Table(_) => table_count += 1,
                }
            }
            let has_page_number = header_footer.fields().iter().any(|field| {
                field.instruction().ok().flatten().is_some_and(|value| {
                    value
                        .split_whitespace()
                        .any(|word| word.eq_ignore_ascii_case("PAGE"))
                })
            });
            let picture_count = header_footer.pictures().count() as u32;
            let has_complex_content = table_count > 0
                || !header_footer
                    .content_controls()
                    .collect::<Vec<_>>()
                    .is_empty()
                || !header_footer
                    .tracked_changes()
                    .collect::<Vec<_>>()
                    .is_empty();
            if has_complex_content {
                collector.diagnostic(
                    "COMPLEX_HEADER_FOOTER_CONTENT",
                    "a header or footer contains a table, content control, or tracked change",
                );
            }
            result.push(HeaderFooterSnapshot {
                section_index: section_index as u32,
                kind: match reference.kind() {
                    HeaderFooterKind::Header => "header",
                    HeaderFooterKind::Footer => "footer",
                }
                .to_owned(),
                variant: match reference.reference_type() {
                    HeaderFooterType::Default => "default".to_owned(),
                    HeaderFooterType::First => "first".to_owned(),
                    HeaderFooterType::Even => "even".to_owned(),
                    HeaderFooterType::Unknown(value) => format!("unknown:{value}"),
                },
                paragraph_count,
                table_count,
                picture_count,
                has_page_number,
                style_ids,
                fonts,
                text_colors: colors,
                has_complex_content,
            });
        }
    }
    result
}

pub(crate) fn inspect_table(
    source: &SourceDocument,
    table_id: crate::NodeId,
    index: u32,
) -> TableSnapshot {
    let properties = child(source, table_id, "tblPr");
    let width_node = properties
        .and_then(|id| child(source, id, "tblW"))
        .and_then(|id| source.node(id));
    let mut snapshot = TableSnapshot {
        index,
        style_id: properties
            .and_then(|id| child(source, id, "tblStyle"))
            .and_then(|id| source.node(id))
            .and_then(|node| node.attribute("val"))
            .map(str::to_owned),
        width_twips: width_node
            .and_then(|node| node.attribute("w"))
            .and_then(|value| value.parse().ok()),
        width_type: width_node
            .and_then(|node| node.attribute("type"))
            .map(str::to_owned),
        alignment: properties
            .and_then(|id| child(source, id, "jc"))
            .and_then(|id| source.node(id))
            .and_then(|node| node.attribute("val"))
            .map(str::to_owned),
        ..TableSnapshot::default()
    };
    if let Some(grid) = child(source, table_id, "tblGrid") {
        snapshot.column_widths_twips = source
            .children(grid)
            .filter(|id| is_word(source, *id, "gridCol"))
            .filter_map(|id| {
                source
                    .node(id)
                    .and_then(|node| node.attribute("w"))
                    .and_then(|value| value.parse().ok())
            })
            .collect();
    }
    if let Some(margins) = properties.and_then(|id| child(source, id, "tblCellMar")) {
        for side in ["top", "right", "bottom", "left"] {
            if let Some(value) = child(source, margins, side)
                .and_then(|id| source.node(id))
                .and_then(|node| node.attribute("w"))
                .and_then(|value| value.parse().ok())
            {
                snapshot.cell_margins_twips.insert(side.to_owned(), value);
            }
        }
    }
    if let Some(borders) = properties.and_then(|id| child(source, id, "tblBorders")) {
        for id in source.children(borders) {
            let Some(name) = local_name(source, id) else {
                continue;
            };
            let Some(node) = source.node(id) else {
                continue;
            };
            snapshot.borders.push(BorderSnapshot {
                side: name.to_owned(),
                style: node.attribute("val").map(str::to_owned),
                color: node.attribute("color").map(str::to_owned),
                size_eighth_points: node.attribute("sz").and_then(|value| value.parse().ok()),
            });
        }
    }
    let row_ids = source
        .children(table_id)
        .filter(|id| is_word(source, *id, "tr"))
        .collect::<Vec<_>>();
    snapshot.row_count = row_ids.len() as u32;
    snapshot.column_count = row_ids
        .iter()
        .map(|id| {
            source
                .children(*id)
                .filter(|child| is_word(source, *child, "tc"))
                .count()
        })
        .max()
        .unwrap_or_default() as u32;
    let first_row = row_ids.first().copied();
    let mut shading = BTreeMap::new();
    let mut border_colors = BTreeMap::new();
    for id in descendants(source, table_id) {
        if is_word(source, id, "shd") {
            if let Some(fill) = source.node(id).and_then(|node| node.attribute("fill")) {
                *shading.entry(fill.to_owned()).or_default() += 1;
                if first_row.is_some_and(|row| is_descendant_of(source, id, row)) {
                    push_unique(&mut snapshot.first_row_shading_colors, fill.to_owned());
                }
            }
        }
        if ["top", "right", "bottom", "left", "insideH", "insideV"]
            .iter()
            .any(|name| is_word(source, id, name))
        {
            if let Some(color) = source.node(id).and_then(|node| node.attribute("color")) {
                *border_colors.entry(color.to_owned()).or_default() += 1;
            }
        }
        if is_word(source, id, "gridSpan") || is_word(source, id, "vMerge") {
            snapshot.merged_cell_count += 1;
        }
        if is_word(source, id, "tbl") && id != table_id {
            snapshot.has_complex_structure = true;
        }
        if is_word(source, id, "b")
            && first_row.is_some_and(|row| is_descendant_of(source, id, row))
            && source
                .node(id)
                .and_then(|node| node.attribute("val"))
                .is_none_or(|value| !matches!(value, "0" | "false" | "off"))
        {
            snapshot.first_row_bold_run_count += 1;
        }
    }
    snapshot.has_complex_structure |= snapshot.merged_cell_count > 0
        || row_ids.iter().any(|id| {
            source
                .children(*id)
                .filter(|child| is_word(source, *child, "tc"))
                .count() as u32
                != snapshot.column_count
        });
    snapshot.shading_colors = counted_values(shading);
    snapshot.border_colors = counted_values(border_colors);
    snapshot
}

fn collect_theme_references(source: &SourceDocument, counts: &mut BTreeMap<(String, String), u32>) {
    for id in source.node_ids() {
        let Some(node) = source.node(id) else {
            continue;
        };
        for property in [
            "themeColor",
            "themeFill",
            "asciiTheme",
            "hAnsiTheme",
            "eastAsiaTheme",
            "cstheme",
        ] {
            if let Some(value) = node.attribute(property) {
                *counts
                    .entry((property.to_owned(), value.to_owned()))
                    .or_default() += 1;
            }
        }
    }
}

fn count_run_formatting(format: &RunFormatting, collector: &mut Collector) {
    if let Some(value) = &format.font_family {
        *collector.fonts.entry(value.clone()).or_default() += 1;
    }
    if let Some(value) = format.font_size_half_points {
        *collector.sizes.entry(value.to_string()).or_default() += 1;
    }
    if let Some(value) = &format.color {
        *collector.colors.entry(value.clone()).or_default() += 1;
    }
    if let Some(value) = &format.highlight {
        *collector.highlights.entry(value.clone()).or_default() += 1;
    }
    collector.bold += u32::from(format.bold == Some(true));
    collector.italic += u32::from(format.italic == Some(true));
    collector.underline += u32::from(format.underline == Some(true));
}

impl Collector {
    fn diagnostic(&mut self, code: &str, message: &str) {
        if self.diagnostics.len() < MAX_DIAGNOSTICS
            && !self.diagnostics.iter().any(|item| item.code == code)
        {
            self.diagnostics.push(StyleInspectionDiagnostic {
                code: code.to_owned(),
                message: message.to_owned(),
            });
        } else if self.diagnostics.len() >= MAX_DIAGNOSTICS {
            self.truncated = true;
        }
    }
}

fn counted_values(values: BTreeMap<String, u32>) -> Vec<CountedValue> {
    values
        .into_iter()
        .take(MAX_ITEMS)
        .map(|(value, count)| CountedValue { value, count })
        .collect()
}
fn push_unique(values: &mut Vec<String>, value: String) {
    if values.len() < MAX_ITEMS && !values.contains(&value) {
        values.push(value);
    }
}
fn alignment_name(value: ParagraphAlignment) -> String {
    match value {
        ParagraphAlignment::Left => "left",
        ParagraphAlignment::Center => "center",
        ParagraphAlignment::Right => "right",
        ParagraphAlignment::Both => "both",
        ParagraphAlignment::Distribute => "distribute",
    }
    .to_owned()
}
fn line_spacing_rule_name(value: LineSpacingRule) -> String {
    match value {
        LineSpacingRule::Auto => "auto",
        LineSpacingRule::Exact => "exact",
        LineSpacingRule::AtLeast => "atLeast",
    }
    .to_owned()
}
fn number_format_name(value: &NumberFormat) -> String {
    match value {
        NumberFormat::Decimal => "decimal",
        NumberFormat::UpperRoman => "upperRoman",
        NumberFormat::LowerRoman => "lowerRoman",
        NumberFormat::UpperLetter => "upperLetter",
        NumberFormat::LowerLetter => "lowerLetter",
        NumberFormat::Bullet => "bullet",
        NumberFormat::None => "none",
        NumberFormat::Unknown(value) => value,
    }
    .to_owned()
}
fn section_type_name(value: SectionType) -> String {
    match value {
        SectionType::NextPage => "nextPage",
        SectionType::Continuous => "continuous",
        SectionType::EvenPage => "evenPage",
        SectionType::OddPage => "oddPage",
        SectionType::Unknown(value) => return value,
    }
    .to_owned()
}
fn orientation_name(value: PageOrientation) -> String {
    match value {
        PageOrientation::Portrait => "portrait",
        PageOrientation::Landscape => "landscape",
        PageOrientation::Unknown(value) => return value,
    }
    .to_owned()
}
fn local_name(source: &SourceDocument, id: crate::NodeId) -> Option<&str> {
    match source.node(id)?.kind() {
        SourceNodeKind::Element { name, .. } => Some(name.local_name()),
        SourceNodeKind::Text => None,
    }
}
fn is_word(source: &SourceDocument, id: crate::NodeId, name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name: xml_name, .. }) if xml_name.local_name() == name)
}
fn child(source: &SourceDocument, parent: crate::NodeId, name: &str) -> Option<crate::NodeId> {
    source
        .children(parent)
        .find(|id| is_word(source, *id, name))
}
fn has_child(source: &SourceDocument, parent: crate::NodeId, name: &str) -> bool {
    child(source, parent, name).is_some()
}
fn descendants(source: &SourceDocument, root: crate::NodeId) -> Vec<crate::NodeId> {
    let mut result = Vec::new();
    let mut stack = source.children(root).collect::<Vec<_>>();
    while let Some(id) = stack.pop() {
        result.push(id);
        stack.extend(source.children(id));
    }
    result
}
fn is_descendant_of(source: &SourceDocument, id: crate::NodeId, ancestor: crate::NodeId) -> bool {
    let mut current = source.node(id).and_then(|node| node.parent());
    while let Some(parent) = current {
        if parent == ancestor {
            return true;
        }
        current = source.node(parent).and_then(|node| node.parent());
    }
    false
}

#[cfg(test)]
mod tests {
    use std::{
        fmt::Write as _,
        io::{Cursor, Write as _},
    };

    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

    fn package(document: &str, extra_parts: &[(&str, &str)]) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(&mut bytes);
        let options = SimpleFileOptions::default();
        let mut overrides = String::new();
        let mut relationships = String::new();
        for (index, (name, _)) in extra_parts.iter().enumerate() {
            let (kind, content_type) = if name.ends_with("styles.xml") {
                (
                    "styles",
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml",
                )
            } else if name.ends_with("numbering.xml") {
                (
                    "numbering",
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml",
                )
            } else if name.contains("header") {
                (
                    "header",
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml",
                )
            } else {
                (
                    "footer",
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml",
                )
            };
            overrides.push_str(&format!(
                "<Override PartName=\"/{name}\" ContentType=\"{content_type}\"/>"
            ));
            relationships.push_str(&format!(
                "<Relationship Id=\"rId{}\" Type=\"{REL}/{kind}\" Target=\"{}\"/>",
                index + 1,
                name.trim_start_matches("word/")
            ));
        }
        let parts = [
            ("[Content_Types].xml", format!("<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>{overrides}</Types>")),
            ("_rels/.rels", "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>".to_owned()),
            ("word/_rels/document.xml.rels", format!("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{relationships}</Relationships>")),
            ("word/document.xml", document.to_owned()),
        ];
        for (name, value) in parts.iter().chain(
            extra_parts
                .iter()
                .map(|(name, value)| (*name, (*value).to_owned()))
                .collect::<Vec<_>>()
                .iter(),
        ) {
            zip.start_file(*name, options).unwrap();
            zip.write_all(value.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        bytes.into_inner()
    }

    #[test]
    fn business_report_exposes_styles_table_page_setup_and_footer_page_number() {
        let document = format!(
            "<w:document xmlns:w=\"{WORD}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><w:body><w:p><w:pPr><w:pStyle w:val=\"Title\"/></w:pPr><w:r><w:t>Report</w:t></w:r></w:p><w:tbl><w:tblPr><w:tblW w:w=\"9000\" w:type=\"dxa\"/><w:tblBorders><w:top w:val=\"single\" w:color=\"1F4E79\"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w=\"4500\"/><w:gridCol w:w=\"4500\"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:shd w:fill=\"D9EAF7\"/></w:tcPr><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\"/><w:footerReference w:type=\"default\" r:id=\"rId2\"/></w:sectPr></w:body></w:document>"
        );
        let styles = format!(
            "<w:styles xmlns:w=\"{WORD}\"><w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Title\"/><w:rPr><w:rFonts w:ascii=\"Aptos Display\"/><w:sz w:val=\"32\"/><w:color w:val=\"1F4E79\"/></w:rPr></w:style></w:styles>"
        );
        let footer = format!(
            "<w:ftr xmlns:w=\"{WORD}\"><w:p><w:fldSimple w:instr=\" PAGE \"><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p></w:ftr>"
        );
        let snapshot = inspect_docx_style_snapshot(package(
            &document,
            &[("word/styles.xml", &styles), ("word/footer1.xml", &footer)],
        ));
        assert!(snapshot.ok);
        assert_eq!(snapshot.styles[0].style_id, "Title");
        assert_eq!(snapshot.tables[0].first_row_shading_colors, ["D9EAF7"]);
        assert_eq!(snapshot.sections[0].margins_twips["top"], 1440);
        assert!(snapshot.headers_footers[0].has_page_number);
    }

    #[test]
    fn resume_like_document_exposes_bullets_direct_bold_and_spacing() {
        let document = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr><w:spacing w:after=\"120\"/><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"7\"/></w:numPr></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Experience</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"
        );
        let numbering = format!(
            "<w:numbering xmlns:w=\"{WORD}\"><w:abstractNum w:abstractNumId=\"3\"><w:lvl w:ilvl=\"0\"><w:numFmt w:val=\"bullet\"/><w:lvlText w:val=\"•\"/><w:lvlRestart w:val=\"1\"/><w:pPr><w:ind w:left=\"720\" w:hanging=\"360\"/></w:pPr></w:lvl></w:abstractNum><w:num w:numId=\"7\"><w:abstractNumId w:val=\"3\"/></w:num></w:numbering>"
        );
        let styles = format!(
            "<w:styles xmlns:w=\"{WORD}\"><w:style w:type=\"paragraph\" w:styleId=\"Normal\" w:default=\"1\"/></w:styles>"
        );
        let snapshot = inspect_docx_style_snapshot(package(
            &document,
            &[
                ("word/styles.xml", &styles),
                ("word/numbering.xml", &numbering),
            ],
        ));
        assert_eq!(snapshot.lists[0].format, "bullet");
        assert_eq!(snapshot.lists[0].left_indent_twips, Some(720));
        assert_eq!(snapshot.lists[0].restart_after_level, Some(1));
        assert_eq!(snapshot.typography.bold_run_count, 1);
        assert_eq!(
            snapshot.paragraph_patterns[0]
                .direct_formatting
                .spacing_after_twips,
            Some(120)
        );
    }

    #[test]
    fn table_heavy_document_exposes_widths_shading_borders_and_cell_formatting() {
        let document = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tblPr><w:tblBorders><w:top w:val=\"single\" w:color=\"FF0000\"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w=\"2000\"/><w:gridCol w:w=\"4000\"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:shd w:fill=\"333333\"/></w:tcPr><w:p><w:r><w:rPr><w:b/><w:color w:val=\"FFFFFF\"/></w:rPr><w:t>Head</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Value</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:sectPr/></w:body></w:document>"
        );
        let snapshot = inspect_docx_style_snapshot(package(&document, &[]));
        assert_eq!(snapshot.tables[0].column_widths_twips, [2000, 4000]);
        assert_eq!(snapshot.tables[0].border_colors[0].value, "FF0000");
        assert_eq!(snapshot.tables[0].first_row_shading_colors, ["333333"]);
        assert_eq!(snapshot.typography.text_colors[0].value, "FFFFFF");
    }

    #[test]
    fn multi_section_document_exposes_each_page_setup() {
        let document = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr><w:sectPr><w:type w:val=\"nextPage\"/><w:pgSz w:w=\"12240\" w:h=\"15840\"/></w:sectPr></w:pPr><w:r><w:t>One</w:t></w:r></w:p><w:sectPr><w:type w:val=\"continuous\"/><w:pgSz w:w=\"15840\" w:h=\"12240\" w:orient=\"landscape\"/><w:cols w:num=\"2\"/></w:sectPr></w:body></w:document>"
        );
        let snapshot = inspect_docx_style_snapshot(package(&document, &[]));
        assert_eq!(snapshot.section_count, 2);
        assert_eq!(
            snapshot.sections[1].orientation.as_deref(),
            Some("landscape")
        );
        assert_eq!(snapshot.sections[1].column_count, Some(2));
    }

    #[test]
    fn mixed_formatting_keeps_style_and_direct_values_separate_and_input_unchanged() {
        let document = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:rPr><w:b/><w:color w:val=\"FF0000\"/></w:rPr><w:t>Mixed</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"
        );
        let styles = format!(
            "<w:styles xmlns:w=\"{WORD}\"><w:style w:type=\"paragraph\" w:styleId=\"Heading1\"><w:name w:val=\"Heading 1\"/><w:rPr><w:rFonts w:asciiTheme=\"majorHAnsi\"/><w:sz w:val=\"28\"/><w:color w:val=\"1F4E79\" w:themeColor=\"accent1\"/></w:rPr></w:style></w:styles>"
        );
        let input = package(&document, &[("word/styles.xml", &styles)]);
        let original = input.clone();
        let snapshot = inspect_docx_style_snapshot(input.clone());
        let run = &snapshot.typography.run_patterns[0];
        assert_eq!(run.direct_formatting.color.as_deref(), Some("FF0000"));
        assert_eq!(
            run.effective_formatting
                .as_ref()
                .and_then(|value| value.font_size_half_points),
            Some(28)
        );
        assert!(snapshot
            .theme_references
            .iter()
            .any(|reference| reference.property == "themeColor" && reference.value == "accent1"));
        assert_eq!(input, original);
    }

    #[test]
    fn snapshot_uses_only_the_explicit_default_style_for_unstyled_content() {
        let document = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>Body</w:t></w:r></w:p><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Direct</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val=\"Heading3\"/></w:pPr><w:r><w:t>Heading</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"
        );
        let styles = format!(
            "<w:styles xmlns:w=\"{WORD}\"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii=\"Arial\"/><w:sz w:val=\"22\"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:before=\"160\" w:after=\"160\"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type=\"paragraph\" w:styleId=\"Normal\" w:default=\"1\"/><w:style w:type=\"paragraph\" w:styleId=\"Title\"/><w:style w:type=\"paragraph\" w:styleId=\"Heading1\"/><w:style w:type=\"paragraph\" w:styleId=\"Heading2\"/><w:style w:type=\"paragraph\" w:styleId=\"Heading3\"><w:pPr><w:spacing w:after=\"80\"/><w:keepNext/></w:pPr><w:rPr><w:b/><w:sz w:val=\"24\"/></w:rPr></w:style></w:styles>"
        );
        let snapshot =
            inspect_docx_style_snapshot(package(&document, &[("word/styles.xml", &styles)]));

        assert_eq!(
            snapshot.defaults.default_paragraph_style_id.as_deref(),
            Some("Normal")
        );
        let unstyled_paragraph = snapshot
            .paragraph_patterns
            .iter()
            .find(|pattern| pattern.style_id.is_none())
            .unwrap();
        let effective_paragraph = unstyled_paragraph.effective_formatting.as_ref().unwrap();
        assert_eq!(effective_paragraph.spacing_before_twips, Some(160));
        assert_eq!(effective_paragraph.spacing_after_twips, Some(160));
        assert_eq!(effective_paragraph.keep_with_next, None);

        let plain_run = snapshot
            .typography
            .run_patterns
            .iter()
            .find(|pattern| {
                pattern.paragraph_style_id.is_none() && pattern.direct_formatting.bold.is_none()
            })
            .unwrap();
        let plain_effective = plain_run.effective_formatting.as_ref().unwrap();
        assert_eq!(plain_effective.font_size_half_points, Some(22));
        assert_eq!(plain_effective.bold, None);

        let direct_run = snapshot
            .typography
            .run_patterns
            .iter()
            .find(|pattern| pattern.direct_formatting.bold == Some(true))
            .unwrap();
        assert_eq!(
            direct_run
                .effective_formatting
                .as_ref()
                .and_then(|formatting| formatting.bold),
            Some(true)
        );
        let heading_run = snapshot
            .typography
            .run_patterns
            .iter()
            .find(|pattern| pattern.paragraph_style_id.as_deref() == Some("Heading3"))
            .unwrap();
        assert_eq!(
            heading_run
                .effective_formatting
                .as_ref()
                .and_then(|formatting| formatting.font_size_half_points),
            Some(24)
        );
        assert_eq!(snapshot.typography.bold_run_count, 2);
    }

    #[test]
    fn large_document_output_is_bounded() {
        let mut paragraphs = String::new();
        for index in 0..400 {
            write!(paragraphs, "<w:p><w:pPr><w:spacing w:after=\"{index}\"/></w:pPr><w:r><w:rPr><w:sz w:val=\"{}\"/></w:rPr><w:t>{index}</w:t></w:r></w:p>", index + 10).unwrap();
        }
        let document = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body>{paragraphs}<w:sectPr/></w:body></w:document>"
        );
        let snapshot = inspect_docx_style_snapshot(package(&document, &[]));
        assert_eq!(snapshot.paragraph_count, 400);
        assert!(snapshot.paragraph_patterns.len() <= MAX_ITEMS);
        assert!(snapshot.typography.run_patterns.len() <= MAX_ITEMS);
        assert!(snapshot.typography.font_sizes_half_points.len() <= MAX_ITEMS);
        assert!(snapshot.truncated);
    }
}
