//! Standard Word comments. Only selected boundary runs are split; package parts stay source-backed.
use super::*;
use crate::comment::{CONTENT_TYPE, CommentData, CommentSet, comment_stamp};
use opensuite_protocol::{AddComment, DeleteComment, UpdateComment};

fn load_comments(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
) -> Result<CommentSet, OperationResult> {
    let comments = crate::comment::load(package, main, source)
        .map_err(|e| comment_failure(e.code(), e.to_string()))?;
    if comments.errors().next().is_some() || comments.comments.iter().any(|c| c.reference.is_none())
    {
        return Err(comment_failure(
            "MALFORMED_COMMENT_STRUCTURE",
            "repair orphaned or malformed comments before editing",
        ));
    }
    // Threaded metadata points to comment paragraph identities. Do not invalidate it.
    if comments.has_threaded_metadata {
        return Err(unsupported(
            "editing threaded-comment metadata is not supported",
        ));
    }
    let mut numeric_ids = HashSet::new();
    for comment in &comments.comments {
        if comment
            .id
            .as_ref()
            .is_none_or(|id| id.parse::<i32>().is_err())
        {
            return Err(comment_failure(
                "MALFORMED_COMMENT_STRUCTURE",
                "comment IDs must be signed 32-bit integers",
            ));
        }
        if !numeric_ids.insert(comment.id.as_ref().unwrap().parse::<i32>().unwrap()) {
            return Err(comment_failure(
                "COMMENT_ID_CONFLICT",
                "comment IDs have duplicate numeric values",
            ));
        }
        for marker in [comment.range_start, comment.range_end]
            .into_iter()
            .flatten()
        {
            if !source
                .node(marker)
                .and_then(|n| n.parent())
                .is_some_and(|p| word(source, p, "p"))
            {
                return Err(unsupported(
                    "editing comments with range markers outside paragraphs is unsupported",
                )
                .with_reason_code("UNSUPPORTED_COMMENT_RANGE"));
            }
        }
    }
    Ok(comments)
}

fn checked_comment<'a>(
    source: &SourceDocument,
    comments: &'a CommentSet,
    handle: &str,
) -> Result<&'a CommentData, OperationResult> {
    let mut pieces = handle.splitn(3, ':');
    if pieces.next() != Some("comment")
        || pieces.next() != Some(comment_stamp(source, comments).as_str())
    {
        return Err(comment_failure(
            "PRECONDITION_FAILED",
            "comment handle is stale or malformed; inspect comments again",
        ));
    }
    let id = pieces
        .next()
        .ok_or_else(|| comment_failure("PRECONDITION_FAILED", "invalid comment handle"))?;
    comments
        .comments
        .iter()
        .find(|c| c.id.as_deref() == Some(id))
        .ok_or_else(|| comment_failure("COMMENT_NOT_FOUND", "comment does not exist"))
}

fn plain_text(text: &str, namespace: &str) -> Result<String, OperationResult> {
    if text.trim().is_empty()
        || text.len() > 32_000
        || text.chars().any(|c| {
            (c.is_control() && c != '\n' && c != '\t') || matches!(c, '\u{fffe}' | '\u{ffff}')
        })
    {
        return Err(comment_failure(
            "INVALID_OPERATION",
            "comment text must be nonempty, valid plain text, and at most 32000 bytes",
        ));
    }
    // A local namespace keeps imported prefixes and default namespaces untouched.
    use std::fmt::Write as _;
    let mut xml = String::new();
    for line in text.split('\n') {
        write!(
            xml,
            r#"<w:p xmlns:w="{namespace}"><w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
            escape(line)
        )
        .expect("writing to a string");
    }
    Ok(xml)
}

