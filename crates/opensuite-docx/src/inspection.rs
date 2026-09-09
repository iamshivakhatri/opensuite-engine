use opensuite_opc::{Package, Part};
use opensuite_protocol::{
    Affordance, AffordanceReason, DocxBodyBlock, DocxBodyBlockKind, DocxHeading, DocxOverview,
    DocxParagraph, DocxPicture, DocxPictureFormat, DocxTable, DocxTableColumn, DocxTableRow,
    InspectDocx, InspectDocxContent, InspectDocxFocus, InspectDocxResult, InspectionPage,
};

use crate::{BodyBlock, DocxDocument, RevisionView, SourceDocument, StyleSheet, load_styles};

const MAX_LIMIT: usize = 100;

/// Runs bounded semantic DOCX inspection over an already-opened main document source.
pub fn inspect_docx_document(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    request: &InspectDocx,
) -> InspectDocxResult {
    let document = match DocxDocument::new(source) {
        Ok(document) => document,
        Err(error) => {
            return InspectDocxResult::failed(error.code(), "could not inspect DOCX artifact");
        }
    };
    match &request.focus {
        InspectDocxFocus::Overview => overview(document),
        InspectDocxFocus::BodyBlocks { offset, limit } => {
            body_blocks(package, main, source, document, *offset, *limit)
        }
        InspectDocxFocus::Headings { offset, limit } => {
            let styles = match load_styles(package, main) {
                Ok(styles) => styles,
                Err(error) => {
                    return InspectDocxResult::failed(
                        error.code(),
                        "could not inspect DOCX styles",
                    );
                }
            };
            headings(document, styles.as_ref(), *offset, *limit)
        }
        InspectDocxFocus::Paragraphs { offset, limit } => {
            let styles = match load_styles(package, main) {
                Ok(styles) => styles,
                Err(error) => {
                    return InspectDocxResult::failed(
                        error.code(),
                        "could not inspect DOCX styles",
                    );
                }
            };
            paragraphs(source, document, styles.as_ref(), *offset, *limit)
        }
        InspectDocxFocus::Tables { offset, limit } => tables(source, document, *offset, *limit),
        InspectDocxFocus::Context(request) => {
            let result = match crate::inspect_text_context(source, request) {
                Ok(result) => result,
                Err(error) => {
                    return InspectDocxResult::failed(
                        error.code(),
                        "could not inspect DOCX artifact",
                    );
                }
            };
            InspectDocxResult {
                diagnostics: result.diagnostics.clone(),
                content: Some(InspectDocxContent::Context(result)),
            }
        }
    }
}

fn overview(document: DocxDocument<'_>) -> InspectDocxResult {
    let mut body_block_count = 0;
    let mut paragraph_count = 0;
    let mut table_count = 0;
    for block in document.blocks() {
        body_block_count += 1;
        match block {
            BodyBlock::Paragraph(_) => paragraph_count += 1,
            BodyBlock::Table(_) => table_count += 1,
        }
    }
    InspectDocxResult::success(InspectDocxContent::Overview(DocxOverview {
        body_block_count,
        paragraph_count,
        table_count,
        section_count: document.sections().count(),
    }))
}

fn headings(
    document: DocxDocument<'_>,
    styles: Option<&StyleSheet>,
    offset: usize,
    limit: usize,
) -> InspectDocxResult {
    let Some(page) = page(offset, limit) else {
        return invalid_bounds();
    };
    let mut total = 0;
    let mut items = Vec::new();
    for paragraph in document.paragraphs() {
        let Some(style_name) = paragraph_style_name(&paragraph, styles) else {
            continue;
        };
        let Some(level) = heading_level(&style_name) else {
            continue;
        };
        let occurrence = total;
        total += 1;
        if occurrence < page.0 || items.len() == page.1 {
            continue;
        }
        let text = match paragraph.text_for_view(RevisionView::Current) {
            Ok(text) => text,
            Err(error) => {
                return InspectDocxResult::failed(error.code(), "could not inspect DOCX artifact");
            }
        };
        items.push(DocxHeading {
            occurrence,
            text,
            style_name,
            level: Some(level),
        });
    }
    InspectDocxResult::success(InspectDocxContent::Headings(page_result(
        total, page, items,
    )))
}

