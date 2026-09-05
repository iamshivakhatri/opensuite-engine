use opensuite_protocol::{
    Diagnostic, DiagnosticSeverity, FindText, FindTextMatch, FindTextResult, TextContainer,
};

use crate::{BodyBlock, DocxDocument, NodeId, SemanticError, SourceDocument, SourceNodeKind};

const CONTEXT_LIMIT: usize = 64;
const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];

pub fn find_text(
    source: &SourceDocument,
    request: &FindText,
) -> Result<FindTextResult, SemanticError> {
    if request.text.is_empty() {
        return Ok(FindTextResult {
            query: request.text.clone(),
            matches: Vec::new(),
            diagnostics: vec![Diagnostic::new(
                "INVALID_TEXT_QUERY",
                DiagnosticSeverity::Error,
                "text query must not be empty",
            )],
        });
    }
    let matches = resolve_text(source, &request.text)?;
    Ok(FindTextResult {
        query: request.text.clone(),
        matches: matches
            .into_iter()
            .enumerate()
            .map(|(occurrence, item)| FindTextMatch {
                occurrence,
                text: item.text,
                before: item.before,
                after: item.after,
                container: item.container,
            })
            .collect(),
        diagnostics: Vec::new(),
    })
}

pub(crate) struct ResolvedTextMatch {
    pub segments: Vec<ResolvedTextSegment>,
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub before: String,
    pub after: String,
    pub container: TextContainer,
}

#[derive(Clone)]
pub(crate) struct ResolvedTextSegment {
    pub id: NodeId,
    pub start: usize,
    pub end: usize,
    pub source_text: String,
    pub inside_tracked_change: bool,
}

pub(crate) fn resolve_text(
    source: &SourceDocument,
    query: &str,
) -> Result<Vec<ResolvedTextMatch>, SemanticError> {
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let document = DocxDocument::new(source)?;
    let mut matches = Vec::new();
    for block in document.blocks() {
        match block {
            BodyBlock::Paragraph(paragraph) => {
                find_in_container(
                    source,
                    paragraph.source_id(),
                    TextContainer::Paragraph,
                    query,
                    &mut matches,
                )?;
            }
            BodyBlock::Table(table) => {
                for row in table.rows() {
                    for cell in row.cells() {
                        for paragraph in cell.paragraphs() {
                            find_in_container(
                                source,
                                paragraph.source_id(),
                                TextContainer::TableCell,
                                query,
                                &mut matches,
                            )?;
                        }
                    }
                }
            }
        }
    }
    Ok(matches)
}

fn find_in_container(
    source: &SourceDocument,
    container: NodeId,
    kind: TextContainer,
    query: &str,
    matches: &mut Vec<ResolvedTextMatch>,
) -> Result<(), SemanticError> {
    let mut segments = Vec::new();
    collect_current_segments(source, container, false, &mut segments)?;
    let text = segments
        .iter()
        .map(|segment| segment.source_text.as_str())
        .collect::<String>();
    for (start, value) in text.match_indices(query) {
        let end = start + value.len();
        matches.push(ResolvedTextMatch {
            segments: segments
                .iter()
                .filter(|segment| segment.start < end && start < segment.end)
                .cloned()
                .collect(),
            start,
            end,
            text: value.to_owned(),
            before: prefix(&text[..start]),
            after: suffix(&text[end..]),
            container: kind,
        });
    }
    Ok(())
}

fn collect_current_segments(
    source: &SourceDocument,
    id: NodeId,
    inside_tracked_change: bool,
    segments: &mut Vec<ResolvedTextSegment>,
) -> Result<(), SemanticError> {
    if word(source, id, "del") || word(source, id, "moveFrom") {
        return Ok(());
    }
    let inside_tracked_change =
        inside_tracked_change || word(source, id, "ins") || word(source, id, "moveTo");
    if word(source, id, "t") {
        let source_text = crate::semantic::text_value(source, id)?;
        let start = segments.last().map_or(0, |segment| segment.end);
        let end = start + source_text.len();
        segments.push(ResolvedTextSegment {
            id,
            start,
            end,
            source_text,
            inside_tracked_change,
        });
        return Ok(());
    }
    for child in source.children(id) {
        collect_current_segments(source, child, inside_tracked_change, segments)?;
    }
    Ok(())
}