pub fn add_comment_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &AddComment,
) -> Result<Vec<u8>, OperationResult> {
    let comments = load_comments(package, main, source)?;
    if operation.author.trim().is_empty()
        || [&operation.author, &operation.date]
            .into_iter()
            .chain(operation.initials.iter())
            .any(|v| {
                v.len() > 1024
                    || v.chars()
                        .any(|c| c.is_control() || matches!(c, '\u{fffe}' | '\u{ffff}'))
            })
        || !valid_review_date(&operation.date)
    {
        return Err(comment_failure(
            "INVALID_OPERATION",
            "provide a nonempty author and ISO date without control characters",
        ));
    }
    let (matched, paragraph) =
        text::simple_body_text_range(source, &operation.target, "UNSUPPORTED_COMMENT_RANGE")?;
    let runs: Vec<_> = matched
        .segments
        .iter()
        .map(|segment| {
            (
                ordinary_run(source, segment.id).unwrap(),
                matched.start.max(segment.start) - segment.start,
                matched.end.min(segment.end) - segment.start,
                segment.source_text.as_str(),
            )
        })
        .collect();
    let max = comments
        .comments
        .iter()
        .filter_map(|c| c.id.as_ref()?.parse::<i32>().ok())
        .max();
    let id = max
        .map_or(Some(0), |n| n.max(-1).checked_add(1))
        .ok_or_else(|| comment_failure("COMMENT_ID_CONFLICT", "no comment ID available"))?
        .to_string();
    let namespace = source
        .node(paragraph)
        .and_then(|n| match n.kind() {
            SourceNodeKind::Element { name, .. } => name.namespace_uri(),
            _ => None,
        })
        .unwrap();
    let start = format!(r#"<w:commentRangeStart xmlns:w="{namespace}" w:id="{id}"/>"#);
    let end = format!(
        r#"<w:commentRangeEnd xmlns:w="{namespace}" w:id="{id}"/><w:r xmlns:w="{namespace}"><w:commentReference w:id="{id}"/></w:r>"#
    );
    let mut patches = Vec::new();
    for (index, (run, a, b, text)) in runs.iter().enumerate() {
        let span = source.node(*run).unwrap().span();
        let original = String::from_utf8_lossy(&source.original_bytes()[span.start..span.end]);
        if *a == 0 && *b == text.len() {
            // Keep complete runs byte-for-byte, even unfamiliar run properties.
            patches.push(Patch {
                span,
                replacement: format!(
                    "{}{}{}",
                    if index == 0 { &start } else { "" },
                    original,
                    if index + 1 == runs.len() { &end } else { "" }
                )
                .into_bytes(),
            });
        } else {
            let (before, selected, after) =
                text::text_run_pieces(source, *run, text, *a, *b, false)?;
            patches.push(Patch {
                span,
                replacement: format!(
                    "{before}{}{selected}{}{after}",
                    if index == 0 { &start } else { "" },
                    if index + 1 == runs.len() { &end } else { "" }
                )
                .into_bytes(),
            });
        }
    }
    let body = plain_text(&operation.text, namespace)?;
    let initials = operation
        .initials
        .as_ref()
        .map(|v| format!(r#" w:initials="{}""#, escape(v)))
        .unwrap_or_default();
    let record = format!(
        r#"<w:comment xmlns:w="{namespace}" w:id="{id}" w:author="{}" w:date="{}"{initials}>{body}</w:comment>"#,
        escape(&operation.author),
        escape(&operation.date)
    );
    let document = apply_patches(source, patches)?;
    let mut replaced = vec![(main.name.clone(), document.as_slice())];
    let mut added = Vec::new();
    let bytes;
    let rels_bytes;
    let types_bytes;
    if let Some(part) = &comments.part {
        let comment_source = comments.source.as_ref().unwrap();
        let insertion =
            element_insertion(comment_source, comment_source.root(), record.into_bytes())?;
        bytes = apply_patches(comment_source, vec![insertion])?;
        replaced.push((part.name.clone(), bytes.as_slice()));
    } else {
        let directory = main.name.as_str().rsplit_once('/').unwrap().0;
        let part_name =
            PartName::parse(format!("{directory}/comments.xml")).map_err(document_invalid)?;
        if package.read_part_by_name(&part_name).is_ok() {
            return Err(comment_failure(
                "PACKAGE_CONFLICT",
                "unlinked comments part already exists",
            ));
        }
        bytes = format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:comments xmlns:w="{namespace}">{record}</w:comments>"#).into_bytes();
        added.push((part_name.clone(), bytes.as_slice()));
        let (rels_name, exists, rels, rel_id) = picture_relationships(package, main)?;
        let rel_type = if namespace == NS[1] {
            crate::comment::REL[1]
        } else {
            crate::comment::REL[0]
        };
        rels_bytes = append_xml_element(
            &rels,
            &format!(r#"<Relationship Id="{rel_id}" Type="{rel_type}" Target="comments.xml"/>"#),
        )?;
        if exists {
            replaced.push((rels_name, rels_bytes.as_slice()));
        } else {
            added.push((rels_name, rels_bytes.as_slice()));
        }
        let types_name = PartName::parse("/[Content_Types].xml").map_err(document_invalid)?;
        let types = package
            .read_part_by_name(&types_name)
            .map_err(document_invalid)?;
        let default = if package.content_type_default("rels").is_none() {
            r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#
        } else {
            ""
        };
        types_bytes = append_xml_element(
            &types,
            &format!(
                r#"{default}<Override PartName="{}" ContentType="{CONTENT_TYPE}"/>"#,
                part_name.as_str()
            ),
        )?;
        replaced.push((types_name, types_bytes.as_slice()));
    }
    let output = package
        .write_package_with_named_changes_to_vec(&replaced, &added)
        .map_err(document_invalid)?;
    verify_comments(
        output,
        source,
        &id,
        Some(&operation.text),
        Some(&matched.text),
    )
}

pub fn update_comment_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &UpdateComment,
) -> Result<Vec<u8>, OperationResult> {
    let comments = load_comments(package, main, source)?;
    let comment = checked_comment(source, &comments, &operation.handle)?;
    let comment_source = comments.source.as_ref().unwrap();
    let node = comment_source.node(comment.source_id).unwrap();
    let SourceNodeKind::Element {
        name,
        start_tag,
        end_tag: Some(end_tag),
        ..
    } = node.kind()
    else {
        return Err(unsupported("comment has no editable content"));
    };
    // Preserve the exact comment tag and metadata. Rich imported content is not silently flattened.
    if comment_source.children(comment.source_id).any(|id| {
        !word(comment_source, id, "p")
            && !matches!(
                comment_source.node(id).unwrap().kind(),
                SourceNodeKind::Text
            )
    }) || comment_source.node_ids().any(|id| {
        let span = comment_source.node(id).unwrap().span();
        span.start >= start_tag.end
            && span.end <= end_tag.start
            && matches!(
                comment_source.node(id).unwrap().kind(),
                SourceNodeKind::Element { .. }
            )
            && !{
                let mut parent = comment_source.node(id).and_then(|n| n.parent());
                let mut property = false;
                while let Some(node) = parent {
                    if word(comment_source, node, "pPr") || word(comment_source, node, "rPr") {
                        property = true;
                        break;
                    }
                    parent = comment_source.node(node).and_then(|n| n.parent());
                }
                property
            }
            && !["p", "pPr", "r", "rPr", "t", "annotationRef"]
                .iter()
                .any(|local| word(comment_source, id, local))
    }) {
        return Err(unsupported(
            "only plain paragraph comment content can be replaced",
        ));
    }
    let body = plain_text(&operation.text, name.namespace_uri().unwrap())?;
    let patched = apply_patches(
        comment_source,
        vec![Patch {
            span: SourceSpan {
                start: start_tag.end,
                end: end_tag.start,
            },
            replacement: body.into_bytes(),
        }],
    )?;
    let output = package
        .write_replaced_part_to_vec(comments.part.as_ref().unwrap(), &patched)
        .map_err(document_invalid)?;
    verify_comments(
        output,
        source,
        comment.id.as_deref().unwrap(),
        Some(&operation.text),
        None,
    )
}

pub fn delete_comment_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &DeleteComment,
) -> Result<Vec<u8>, OperationResult> {
    let comments = load_comments(package, main, source)?;
    let comment = checked_comment(source, &comments, &operation.handle)?;
    let mut patches = Vec::new();
    for id in [comment.range_start, comment.range_end, comment.reference]
        .into_iter()
        .flatten()
    {
        let mut ancestor = source.node(id).and_then(|n| n.parent());
        while let Some(node) = ancestor {
            if ["ins", "del", "moveFrom", "moveTo"]
                .iter()
                .any(|name| word(source, node, name))
            {
                return Err(unsupported("comment marker is inside a revision"));
            }
            ancestor = source.node(node).and_then(|n| n.parent());
        }
        if has_revision_wrapper(source, id) {
            return Err(unsupported("comment marker is inside a revision"));
        }
        patches.push(Patch {
            span: source.node(id).unwrap().span(),
            replacement: Vec::new(),
        });
    }
    let document = apply_patches(source, patches)?;
    let comment_source = comments.source.as_ref().unwrap();
    let part = comments.part.as_ref().unwrap();
    let bytes = apply_patches(
        comment_source,
        vec![Patch {
            span: comment_source.node(comment.source_id).unwrap().span(),
            replacement: Vec::new(),
        }],
    )?;
    // Keep the valid empty part and any now-unused relationship.
    let output = package
        .write_package_with_named_changes_to_vec(
            &[(main.name.clone(), &document), (part.name.clone(), &bytes)],
            &[],
        )
        .map_err(document_invalid)?;
    verify_comments(output, source, comment.id.as_deref().unwrap(), None, None)
}

fn verify_comments(
    output: Vec<u8>,
    before: &SourceDocument,
    id: &str,
    text: Option<&str>,
    anchor: Option<&str>,
) -> Result<Vec<u8>, OperationResult> {
    let package = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let comments = load_comments(&package, &main, &source)?;
    let original = body_texts(before)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    let current = body_texts(&source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    let found = comments.comments().find(|c| c.id() == Some(id));
    if original != current
        || found
            .as_ref()
            .map(|c| c.text())
            .transpose()
            .map_err(document_invalid)?
            .as_deref()
            != text
    {
        return Err(comment_failure(
            "DOCUMENT_INVALID",
            "comment postcondition or document text preservation failed",
        ));
    }
    if let Some(anchor) = anchor {
        if found
            .as_ref()
            .unwrap()
            .anchored_text(&source)
            .map_err(document_invalid)?
            .as_deref()
            != Some(anchor)
        {
            return Err(comment_failure(
                "DOCUMENT_INVALID",
                "comment anchor does not match selection",
            ));
        }
    }
    Ok(output)
}

fn comment_failure(code: &str, message: impl Into<String>) -> OperationResult {
    OperationResult::failed(code, message).with_reason_code(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        execute_docx_add_comment, execute_docx_delete_comment, execute_docx_update_comment,
        inspect_docx_comments,
    };

    fn fixture(paragraphs: &str) -> Vec<u8> {
        let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
        let main = package.main_office_document().unwrap();
        let xml = format!(
            r#"<w:document xmlns:w="{}"><w:body>{paragraphs}</w:body></w:document>"#,
            NS[0]
        );
        package
            .write_package_with_named_changes_to_vec(
                &[(main.name, xml.as_bytes())],
                &[(
                    PartName::parse("/word/custom.xml").unwrap(),
                    b"<unknown keep='exactly'/>",
                )],
            )
            .unwrap()
    }
    fn add(bytes: Vec<u8>, target: &str, text: &str) -> Vec<u8> {
        execute_docx_add_comment(
            bytes,
            &AddComment {
                target: TextTarget {
                    text: target.into(),
                    occurrence: None,
                },
                text: text.into(),
                author: "Reviewer & author".into(),
                initials: Some("RA".into()),
                date: "2026-10-04T12:00:00Z".into(),
            },
        )
        .output_artifact
        .unwrap()
    }
    fn part(bytes: &[u8], name: &str) -> Vec<u8> {
        Package::from_bytes(bytes.to_vec())
            .unwrap()
            .read_part_by_name(&PartName::parse(name).unwrap())
            .unwrap()
    }
    fn record(bytes: &[u8], id: &str) -> Vec<u8> {
        let package = Package::from_bytes(bytes.to_vec()).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let set = crate::comment::load(&package, &main, &source).unwrap();
        let node = set
            .comments
            .iter()
            .find(|c| c.id.as_deref() == Some(id))
            .unwrap()
            .source_id;
        let source = set.source.unwrap();
        let span = source.node(node).unwrap().span();
        source.original_bytes()[span.start..span.end].to_vec()
    }

    #[test]
    fn first_comment_round_trip_preserves_multi_run_text_and_deletes_only_markers() {
        let before = fixture(&format!(
            r#"<w:p><w:pPr><w:keepNext/></w:pPr><x:r xmlns:x="{}"><x:rPr><x:b/></x:rPr><x:t xml:space="preserve" data-keep="yes">Revenue increased 1</x:t></x:r><w:r><w:rPr><w:i/></w:rPr><w:t>2% in Q3.</w:t></w:r></w:p>"#,
            NS[0]
        ));
        let bytes = add(before.clone(), "12%", "Confirm & check\nSecond line");
        let inspection = inspect_docx_comments(bytes.clone(), 0, 20);
        assert_eq!(inspection.comments[0].anchored_text.as_deref(), Some("12%"));
        assert_eq!(
            inspection.comments[0].author.as_deref(),
            Some("Reviewer & author")
        );
        assert_eq!(inspection.comments[0].paragraph_index, Some(0));
        assert!(inspection.diagnostics.is_empty());
        let main = String::from_utf8(part(&bytes, "/word/document.xml")).unwrap();
        assert!(main.contains("<x:rPr><x:b/></x:rPr>"));
        assert!(main.contains("data-keep=\"yes\""));
        assert!(main.contains("<w:rPr><w:i/></w:rPr>"));
        assert_eq!(
            part(&before, "/word/styles.xml"),
            part(&bytes, "/word/styles.xml")
        );
        assert_eq!(
            part(&before, "/word/custom.xml"),
            part(&bytes, "/word/custom.xml")
        );
        let handle = inspection.comments[0].handle.clone().unwrap();
        let changed = execute_docx_update_comment(
            bytes.clone(),
            &UpdateComment {
                handle: handle.clone(),
                text: "Updated".into(),
            },
        )
        .output_artifact
        .unwrap();
        assert_eq!(
            part(&bytes, "/word/document.xml"),
            part(&changed, "/word/document.xml")
        );
        assert!(
            execute_docx_delete_comment(changed.clone(), &DeleteComment { handle })
                .output_artifact
                .is_none()
        );
        let handle = inspect_docx_comments(changed.clone(), 0, 20).comments[0]
            .handle
            .clone()
            .unwrap();
        let deleted = execute_docx_delete_comment(changed, &DeleteComment { handle })
            .output_artifact
            .unwrap();
        assert_eq!(inspect_docx_comments(deleted.clone(), 0, 20).total, 0);
        assert!(
            !String::from_utf8(part(&deleted, "/word/document.xml"))
                .unwrap()
                .contains("comment")
        );
        assert_eq!(
            part(&deleted, "/word/custom.xml"),
            part(&before, "/word/custom.xml")
        );
    }

    #[test]
    fn imported_unrelated_comments_keep_exact_records_and_ids() {
        let mut bytes = fixture("<w:p><w:r><w:t>First Second Third</w:t></w:r></w:p>");
        for target in ["First", "Second", "Third"] {
            bytes = add(bytes, target, target);
        }
        // Imported producer metadata and standard comment-number/style markup.
        let package = Package::from_bytes(bytes.clone()).unwrap();
        let name = PartName::parse("/word/comments.xml").unwrap();
        let original = String::from_utf8(part(&bytes, "/word/comments.xml"))
            .unwrap()
            .replace("w:author=", "data-producer='preserve' w:author=")
            .replace(
                "<w:r><w:t",
                "<w:r><w:rPr><w:rStyle w:val=\"CommentText\"/></w:rPr><w:t",
            );
        bytes = package
            .write_package_with_named_changes_to_vec(&[(name, original.as_bytes())], &[])
            .unwrap();
        let first = record(&bytes, "0");
        let third = record(&bytes, "2");
        let handle = inspect_docx_comments(bytes.clone(), 0, 20).comments[1]
            .handle
            .clone()
            .unwrap();
        let edited = execute_docx_update_comment(
            bytes.clone(),
            &UpdateComment {
                handle,
                text: "Middle changed".into(),
            },
        )
        .output_artifact
        .unwrap();
        assert_eq!(first, record(&edited, "0"));
        assert_eq!(third, record(&edited, "2"));
        assert_eq!(
            part(&bytes, "/word/document.xml"),
            part(&edited, "/word/document.xml")
        );
        let added = add(edited, "First", "Another");
        let handle = inspect_docx_comments(added.clone(), 0, 20)
            .comments
            .iter()
            .find(|c| c.id.as_deref() == Some("0"))
            .unwrap()
            .handle
            .clone()
            .unwrap();
        let deleted = execute_docx_delete_comment(added, &DeleteComment { handle })
            .output_artifact
            .unwrap();
        assert_eq!(third, record(&deleted, "2"));
        assert_eq!(
            inspect_docx_comments(deleted, 0, 20)
                .comments
                .iter()
                .map(|c| c.id.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["3", "1", "2"]
        );
    }

    #[test]
    fn unsafe_ranges_and_orphan_markers_fail_without_output() {
        for paragraph in [
            "<w:p><w:r><w:t>needle needle</w:t></w:r></w:p>",
            "<w:p><w:hyperlink><w:r><w:t>needle</w:t></w:r></w:hyperlink></w:p>",
            "<w:p><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:t>needle</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>",
            "<w:p><w:commentRangeStart w:id=\"7\"/><w:r><w:t>needle</w:t></w:r></w:p>",
        ] {
            let result = execute_docx_add_comment(
                fixture(paragraph),
                &AddComment {
                    target: TextTarget {
                        text: "needle".into(),
                        occurrence: None,
                    },
                    text: "Check".into(),
                    author: "Reviewer".into(),
                    initials: None,
                    date: "2026-10-04T12:00:00Z".into(),
                },
            );
            assert!(!result.operation.diagnostics.is_empty());
            assert!(result.output_artifact.is_none());
        }
        assert!(!valid_review_date("2026-02-30T12:00:00Z"));
        assert!(!valid_review_date("2026-+1-01T12:00:00Z"));
        assert!(plain_text("invalid \u{fffe}", NS[0]).is_err());
        assert!(valid_review_date("2024-02-29T12:00:00.123Z"));
    }
}
