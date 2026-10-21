//! Bounded main-document bookmark and hyperlink inspection; never rewrites XML.
use crate::{NodeId, SourceDocument, SourceNodeKind};
use opensuite_opc::Package;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceInspection {
    pub ok: bool,
    pub total: usize,
    pub offset: usize,
    pub has_more: bool,
    pub references: Vec<ReferenceSnapshot>,
    pub diagnostics: Vec<crate::StyleInspectionDiagnostic>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSnapshot {
    pub index: usize,
    pub kind: &'static str,
    pub id: Option<String>,
    pub name: Option<String>,
    pub part_name: String,
    pub paragraph_index: Option<usize>,
    pub end_paragraph_index: Option<usize>,
    pub body_block_handle: Option<String>,
    pub container: &'static str,
    pub target_kind: Option<&'static str>,
    pub target: Option<String>,
    pub text: Option<String>,
    pub text_target: Option<ReferenceTextTarget>,
    /// Exact usable destination for `set_hyperlink.bookmark_name`.
    pub bookmark_name: Option<String>,
    pub truncated: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReferenceTextTarget {
    pub text: String,
    pub occurrence: usize,
}

/// Default 20/max 100 source-ordered records, strings capped at 2,000 characters.
/// Malformed bookmark markers remain visible, but cannot supply mutation destinations.
pub fn inspect_docx_references(input: Vec<u8>, offset: usize, limit: usize) -> ReferenceInspection {
    let mut result = ReferenceInspection {
        ok: true,
        total: 0,
        offset,
        has_more: false,
        references: Vec::new(),
        diagnostics: Vec::new(),
    };
    let read = (|| {
        let package = Package::from_bytes(input).map_err(|e| e.code().to_owned())?;
        let (main, source) = crate::open_main_source(&package).map_err(|e| e.code().to_owned())?;
        crate::DocxDocument::new(&source).map_err(|e| e.code().to_owned())?;
        Ok::<_, String>((package, main, source))
    })();
    let (package, main, source) = match read {
        Ok(value) => value,
        Err(code) => {
            result.ok = false;
            result.diagnostics.push(crate::StyleInspectionDiagnostic {
                code,
                message: "could not inspect document references".into(),
            });
            return result;
        }
    };
    let bookmarks = crate::references::bookmarks(&source);
    let validate_destination = |name: &str| match &bookmarks {
        Ok(bookmarks) => crate::mutation::validate_parsed_bookmark_destination(bookmarks, name),
        Err(error) => Err(opensuite_protocol::OperationResult::failed(
            error.code(),
            error.to_string(),
        )),
    };
    if let Err(error) = &bookmarks {
        result.diagnostics.push(crate::StyleInspectionDiagnostic {
            code: error.code().into(),
            message: "bookmark markers are malformed; destinations are unavailable".into(),
        });
    }
    let paragraphs = source
        .node_ids()
        .filter(|id| word(&source, *id, "p"))
        .collect::<Vec<_>>();
    let document = crate::DocxDocument::new(&source).unwrap();
    let body = document.body_id();
    let blocks = source
        .children(body)
        .filter(|id| word(&source, *id, "p") || word(&source, *id, "tbl"))
        .collect::<Vec<_>>();
    let paragraph = |id| {
        paragraphs.iter().position(|p| {
            let outer = source.node(*p).unwrap().span();
            let inner = source.node(id).unwrap().span();
            outer.start <= inner.start && outer.end >= inner.end
        })
    };
    let start_ids = source
        .node_ids()
        .filter(|id| word(&source, *id, "bookmarkStart"))
        .filter_map(|id| source.node(id).unwrap().attribute("id"))
        .collect::<std::collections::HashSet<_>>();
    let records = source
        .node_ids()
        .filter(|id| {
            word(&source, *id, "bookmarkStart")
                || word(&source, *id, "hyperlink")
                || (word(&source, *id, "bookmarkEnd")
                    && !source
                        .node(*id)
                        .unwrap()
                        .attribute("id")
                        .is_some_and(|id| start_ids.contains(id)))
        })
        .collect::<Vec<_>>();
    result.total = records.len();
    for (index, id) in records
        .into_iter()
        .enumerate()
        .skip(offset)
        .take(limit.clamp(1, 100))
    {
        let node = source.node(id).unwrap();
        let is_link = word(&source, id, "hyperlink");
        let mut item = ReferenceSnapshot {
            index,
            kind: if is_link { "hyperlink" } else { "bookmark" },
            id: node.attribute("id").map(str::to_owned),
            name: node.attribute("name").map(str::to_owned),
            part_name: main.name.as_str().into(),
            paragraph_index: paragraph(id),
            end_paragraph_index: None,
            body_block_handle: None,
            container: "body",
            target_kind: None,
            target: None,
            text: None,
            text_target: None,
            bookmark_name: None,
            truncated: false,
            reasons: Vec::new(),
        };
        let mut parent = node.parent();
        while let Some(id) = parent {
            if word(&source, id, "tc") {
                item.container = "tableCell";
                break;
            }
            if word(&source, id, "sdt") {
                item.container = "contentControl";
                break;
            }
            parent = source.node(id).and_then(|n| n.parent());
        }
        if let Some(p) = item.paragraph_index.and_then(|index| paragraphs.get(index)) {
            item.body_block_handle = blocks
                .iter()
                .position(|id| id == p)
                .map(|index| format!("b{index}"));
        }
        if is_link {
            let link = crate::references::Hyperlink {
                source: &source,
                source_id: id,
            };
            if link.anchor().is_some() && link.relationship_id().is_some() {
                item.reasons.push("MIXED_HYPERLINK_TARGET".into());
            }
            match link.target(&package, &main) {
                Ok(Some(crate::HyperlinkTarget::InternalAnchor(name))) => {
                    item.target_kind = Some("bookmark");
                    if let Err(error) = validate_destination(&name) {
                        item.reasons
                            .extend(error.diagnostics.iter().take(5).map(|d| d.code.clone()));
                    }
                    item.target = Some(name);
                }
                Ok(Some(crate::HyperlinkTarget::External(url))) => {
                    item.target_kind = Some("external");
                    item.target = Some(url);
                }
                Ok(Some(crate::HyperlinkTarget::InternalPart(part))) => {
                    item.target_kind = Some("packagePart");
                    item.target = Some(part.name.as_str().into());
                    item.reasons.push("UNSUPPORTED_PART_HYPERLINK".into());
                }
                Ok(None) => item.reasons.push("MISSING_HYPERLINK_TARGET".into()),
                Err(error) => item.reasons.push(error.code().into()),
            }
            if !source.children(id).all(|child| word(&source, child, "r")) {
                item.reasons.push("UNSUPPORTED_HYPERLINK_CONTENT".into());
            }
            match link.text() {
                Ok(text) => {
                    if !text.is_empty()
                        && text.chars().count() <= 2000
                        && !item
                            .reasons
                            .iter()
                            .any(|r| r == "UNSUPPORTED_HYPERLINK_CONTENT")
                    {
                        if let Ok(matches) = crate::text_search::resolve_text(&source, &text) {
                            if let Some(occurrence) = matches.iter().position(|m| {
                                m.segments.iter().all(|s| {
                                    let span = source.node(s.id).unwrap().span();
                                    span.start >= node.span().start && span.end <= node.span().end
                                })
                            }) {
                                item.text_target = Some(ReferenceTextTarget {
                                    text: text.clone(),
                                    occurrence,
                                });
                            }
                        }
                    }
                    item.text = Some(text);
                }
                Err(error) => item.reasons.push(error.code().into()),
            }
        } else if let Ok(bookmarks) = &bookmarks {
            if let Some(bookmark) = bookmarks.iter().find(|b| b.start_node_id == id) {
                item.end_paragraph_index = paragraph(bookmark.end_node_id);
                match validate_destination(&bookmark.name) {
                    Ok(()) => item.bookmark_name = Some(bookmark.name.clone()),
                    Err(error) => item
                        .reasons
                        .extend(error.diagnostics.iter().take(5).map(|d| d.code.clone())),
                }
            }
        } else if let Err(error) = &bookmarks {
            item.reasons.push(error.code().into());
        }
        for value in [
            &mut item.id,
            &mut item.name,
            &mut item.target,
            &mut item.text,
        ]
        .into_iter()
        .flatten()
        {
            if value.chars().count() > 2000 {
                *value = value.chars().take(2000).collect();
                item.truncated = true;
            }
        }
        if item.truncated {
            item.reasons.push("INSPECTION_VALUE_TRUNCATED".into());
        }
        result.references.push(item);
    }
    result.has_more = offset.saturating_add(result.references.len()) < result.total;
    result
}

fn word(source: &SourceDocument, id: NodeId, local: &str) -> bool {
    matches!(source.node(id).map(|n| n.kind()), Some(SourceNodeKind::Element { name, .. }) if name.local_name() == local && matches!(name.namespace_uri(), Some("http://schemas.openxmlformats.org/wordprocessingml/2006/main" | "http://purl.oclc.org/ooxml/wordprocessingml/main")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(body: &str) -> Vec<u8> {
        let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
        let (main, _) = crate::open_main_source(&package).unwrap();
        package.write_replaced_part_to_vec(&main,format!("<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}</w:body></w:document>").as_bytes()).unwrap()
    }

    #[test]
    fn imported_references_supply_locations_targets_and_paging_without_source_changes() {
        let bytes = include_bytes!("../../../tests/fixtures/references.docx").to_vec();
        let result = inspect_docx_references(bytes.clone(), 0, 100);
        assert!(result.ok);
        assert_eq!(result.total, 4);
        let links = result
            .references
            .iter()
            .filter(|r| r.kind == "hyperlink")
            .collect::<Vec<_>>();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target_kind, Some("external"));
        assert_eq!(links[0].text.as_deref(), Some("OpenSuite"));
        assert_eq!(links[0].text_target.as_ref().unwrap().text, "OpenSuite");
        assert_eq!(links[1].target.as_deref(), Some("RevenueSection"));
        let table_mark = result
            .references
            .iter()
            .find(|r| r.name.as_deref() == Some("TableCell"))
            .unwrap();
        assert_eq!(table_mark.container, "tableCell");
        assert!(table_mark.paragraph_index.is_some());
        assert_eq!(table_mark.bookmark_name.as_deref(), Some("TableCell"));
        let page = inspect_docx_references(bytes.clone(), 1, 1);
        assert_eq!(page.references.len(), 1);
        assert!(page.has_more);
        assert_eq!(page.references[0].index, 1);
        assert!(
            inspect_docx_references(bytes.clone(), usize::MAX, 20)
                .references
                .is_empty()
        );
        assert_eq!(
            inspect_docx_references(bytes, 0, 100).references[0].kind,
            result.references[0].kind
        );
    }

    #[test]
    fn malformed_duplicate_missing_and_richer_references_are_diagnosed() {
        let bytes = document(
            "<w:p><w:bookmarkStart w:id='1' w:name='Same'/><w:bookmarkEnd w:id='1'/><w:bookmarkStart w:id='2' w:name='Same'/><w:bookmarkEnd w:id='2'/><w:hyperlink w:anchor='Missing'><w:r><w:t>Link</w:t></w:r></w:hyperlink><w:hyperlink/><w:hyperlink w:anchor='Same'><w:sdt/></w:hyperlink></w:p>",
        );
        let result = inspect_docx_references(bytes, 0, 100);
        assert_eq!(result.total, 5);
        assert!(
            result.references[0]
                .reasons
                .iter()
                .any(|r| r == "BOOKMARK_AMBIGUOUS")
        );
        assert!(result.references[0].bookmark_name.is_none());
        assert!(
            result.references[2]
                .reasons
                .iter()
                .any(|r| r == "BOOKMARK_NOT_FOUND")
        );
        assert!(
            result.references[3]
                .reasons
                .iter()
                .any(|r| r == "MISSING_HYPERLINK_RELATIONSHIP_ID")
        );
        assert!(
            result.references[4]
                .reasons
                .iter()
                .any(|r| r == "UNSUPPORTED_HYPERLINK_CONTENT")
        );
        let result = inspect_docx_references(
            document(
                "<w:p><w:bookmarkEnd w:id='7'/><w:bookmarkStart w:id='bad' w:name='Broken'/></w:p>",
            ),
            0,
            20,
        );
        assert!(result.ok);
        assert_eq!(result.total, 2);
        assert!(!result.diagnostics.is_empty());
        assert!(
            result
                .references
                .iter()
                .all(|r| r.bookmark_name.is_none() && !r.reasons.is_empty())
        );
    }

    #[test]
    fn bounds_strings_record_count_and_invalid_package_output() {
        let links = format!(
            "<w:hyperlink w:anchor='Missing'><w:r><w:t>{}</w:t></w:r></w:hyperlink>",
            "a".repeat(2100)
        )
        .repeat(120);
        let result =
            inspect_docx_references(document(&format!("<w:p>{links}</w:p>")), 0, usize::MAX);
        assert_eq!(result.total, 120);
        assert_eq!(result.references.len(), 100);
        assert!(result.has_more);
        assert!(result.references.iter().all(|r| r.truncated
            && r.text.as_ref().unwrap().len() == 2000
            && r.text_target.is_none()));
        let bad = inspect_docx_references(vec![1, 2, 3], 0, 20);
        assert!(!bad.ok);
        assert_eq!(bad.diagnostics.len(), 1);
    }

    #[test]
    fn libreoffice_produced_bookmark_and_links_remain_inspectable() {
        let result = inspect_docx_references(
            include_bytes!("../../../tests/fixtures/libreoffice-references.docx").to_vec(),
            0,
            20,
        );
        assert!(result.ok);
        assert_eq!(result.total, 3);
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.references[0].target_kind, Some("bookmark"));
        assert_eq!(result.references[1].target_kind, Some("external"));
        assert_eq!(
            result.references[2].bookmark_name.as_deref(),
            Some("Findings")
        );
    }
}