fn paragraphs(
    source: &SourceDocument,
    document: DocxDocument<'_>,
    styles: Option<&StyleSheet>,
    offset: usize,
    limit: usize,
) -> InspectDocxResult {
    let Some(page) = page(offset, limit) else {
        return invalid_bounds();
    };
    let mut total = 0;
    let mut items = Vec::new();
    for paragraph in document.paragraphs() {
        let occurrence = total;
        total += 1;
        if occurrence < page.0 || items.len() == page.1 {
            continue;
        }
        let text = match paragraph.text_for_view(RevisionView::Current) {
            Ok(text) => text,
            Err(error) => {
                return InspectDocxResult::failed(error.code(), "could not inspect DOCX artifact");
            }
        };
        items.push(DocxParagraph {
            occurrence,
            handle: direct_body_block_index(source, document.body_id(), paragraph.source_id())
                .map(|index| format!("b{index}")),
            text,
            style_name: paragraph_style_name(&paragraph, styles),
        });
    }
    InspectDocxResult::success(InspectDocxContent::Paragraphs(page_result(
        total, page, items,
    )))
}

fn body_blocks(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    document: DocxDocument<'_>,
    offset: usize,
    limit: usize,
) -> InspectDocxResult {
    let Some(page) = page(offset, limit) else {
        return invalid_bounds();
    };
    let body = document.body_id();
    let mut total = 0;
    let mut items = Vec::new();
    let mut table_index = 0;
    let mut picture_index = 0;
    for id in source.children(body) {
        let (kind, text, table_handle, picture, affordances) = if is_word(source, id, "p") {
            if page_break_paragraph(source, id) {
                (
                    DocxBodyBlockKind::PageBreak,
                    None,
                    None,
                    None,
                    vec![Affordance::supported("delete_page_break")],
                )
            } else if let Some(picture) = body_picture(package, main, source, id, picture_index) {
                picture_index += 1;
                (
                    DocxBodyBlockKind::Picture,
                    None,
                    None,
                    Some(picture),
                    Vec::new(),
                )
            } else {
                let text =
                    match crate::tracked_change::text_for_view(source, id, RevisionView::Current) {
                        Ok(text) => text,
                        Err(error) => {
                            return InspectDocxResult::failed(
                                error.code(),
                                "could not inspect DOCX artifact",
                            );
                        }
                    };
                (
                    DocxBodyBlockKind::Paragraph,
                    Some(text),
                    None,
                    None,
                    Vec::new(),
                )
            }
        } else if is_word(source, id, "tbl") {
            let handle = format!("t{table_index}");
            table_index += 1;
            (
                DocxBodyBlockKind::Table,
                None,
                Some(handle),
                None,
                Vec::new(),
            )
        } else {
            continue;
        };
        let index = total;
        total += 1;
        if index >= page.0 && items.len() < page.1 {
            items.push(DocxBodyBlock {
                handle: format!("b{index}"),
                kind,
                text,
                table_handle,
                picture,
                affordances,
            });
        }
    }
    InspectDocxResult::success(InspectDocxContent::BodyBlocks(page_result(
        total, page, items,
    )))
}

