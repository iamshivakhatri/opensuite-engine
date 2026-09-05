use opensuite_protocol::{
    InspectTextContext, InspectTextContextResult, TextContextContainer, TextTarget,
};

use crate::{SemanticError, SourceDocument};

const CONTEXT_LIMIT: usize = 10;

/// Inspects the full Current-view paragraph containing a text target and nearby paragraphs.
pub fn inspect_text_context(
    source: &SourceDocument,
    request: &InspectTextContext,
) -> Result<InspectTextContextResult, SemanticError> {
    if request.before > CONTEXT_LIMIT || request.after > CONTEXT_LIMIT {
        return Ok(InspectTextContextResult::failed(
            request.target.clone(),
            "INVALID_CONTEXT_RANGE",
            "nearby context before and after counts must not exceed 10",
        ));
    }
    let (containers, matches) =
        crate::text_search::resolve_text_with_containers(source, &request.target.text)?;
    let match_count = matches.len();
    let Some(matched) = select_match(matches, &request.target) else {
        let (code, message) = if match_count > 1 && request.target.occurrence.is_none() {
            (
                "TARGET_AMBIGUOUS",
                "text target matches more than one current semantic range",
            )
        } else {
            ("TARGET_NOT_FOUND", "text target was not found")
        };
        return Ok(InspectTextContextResult::failed(
            request.target.clone(),
            code,
            message,
        ));
    };
    let start = matched.container_index.saturating_sub(request.before);
    let end = (matched.container_index + request.after + 1).min(containers.len());
    let nearby = containers[start..end]
        .iter()
        .enumerate()
        .map(|(offset, item)| TextContextContainer {
            relative_position: offset as isize + start as isize - matched.container_index as isize,
            text: item.text.clone(),
            container: item.container,
        })
        .collect::<Vec<_>>();
    let container = nearby
        .iter()
        .find(|item| item.relative_position == 0)
        .expect("target container is nearby")
        .clone();
    Ok(InspectTextContextResult::found(
        request.target.clone(),
        container,
        nearby,
    ))
}

fn select_match(
    matches: Vec<crate::text_search::ResolvedTextMatch>,
    target: &TextTarget,
) -> Option<crate::text_search::ResolvedTextMatch> {
    match target.occurrence {
        Some(occurrence) => matches.into_iter().nth(occurrence),
        None if matches.len() == 1 => matches.into_iter().next(),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use opensuite_protocol::{InspectTextContext, TextContainer, TextTarget};

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    fn inspect(xml: &str, text: &str, occurrence: Option<usize>) -> InspectTextContextResult {
        let source = SourceDocument::parse(xml.as_bytes().to_vec()).unwrap();
        inspect_text_context(
            &source,
            &InspectTextContext {
                target: TextTarget {
                    text: text.to_owned(),
                    occurrence,
                },
                before: 1,
                after: 1,
            },
        )
        .unwrap()
    }

    #[test]
    fn returns_full_container_and_nearby_source_order() {
        let result = inspect(
            &format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>first</w:t></w:r></w:p><w:p><w:r><w:t>before </w:t></w:r><w:hyperlink><w:r><w:t>needle</w:t></w:r></w:hyperlink><w:sdt><w:sdtContent><w:r><w:t> after</w:t></w:r></w:sdtContent></w:sdt></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
            ),
            "needle",
            None,
        );
        assert_eq!(
            result.container.as_ref().unwrap().text,
            "before needle after"
        );
        assert_eq!(
            result
                .nearby
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>(),
            ["first", "before needle after", "cell"]
        );
        assert_eq!(result.nearby[2].container, TextContainer::TableCell);
        assert!(!result.to_json().to_string().contains("NodeId"));
    }

    #[test]
    fn resolves_occurrences_and_current_view_only() {
        let xml = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>needle</w:t></w:r></w:p><w:p><w:del><w:r><w:delText>needle</w:delText></w:r></w:del><w:ins><w:r><w:t>needle</w:t></w:r></w:ins></w:p></w:body></w:document>"
        );
        assert_eq!(
            inspect(&xml, "needle", None).diagnostics[0].code,
            "TARGET_AMBIGUOUS"
        );
        assert_eq!(
            inspect(&xml, "needle", Some(1)).container.unwrap().text,
            "needle"
        );
        assert_eq!(
            inspect(&xml, "missing", None).diagnostics[0].code,
            "TARGET_NOT_FOUND"
        );
    }

    #[test]
    fn rejects_excessive_nearby_counts() {
        let source = SourceDocument::parse(format!("<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>needle</w:t></w:r></w:p></w:body></w:document>").into_bytes()).unwrap();
        let result = inspect_text_context(
            &source,
            &InspectTextContext {
                target: TextTarget {
                    text: "needle".to_owned(),
                    occurrence: None,
                },
                before: 11,
                after: 0,
            },
        )
        .unwrap();
        assert_eq!(result.diagnostics[0].code, "INVALID_CONTEXT_RANGE");
    }
}