fn prefix(text: &str) -> String {
    let mut value: String = text.chars().rev().take(CONTEXT_LIMIT).collect();
    value = value.chars().rev().collect();
    value
}

fn suffix(text: &str) -> String {
    text.chars().take(CONTEXT_LIMIT).collect()
}

fn word(source: &SourceDocument, id: NodeId, local_name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name, .. }) if name.local_name() == local_name && name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}

#[cfg(test)]
mod tests {
    use opensuite_protocol::FindText;

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    #[test]
    fn finds_current_text_across_runs_without_crossing_paragraphs_or_cells() {
        let source = SourceDocument::parse(format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>FY2026 </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>Revenue </w:t></w:r><w:hyperlink><w:r><w:t>was </w:t></w:r></w:hyperlink><w:sdt><w:sdtContent><w:r><w:t>$10M</w:t></w:r></w:sdtContent></w:sdt></w:p><w:p><w:r><w:t>Revenue </w:t></w:r></w:p><w:p><w:r><w:t>was $10M</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell Revenue was $10M</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Revenue </w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>was $10M</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
        ).into_bytes()).unwrap();
        let result = find_text(
            &source,
            &FindText {
                text: "Revenue was $10M".to_owned(),
            },
        )
        .unwrap();

        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.matches[0].occurrence, 0);
        assert_eq!(result.matches[0].before, "FY2026 ");
        assert_eq!(result.matches[0].container, TextContainer::Paragraph);
        assert_eq!(result.matches[1].container, TextContainer::TableCell);
        assert!(!result.to_json().to_string().contains("NodeId"));
    }

    #[test]
    fn uses_current_revision_content_only() {
        let source = SourceDocument::parse(format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:del><w:r><w:delText>deleted</w:delText></w:r></w:del><w:ins><w:r><w:t>inserted</w:t></w:r></w:ins><w:moveFrom><w:r><w:t>from</w:t></w:r></w:moveFrom><w:moveTo><w:r><w:t>to</w:t></w:r></w:moveTo></w:p></w:body></w:document>"
        ).into_bytes()).unwrap();

        assert_eq!(
            find_text(
                &source,
                &FindText {
                    text: "inserted".to_owned()
                }
            )
            .unwrap()
            .matches
            .len(),
            1
        );
        assert_eq!(
            find_text(
                &source,
                &FindText {
                    text: "to".to_owned()
                }
            )
            .unwrap()
            .matches
            .len(),
            1
        );
        assert!(
            find_text(
                &source,
                &FindText {
                    text: "deleted".to_owned()
                }
            )
            .unwrap()
            .matches
            .is_empty()
        );
        assert!(
            find_text(
                &source,
                &FindText {
                    text: "from".to_owned()
                }
            )
            .unwrap()
            .matches
            .is_empty()
        );
    }

    #[test]
    fn preserves_duplicate_occurrence_order_and_bounded_context() {
        let source = SourceDocument::parse(format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>before needle after needle tail</w:t></w:r></w:p></w:body></w:document>"
        ).into_bytes()).unwrap();
        let result = find_text(
            &source,
            &FindText {
                text: "needle".to_owned(),
            },
        )
        .unwrap();

        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.matches[0].occurrence, 0);
        assert_eq!(result.matches[0].before, "before ");
        assert_eq!(result.matches[0].after, " after needle tail");
        assert_eq!(result.matches[1].occurrence, 1);
        assert_eq!(result.matches[1].before, "before needle after ");
    }
}