/// Recognizes only the canonical dedicated paragraph used for editable page breaks.
pub(crate) fn page_break_paragraph(source: &SourceDocument, paragraph: crate::NodeId) -> bool {
    let Some(body) = source.node(paragraph).and_then(|node| node.parent()) else {
        return false;
    };
    if !is_word(source, paragraph, "p")
        || !is_word(source, body, "body")
        || !source
            .node(body)
            .and_then(|node| node.parent())
            .is_some_and(|document| is_word(source, document, "document"))
    {
        return false;
    }
    let runs = source.children(paragraph).collect::<Vec<_>>();
    if runs.len() != 1 || !is_word(source, runs[0], "r") {
        return false;
    }
    let breaks = source.children(runs[0]).collect::<Vec<_>>();
    breaks.len() == 1
        && is_word(source, breaks[0], "br")
        && matches!(source.node(breaks[0]).map(|node| node.kind()), Some(crate::SourceNodeKind::Element { attributes, .. }) if attributes.len() == 1 && attributes[0].local_name() == "type" && attributes[0].value() == "page")
}

fn body_picture(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    paragraph: crate::NodeId,
    occurrence: usize,
) -> Option<DocxPicture> {
    let runs = source
        .children(paragraph)
        .filter(|id| !is_word(source, *id, "pPr"))
        .collect::<Vec<_>>();
    if runs.len() != 1 || !is_word(source, runs[0], "r") {
        return None;
    }
    let drawing = source.children(runs[0]).collect::<Vec<_>>();
    if drawing.len() != 1 || !is_word(source, drawing[0], "drawing") {
        return None;
    }
    let picture = DocxDocument::new(source)
        .ok()?
        .pictures()
        .find(|item| item.source_id() == drawing[0])?;
    if picture.kind() != crate::PictureKind::Inline {
        return None;
    }
    let extent = picture.extent().ok()??;
    let crate::ImageReference::Embedded(image) = picture.image_reference(package, main).ok()?
    else {
        return None;
    };
    let format = match image.part.content_type.as_str() {
        "image/png" => DocxPictureFormat::Png,
        "image/jpeg" => DocxPictureFormat::Jpeg,
        _ => return None,
    };
    Some(DocxPicture {
        handle: format!("p{occurrence}"),
        format,
        width_emu: extent.width_emu,
        height_emu: extent.height_emu,
        alt_text: picture.metadata().and_then(|meta| meta.description),
        affordances: vec![
            Affordance::supported("replace_picture"),
            Affordance::supported("delete_picture"),
            Affordance::supported("set_picture_size"),
        ],
    })
}

pub(crate) fn picture_source_for_handle(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    handle: &str,
) -> Option<crate::NodeId> {
    let index = handle.strip_prefix('p')?.parse::<usize>().ok()?;
    let document = DocxDocument::new(source).ok()?;
    source
        .children(document.body_id())
        .filter(|id| is_word(source, *id, "p"))
        .filter_map(|paragraph| {
            let picture = body_picture(package, main, source, paragraph, 0)?;
            let drawing = source
                .children(paragraph)
                .find(|id| is_word(source, *id, "r"))
                .and_then(|run| {
                    source
                        .children(run)
                        .find(|id| is_word(source, *id, "drawing"))
                })?;
            Some((picture, drawing))
        })
        .nth(index)
        .map(|(_, drawing)| drawing)
}

fn direct_body_block_index(
    source: &SourceDocument,
    body: crate::NodeId,
    target: crate::NodeId,
) -> Option<usize> {
    source
        .children(body)
        .filter(|id| is_word(source, *id, "p") || is_word(source, *id, "tbl"))
        .position(|id| id == target)
}

fn is_word(source: &SourceDocument, id: crate::NodeId, local: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(crate::SourceNodeKind::Element { name, .. }) if name.local_name() == local && name.namespace_uri().is_some_and(|uri| matches!(uri, "http://schemas.openxmlformats.org/wordprocessingml/2006/main" | "http://purl.oclc.org/ooxml/wordprocessingml/main")))
}

