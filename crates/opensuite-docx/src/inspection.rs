use opensuite_opc::{Package, Part};
use opensuite_protocol::{
    DocxHeading, DocxOverview, DocxParagraph, DocxTable, DocxTableRow, InspectDocx,
    InspectDocxContent, InspectDocxFocus, InspectDocxResult, InspectionPage,
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
            paragraphs(document, styles.as_ref(), *offset, *limit)
        }
        InspectDocxFocus::Tables { offset, limit } => tables(document, *offset, *limit),
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
            text,
            style_name: paragraph_style_name(&paragraph, styles),
        });
    }
    InspectDocxResult::success(InspectDocxContent::Paragraphs(page_result(
        total, page, items,
    )))
}

fn tables(document: DocxDocument<'_>, offset: usize, limit: usize) -> InspectDocxResult {
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
        let mut rows = Vec::new();
        for row in table.rows() {
            let mut cells = Vec::new();
            for cell in row.cells() {
                match cell.text_for_view(RevisionView::Current) {
                    Ok(text) => cells.push(text),
                    Err(error) => {
                        return InspectDocxResult::failed(
                            error.code(),
                            "could not inspect DOCX artifact",
                        );
                    }
                }
            }
            rows.push(DocxTableRow { cells });
        }
        let is_rectangular = rows
            .first()
            .is_none_or(|first| rows.iter().all(|row| row.cells.len() == first.cells.len()));
        items.push(DocxTable {
            occurrence,
            row_count: rows.len(),
            is_rectangular,
            rows,
        });
    }
    InspectDocxResult::success(InspectDocxContent::Tables(page_result(total, page, items)))
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
    use opensuite_protocol::InspectDocxContent;

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    fn source(body: &str) -> SourceDocument {
        SourceDocument::parse(
            format!("<w:document xmlns:w=\"{WORD}\"><w:body>{body}</w:body></w:document>")
                .into_bytes(),
        )
        .unwrap()
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

        let paragraphs = paragraphs(DocxDocument::new(&source).unwrap(), Some(&styles), 0, 2);
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
        let result = tables(DocxDocument::new(&source).unwrap(), 0, 1);
        let Some(InspectDocxContent::Tables(tables)) = result.content else {
            panic!("expected tables")
        };
        assert_eq!(tables.total, 2);
        assert!(tables.has_more);
        assert_eq!(tables.items[0].rows[1].cells, ["new"]);
        assert!(!tables.items[0].is_rectangular);
    }

    #[test]
    fn rejects_invalid_bounds_without_source_details() {
        let source = source("<w:p><w:r><w:t>text</w:t></w:r></w:p>");
        let result = paragraphs(DocxDocument::new(&source).unwrap(), None, 0, 0);
        assert_eq!(result.diagnostics[0].code, "INVALID_INSPECTION_BOUNDS");
        let text = format!("{result:?}");
        assert!(!text.contains("NodeId"));
        assert!(!text.contains("SourceSpan"));
    }
}