fn tables(
    source: &SourceDocument,
    document: DocxDocument<'_>,
    offset: usize,
    limit: usize,
) -> InspectDocxResult {
    let Some(page) = page(offset, limit) else {
        return invalid_bounds();
    };
    let mut total = 0;
    let mut items = Vec::new();
    for block in document.blocks() {
        let BodyBlock::Table(table) = block else {
            continue;
        };
        let occurrence = total;
        total += 1;
        if occurrence < page.0 || items.len() == page.1 {
            continue;
        }
        let table_id = table.source_id();
        let cell_table_reason = crate::mutation::table_structure_reason(source, table_id);
        let table_reason = crate::mutation::table_mutation_reason_from_structure(
            source,
            table_id,
            cell_table_reason,
        );
        let width = table.rows().next().map_or(0, |row| row.cells().count());
        let mut rows = Vec::new();
        for (row_index, row) in table.rows().enumerate() {
            let mut cells = Vec::new();
            let mut cell_handles = Vec::new();
            let mut cell_affordances = Vec::new();
            for (column_index, cell) in row.cells().enumerate() {
                match cell.text_for_view(RevisionView::Current) {
                    Ok(text) => cells.push(text),
                    Err(error) => {
                        return InspectDocxResult::failed(
                            error.code(),
                            "could not inspect DOCX artifact",
                        );
                    }
                }
                cell_handles.push(format!("t{occurrence}:r{row_index}:c{column_index}"));
                let cell_reason = crate::mutation::table_cell_text_reason(source, cell.source_id());
                cell_affordances.push(vec![
                    affordance("set_table_cell_text", cell_table_reason.or(cell_reason)),
                    affordance("set_table_cells_text", table_reason.or(cell_reason)),
                ]);
            }
            rows.push(DocxTableRow {
                handle: format!("t{occurrence}:r{row_index}"),
                cells,
                cell_handles,
                cell_affordances,
            });
        }
        let is_rectangular = rows
            .first()
            .is_none_or(|first| rows.iter().all(|row| row.cells.len() == first.cells.len()));
        items.push(DocxTable {
            occurrence,
            handle: format!("t{occurrence}"),
            row_count: rows.len(),
            is_rectangular,
            affordances: vec![
                affordance("set_table_formatting", table_reason),
                affordance("delete_table", table_reason),
                affordance(
                    "delete_table_row",
                    table_reason
                        .or_else(|| (rows.len() == 1).then_some(AffordanceReason::LastTableRow)),
                ),
                affordance(
                    "delete_table_column",
                    table_reason
                        .or_else(|| (width == 1).then_some(AffordanceReason::LastTableColumn))
                        .or_else(|| crate::mutation::table_grid_reason(source, table_id, width)),
                ),
                affordance("insert_table_rows", table_reason),
                affordance(
                    "insert_table_column",
                    table_reason
                        .or_else(|| crate::mutation::table_grid_reason(source, table_id, width)),
                ),
            ],
            columns: rows.first().map_or_else(Vec::new, |row| {
                row.cells
                    .iter()
                    .enumerate()
                    .map(|(column, text)| DocxTableColumn {
                        occurrence: column,
                        handle: format!("t{occurrence}:c{column}"),
                        text: text.clone(),
                    })
                    .collect()
            }),
            rows,
        });
    }
    InspectDocxResult::success(InspectDocxContent::Tables(page_result(total, page, items)))
}

fn affordance(capability: &'static str, reason: Option<AffordanceReason>) -> Affordance {
    reason.map_or_else(
        || Affordance::supported(capability),
        |reason| Affordance::unsupported(capability, reason),
    )
}

fn paragraph_style_name(
    paragraph: &crate::Paragraph<'_>,
    styles: Option<&StyleSheet>,
) -> Option<String> {
    let style_id = paragraph.style_id()?;
    styles?.style(&style_id)?.name().map(str::to_owned)
}

fn heading_level(style_name: &str) -> Option<u8> {
    style_name
        .strip_prefix("Heading ")?
        .parse::<u8>()
        .ok()
        .filter(|level| (1..=9).contains(level))
}

fn page(offset: usize, limit: usize) -> Option<(usize, usize)> {
    (limit > 0 && limit <= MAX_LIMIT).then_some((offset, limit))
}

fn invalid_bounds() -> InspectDocxResult {
    InspectDocxResult::failed(
        "INVALID_INSPECTION_BOUNDS",
        format!("inspection limit must be between 1 and {MAX_LIMIT}"),
    )
}

fn page_result<T>(total: usize, (offset, _): (usize, usize), items: Vec<T>) -> InspectionPage<T> {
    let returned = items.len();
    InspectionPage {
        total,
        offset,
        returned,
        has_more: offset.saturating_add(returned) < total,
        items,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use opensuite_protocol::{
        AffordanceReason, InsertTableColumnAfter, InsertTableRowsAfter, InspectDocxContent,
        SetTableCellsText, TableCellTarget, TableCellTextUpdate, TableRowTarget, TableTarget,
    };

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    fn source(body: &str) -> SourceDocument {
        SourceDocument::parse(
            format!("<w:document xmlns:w=\"{WORD}\"><w:body>{body}</w:body></w:document>")
                .into_bytes(),
        )
        .unwrap()
    }

    fn png() -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&2u32.to_be_bytes());
        bytes.extend_from_slice(&[8, 2, 0, 0, 0, 0, 0, 0, 0]);
        bytes
    }

    #[test]
    fn exposes_an_inserted_inline_picture_as_a_body_block() {
        let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let bytes = crate::insert_picture_to_vec(
            &package,
            &main,
            &source,
            &opensuite_protocol::InsertPicture {
                image_bytes: png(),
                placement: opensuite_protocol::ParagraphPlacement::Start,
                alt_text: Some("diagram".to_owned()),
                base_revision: None,
            },
        )
        .unwrap();
        let package = Package::from_bytes(bytes).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let result = inspect_docx_document(
            &package,
            &main,
            &source,
            &InspectDocx {
                focus: InspectDocxFocus::BodyBlocks {
                    offset: 0,
                    limit: 10,
                },
            },
        );
        let Some(InspectDocxContent::BodyBlocks(page)) = result.content else {
            panic!("body blocks expected")
        };
        let picture = page.items[0].picture.as_ref().unwrap();
        assert_eq!(page.items[0].kind, DocxBodyBlockKind::Picture);
        assert_eq!(picture.handle, "p0");
        assert_eq!(picture.format, DocxPictureFormat::Png);
        assert_eq!((picture.width_emu, picture.height_emu), (9525, 19050));
        assert_eq!(picture.alt_text.as_deref(), Some("diagram"));
        assert_eq!(
            picture
                .affordances
                .iter()
                .map(|affordance| affordance.capability.as_str())
                .collect::<Vec<_>>(),
            ["replace_picture", "delete_picture", "set_picture_size"]
        );
    }

    #[test]
    fn recognizes_only_the_canonical_page_break_paragraph() {
        let canonical = source("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>");
        let body = canonical.children(canonical.root()).next().unwrap();
        let paragraph = canonical.children(body).next().unwrap();
        assert!(page_break_paragraph(&canonical, paragraph));

        for body in [
            "<w:p><w:r><w:t>Text</w:t><w:br w:type=\"page\"/></w:r></w:p>",
            "<w:p><w:r><w:br w:type=\"page\"/><w:br w:type=\"page\"/></w:r></w:p>",
            "<w:p><w:r><w:br w:type=\"page\" w:clear=\"all\"/></w:r></w:p>",
            "<w:p><w:r><w:br/></w:r></w:p>",
        ] {
            let source = source(body);
            let body = source.children(source.root()).next().unwrap();
            assert!(!page_break_paragraph(
                &source,
                source.children(body).next().unwrap()
            ));
        }
    }

    fn styles() -> StyleSheet {
        StyleSheet::parse(
            format!("<w:styles xmlns:w=\"{WORD}\"><w:style w:type=\"paragraph\" w:styleId=\"Heading1\"><w:name w:val=\"Heading 1\"/></w:style><w:style w:type=\"paragraph\" w:styleId=\"Body\"><w:name w:val=\"Body Text\"/></w:style></w:styles>").into_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn overview_counts_mixed_main_body_blocks() {
        let source = source(
            "<w:p><w:r><w:t>One</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:ins><w:r><w:t>Two</w:t></w:r></w:ins></w:p>",
        );
        let result = overview(DocxDocument::new(&source).unwrap());
        let Some(InspectDocxContent::Overview(value)) = result.content else {
            panic!("expected overview")
        };
        assert_eq!(value.body_block_count, 3);
        assert_eq!(value.paragraph_count, 2);
        assert_eq!(value.table_count, 1);
        assert_eq!(value.section_count, 0);
    }

    #[test]
    fn headings_and_paragraphs_are_ordered_bounded_and_current_view() {
        let source = source(
            "<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>First</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val=\"Body\"/></w:pPr><w:del><w:r><w:delText>old </w:delText></w:r></w:del><w:ins><w:r><w:t>new </w:t></w:r></w:ins><w:r><w:t>text</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>Second</w:t></w:r></w:p>",
        );
        let styles = styles();
        let headings = headings(DocxDocument::new(&source).unwrap(), Some(&styles), 1, 1);
        let Some(InspectDocxContent::Headings(headings)) = headings.content else {
            panic!("expected headings")
        };
        assert_eq!(headings.total, 2);
        assert!(!headings.has_more);
        assert_eq!(headings.items[0].occurrence, 1);
        assert_eq!(headings.items[0].text, "Second");
        assert_eq!(headings.items[0].style_name, "Heading 1");
        assert_eq!(headings.items[0].level, Some(1));

        let paragraphs = paragraphs(
            &source,
            DocxDocument::new(&source).unwrap(),
            Some(&styles),
            0,
            2,
        );
        let Some(InspectDocxContent::Paragraphs(paragraphs)) = paragraphs.content else {
            panic!("expected paragraphs")
        };
        assert_eq!(paragraphs.total, 3);
        assert!(paragraphs.has_more);
        assert_eq!(paragraphs.items[1].text, "new text");
        assert_eq!(paragraphs.items[1].style_name.as_deref(), Some("Body Text"));
    }

    #[test]
    fn tables_keep_uneven_rows_and_current_cell_text() {
        let source = source(
            "<w:tbl><w:tr><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:del><w:r><w:delText>old</w:delText></w:r></w:del><w:ins><w:r><w:t>new</w:t></w:r></w:ins></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>C</w:t></w:r></w:p></w:tc></w:tr></w:tbl>",
        );
        let result = tables(&source, DocxDocument::new(&source).unwrap(), 0, 1);
        let Some(InspectDocxContent::Tables(tables)) = result.content else {
            panic!("expected tables")
        };
        assert_eq!(tables.total, 2);
        assert!(tables.has_more);
        assert_eq!(tables.items[0].rows[1].cells, ["new"]);
        assert_eq!(tables.items[0].handle, "t0");
        assert_eq!(tables.items[0].columns[0].handle, "t0:c0");
        assert_eq!(tables.items[0].rows[1].handle, "t0:r1");
        assert_eq!(tables.items[0].rows[1].cell_handles[0], "t0:r1:c0");
        assert!(!tables.items[0].is_rectangular);
    }

    #[test]
    fn rejects_invalid_bounds_without_source_details() {
        let source = source("<w:p><w:r><w:t>text</w:t></w:r></w:p>");
        let result = paragraphs(&source, DocxDocument::new(&source).unwrap(), None, 0, 0);
        assert_eq!(result.diagnostics[0].code, "INVALID_INSPECTION_BOUNDS");
        let text = format!("{result:?}");
        assert!(!text.contains("NodeId"));
        assert!(!text.contains("SourceSpan"));
    }

    #[test]
    fn google_docs_table_affordances_match_current_cell_safety() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/google-docs-table.docx");
        let package = Package::open(fixture).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let result = tables(&source, DocxDocument::new(&source).unwrap(), 0, 1);
        let Some(InspectDocxContent::Tables(tables)) = result.content else {
            panic!("expected tables")
        };
        let table = &tables.items[0];

        assert!(table.affordances.iter().all(|item| item.supported));
        for (row, column, supported) in [(0, 0, true), (0, 1, false), (1, 0, true), (1, 1, false)] {
            let affordances = &table.rows[row].cell_affordances[column];
            assert!(affordances.iter().all(|item| item.supported == supported));
            if !supported {
                assert!(
                    affordances
                        .iter()
                        .all(|item| { item.reason == Some(AffordanceReason::MultipleParagraphs) })
                );
            }
        }

        let table_target = TableTarget {
            header_cells: Vec::new(),
            occurrence: None,
            handle: Some("t0".to_owned()),
        };
        assert!(
            crate::insert_table_rows_after_to_vec(
                &package,
                &main,
                &source,
                &InsertTableRowsAfter {
                    table: table_target.clone(),
                    after: TableRowTarget {
                        first_cell_text: String::new(),
                        occurrence: None,
                        handle: Some("t0:r1".to_owned()),
                    },
                    rows: vec![vec!["Other".to_owned(), "2027".to_owned()]],
                    base_revision: None,
                },
            )
            .is_ok()
        );
        assert!(
            crate::insert_table_column_after_to_vec(
                &package,
                &main,
                &source,
                &InsertTableColumnAfter {
                    table: table_target.clone(),
                    after_column_header: String::new(),
                    after_column_handle: Some("t0:c1".to_owned()),
                    header: "Status".to_owned(),
                    cells: vec!["Draft".to_owned()],
                    base_revision: None,
                },
            )
            .is_ok()
        );

        let supported = SetTableCellsText {
            table: table_target.clone(),
            updates: vec![
                TableCellTextUpdate {
                    target: TableCellTarget {
                        row_label: String::new(),
                        column_header: String::new(),
                        occurrence: None,
                        handle: Some("t0:r0:c0".to_owned()),
                    },
                    expected_current_text: "Name".to_owned(),
                    replacement: "Person".to_owned(),
                },
                TableCellTextUpdate {
                    target: TableCellTarget {
                        row_label: String::new(),
                        column_header: String::new(),
                        occurrence: None,
                        handle: Some("t0:r1:c0".to_owned()),
                    },
                    expected_current_text: "OpenSuite ".to_owned(),
                    replacement: "OpenSuite Engine".to_owned(),
                },
            ],
            base_revision: None,
        };
        assert!(crate::set_table_cells_text_to_vec(&package, &main, &source, &supported).is_ok());

        for (handle, text) in [("t0:r0:c1", "     Year"), ("t0:r1:c1", "2026")] {
            let unsupported = SetTableCellsText {
                table: table_target.clone(),
                updates: vec![TableCellTextUpdate {
                    target: TableCellTarget {
                        row_label: String::new(),
                        column_header: String::new(),
                        occurrence: None,
                        handle: Some(handle.to_owned()),
                    },
                    expected_current_text: text.to_owned(),
                    replacement: "changed".to_owned(),
                }],
                base_revision: None,
            };
            let error = crate::set_table_cells_text_to_vec(&package, &main, &source, &unsupported)
                .expect_err("multi-paragraph cells must remain unsupported");
            assert_eq!(error.diagnostics[0].code, "UNSUPPORTED_OPERATION");
            assert_eq!(
                error.diagnostics[0].reason_code.as_deref(),
                Some("MULTIPLE_PARAGRAPHS")
            );
        }
    }
}
