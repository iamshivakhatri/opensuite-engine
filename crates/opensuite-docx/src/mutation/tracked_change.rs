//! Author and decide ordinary text revisions using source-preserving patches.
use super::*;
use crate::TrackedChangeKind;
use opensuite_protocol::{
    DeleteTrackedText, InsertTrackedText, ReplaceTextWithTrackedChange, TrackedTextPosition,
};

pub fn insert_tracked_text_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    op: &InsertTrackedText,
) -> Result<Vec<u8>, OperationResult> {
    author_text_change(
        package,
        main,
        source,
        &op.target,
        TextChange::Insert(&op.text, op.position),
        &op.author,
        &op.date,
    )
}
pub fn delete_tracked_text_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    op: &DeleteTrackedText,
) -> Result<Vec<u8>, OperationResult> {
    author_text_change(
        package,
        main,
        source,
        &op.target,
        TextChange::Delete,
        &op.author,
        &op.date,
    )
}
pub fn replace_text_with_tracked_change_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    op: &ReplaceTextWithTrackedChange,
) -> Result<Vec<u8>, OperationResult> {
    author_text_change(
        package,
        main,
        source,
        &op.target,
        TextChange::Replace(&op.replacement),
        &op.author,
        &op.date,
    )
}

enum TextChange<'a> {
    Insert(&'a str, TrackedTextPosition),
    Delete,
    Replace(&'a str),
}

fn author_text_change(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    target: &TextTarget,
    edit: TextChange<'_>,
    author: &str,
    date: &str,
) -> Result<Vec<u8>, OperationResult> {
    let inserted = match edit {
        TextChange::Insert(text, _) | TextChange::Replace(text) => text,
        TextChange::Delete => "",
    };
    if author.trim().is_empty()
        || [author, date]
            .iter()
            .any(|v| v.len() > 1024 || v.chars().any(invalid_xml_character))
        || !valid_review_date(date)
        || inserted.len() > 32_000
        || inserted.chars().any(invalid_xml_character)
        || (!matches!(edit, TextChange::Delete) && inserted.is_empty())
    {
        return Err(failure(
            "INVALID_OPERATION",
            "provide nonempty plain text, author, and a valid UTC ISO date; text is limited to 32000 bytes",
        ));
    }
    let (matched, paragraph) =
        text::simple_body_text_range(source, target, "UNSUPPORTED_REVISION_SELECTION")?;
    let paragraph_span = source.node(paragraph).unwrap().span();
    if source.node_ids().any(|id| {
        let span = source.node(id).unwrap().span();
        span.start >= paragraph_span.start
            && span.end <= paragraph_span.end
            && crate::tracked_change::revision_node(source, id)
    }) {
        return Err(
            unsupported("authoring in a paragraph containing revisions is unsupported")
                .with_reason_code("UNSUPPORTED_REVISION_SELECTION"),
        );
    }
    let runs: Vec<_> = matched
        .segments
        .iter()
        .map(|s| ordinary_run(source, s.id).unwrap())
        .collect();
    let first = source.node(runs[0]).unwrap().span();
    let last = source.node(*runs.last().unwrap()).unwrap().span();
    // Unlike comment anchors, revision wrapping must not relocate review/bookmark markers.
    if source.children(paragraph).any(|id| {
        let span = source.node(id).unwrap().span();
        span.start >= first.start
            && span.end <= last.end
            && !runs.contains(&id)
            && !matches!(source.node(id).unwrap().kind(), SourceNodeKind::Text)
    }) {
        return Err(
            unsupported("revision selection crosses a marker or wrapper")
                .with_reason_code("UNSUPPORTED_REVISION_SELECTION"),
        );
    }
    let formatting = run_properties(source, runs[0]);
    if matches!(edit, TextChange::Replace(_))
        && runs
            .iter()
            .any(|run| run_properties(source, *run) != formatting)
    {
        return Err(
            unsupported("tracked replacement spans incompatible run formatting")
                .with_reason_code("UNSUPPORTED_REVISION_SELECTION"),
        );
    }
    let ids = new_revision_ids(
        source,
        if matches!(edit, TextChange::Replace(_)) {
            2
        } else {
            1
        },
    )?;
    let namespace = match source.node(paragraph).unwrap().kind() {
        SourceNodeKind::Element { name, .. } => name.namespace_uri().unwrap(),
        _ => unreachable!(),
    };
    // A fresh local prefix cannot shadow namespaces used by imported runs/properties.
    let mut prefix = "opensuiteRevision".to_owned();
    let xml = std::str::from_utf8(source.original_bytes()).map_err(document_invalid)?;
    while xml.contains(&prefix) {
        prefix.push('_');
    }
    let wrap = |kind: &str, id: i32, content: &str| {
        format!(
            r#"<{prefix}:{kind} xmlns:{prefix}="{namespace}" {prefix}:id="{id}" {prefix}:author="{}" {prefix}:date="{}">{content}</{prefix}:{kind}>"#,
            escape(author),
            escape(date)
        )
    };
    let mut expected = Vec::new();
    let (span, replacement, current) = if let TextChange::Insert(_, position) = edit {
        let segment = match position {
            TrackedTextPosition::Before => &matched.segments[0],
            TrackedTextPosition::After => matched.segments.last().unwrap(),
        };
        let run = ordinary_run(source, segment.id).unwrap();
        let at = match position {
            TrackedTextPosition::Before => matched.start,
            TrackedTextPosition::After => matched.end,
        };
        let (before, _, after) = text::text_run_pieces(
            source,
            run,
            &segment.source_text,
            at - segment.start,
            at - segment.start,
            false,
        )?;
        let (_, added, _) = text::text_run_pieces(source, run, inserted, 0, inserted.len(), false)?;
        expected.push((ids[0], TrackedChangeKind::Insertion, inserted));
        let old = crate::tracked_change::text_for_view(source, paragraph, RevisionView::Current)
            .map_err(document_invalid)?;
        (
            source.node(run).unwrap().span(),
            format!("{before}{}{after}", wrap("ins", ids[0], &added)),
            format!("{}{inserted}{}", &old[..at], &old[at..]),
        )
    } else {
        let mut before = String::new();
        let mut deleted = String::new();
        let mut after = String::new();
        for (index, segment) in matched.segments.iter().enumerate() {
            let (a, b, c) = text::text_run_pieces(
                source,
                runs[index],
                &segment.source_text,
                matched.start.max(segment.start) - segment.start,
                matched.end.min(segment.end) - segment.start,
                true,
            )?;
            if index == 0 {
                before = a;
            }
            if index + 1 == runs.len() {
                after = c;
            }
            if index > 0 {
                let previous = source.node(runs[index - 1]).unwrap().span();
                let next = source.node(runs[index]).unwrap().span();
                deleted.push_str(&xml[previous.end..next.start]);
            }
            deleted.push_str(&b);
        }
        expected.push((ids[0], TrackedChangeKind::Deletion, matched.text.as_str()));
        let mut revisions = wrap("del", ids[0], &deleted);
        if matches!(edit, TextChange::Replace(_)) {
            let (_, added, _) =
                text::text_run_pieces(source, runs[0], inserted, 0, inserted.len(), false)?;
            revisions.push_str(&wrap("ins", ids[1], &added));
            expected.push((ids[1], TrackedChangeKind::Insertion, inserted));
        }
        let old = crate::tracked_change::text_for_view(source, paragraph, RevisionView::Current)
            .map_err(document_invalid)?;
        (
            SourceSpan {
                start: first.start,
                end: last.end,
            },
            format!("{before}{revisions}{after}"),
            format!("{}{inserted}{}", &old[..matched.start], &old[matched.end..]),
        )
    };
    let document = apply_patches(
        source,
        vec![Patch {
            span,
            replacement: replacement.into_bytes(),
        }],
    )?;
    let output = package
        .write_replaced_part_to_vec(main, &document)
        .map_err(document_invalid)?;
    verify_authored(output, source, paragraph, &current, &expected, author, date)
}

fn invalid_xml_character(c: char) -> bool {
    c.is_control() || matches!(c, '\u{fffe}' | '\u{ffff}')
}
fn failure(code: &str, message: &str) -> OperationResult {
    OperationResult::failed(code, message).with_reason_code(code)
}

fn new_revision_ids(source: &SourceDocument, count: i32) -> Result<Vec<i32>, OperationResult> {
    let mut maximum = -1;
    for id in source.node_ids() {
        let value = source.node(id).unwrap().attribute("id");
        if crate::tracked_change::revision_node(source, id)
            && value.is_none_or(|v| v.parse::<i32>().is_err())
        {
            return Err(failure(
                "MALFORMED_REVISION",
                "existing revisions must have numeric IDs before authoring",
            ));
        }
        // Reserve all numeric source IDs, including unknown/range-end records.
        if let Some(value) = value.and_then(|v| v.parse::<i32>().ok()) {
            maximum = maximum.max(value);
        }
    }
    let end = maximum
        .checked_add(count)
        .ok_or_else(|| failure("REVISION_ID_CONFLICT", "no revision IDs available"))?;
    Ok((maximum + 1..=end).collect())
}

fn verify_authored(
    output: Vec<u8>,
    before: &SourceDocument,
    paragraph: NodeId,
    current: &str,
    expected: &[(i32, TrackedChangeKind, &str)],
    author: &str,
    date: &str,
) -> Result<Vec<u8>, OperationResult> {
    let package = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let original = body_texts(before)?;
    let wanted: Vec<_> = original
        .iter()
        .map(|(id, text)| {
            if *id == paragraph {
                current.to_owned()
            } else {
                text.clone()
            }
        })
        .collect();
    let actual: Vec<_> = body_texts(&source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect();
    // Reuse the source-backed revision inspection, with full text for postconditions.
    let document = crate::DocxDocument::new(&source).map_err(document_invalid)?;
    let block_index = original
        .iter()
        .position(|(id, _)| *id == paragraph)
        .unwrap();
    let Some(crate::BodyBlock::Paragraph(after_paragraph)) = document.blocks().nth(block_index)
    else {
        return Err(failure(
            "DOCUMENT_INVALID",
            "authored revision paragraph moved",
        ));
    };
    if after_paragraph
        .text_for_view(RevisionView::Original)
        .map_err(document_invalid)?
        != original[block_index].1
    {
        return Err(failure(
            "DOCUMENT_INVALID",
            "original revision text verification failed",
        ));
    }
    let authored: Vec<_> = document
        .tracked_changes()
        .filter(|change| {
            change
                .metadata()
                .id
                .is_some_and(|id| expected.iter().any(|(n, _, _)| id == n.to_string()))
        })
        .collect();
    if wanted != actual || authored.len() != expected.len() {
        return Err(failure(
            "DOCUMENT_INVALID",
            "tracked change text or revision count verification failed",
        ));
    }
    for (change, (id, kind, text)) in authored.iter().zip(expected) {
        let metadata = change.metadata();
        if change.kind() != *kind
            || metadata.id.as_deref() != Some(id.to_string().as_str())
            || metadata.author.as_deref() != Some(author)
            || metadata.date.as_deref() != Some(date)
            || change.text().map_err(document_invalid)? != *text
        {
            return Err(failure(
                "DOCUMENT_INVALID",
                "authored revision metadata, order, or text verification failed",
            ));
        }
    }
    Ok(output)
}

pub fn accept_revision_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    op: &opensuite_protocol::AcceptRevision,
) -> Result<Vec<u8>, OperationResult> {
    decide_revision(package, main, source, &op.handle, true)
}
pub fn reject_revision_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    op: &opensuite_protocol::RejectRevision,
) -> Result<Vec<u8>, OperationResult> {
    decide_revision(package, main, source, &op.handle, false)
}

fn decide_revision(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    handle: &str,
    accept: bool,
) -> Result<Vec<u8>, OperationResult> {
    use crate::tracked_change::{inspect_source_revisions, revision_node, revision_stamp};
    let parts: Vec<_> = handle.split(':').collect();
    if parts.len() != 4 || parts[0] != "revision" {
        return Err(failure(
            "REVISION_NOT_FOUND",
            "use a revision handle from fresh inspection",
        ));
    }
    if parts[1] != revision_stamp(source) {
        return Err(failure(
            "STALE_HANDLE",
            "document changed; inspect revisions again",
        ));
    }
    let index = parts[2]
        .parse::<usize>()
        .map_err(|_| failure("REVISION_NOT_FOUND", "invalid revision handle"))?;
    let before = inspect_source_revisions(source, index, 1);
    let snapshot = before
        .revisions
        .first()
        .filter(|r| r.handle == handle)
        .ok_or_else(|| {
            failure(
                "REVISION_NOT_FOUND",
                "revision handle does not identify a source revision",
            )
        })?;
    if snapshot.structure != "supported" {
        return Err(failure(
            if snapshot.structure == "malformed" {
                "MALFORMED_REVISION"
            } else {
                "UNSUPPORTED_REVISION_TYPE"
            },
            "only supported insertion/deletion revisions can be decided",
        ));
    }
    let id = source
        .node_ids()
        .filter(|id| revision_node(source, *id))
        .nth(index)
        .unwrap();
    let node = source.node(id).unwrap();
    let SourceNodeKind::Element {
        name,
        start_tag,
        end_tag: Some(end_tag),
        ..
    } = node.kind()
    else {
        return Err(failure(
            "MALFORMED_REVISION",
            "revision must have a content wrapper",
        ));
    };
    // Keep this pass to run revisions directly inside a paragraph. Inspection can
    // still describe richer content, but decisions never guess at its structure.
    let parent = node.parent().unwrap();
    if !word_element(source, parent, "p")
        || source.children(id).any(|child| {
            !matches!(source.node(child).unwrap().kind(), SourceNodeKind::Text)
                && !word_element(source, child, "r")
        })
    {
        return Err(failure(
            "UNSUPPORTED_REVISION_TYPE",
            "revision decisions require ordinary runs in a paragraph",
        ));
    }
    for run in source
        .children(id)
        .filter(|child| word_element(source, *child, "r"))
    {
        if source.children(run).any(|child| {
            !matches!(source.node(child).unwrap().kind(), SourceNodeKind::Text)
                && !["rPr", "t", "delText", "tab", "br", "cr"]
                    .iter()
                    .any(|local| word_element(source, child, local))
        }) {
            return Err(failure(
                "UNSUPPORTED_REVISION_TYPE",
                "revision runs contain unsupported content",
            ));
        }
    }
    // Non-whitespace text outside a text element would become invalid paragraph/run content.
    if source
        .children(id)
        .chain(source.children(id).flat_map(|run| source.children(run)))
        .any(|child| {
            matches!(source.node(child).unwrap().kind(), SourceNodeKind::Text)
                && source
                    .text_bytes(child)
                    .is_some_and(|bytes| bytes.iter().any(|byte| !byte.is_ascii_whitespace()))
        })
    {
        return Err(failure(
            "MALFORMED_REVISION",
            "revision contains text outside a text element",
        ));
    }
    let deletion = name.local_name() == "del";
    let span = node.span();
    for child in source.node_ids().filter(|child| {
        let s = source.node(*child).unwrap().span();
        s.start >= start_tag.end && s.end <= end_tag.start
    }) {
        if (word_element(source, child, "delText") && !deletion)
            || (word_element(source, child, "t") && deletion)
        {
            return Err(failure(
                "MALFORMED_REVISION",
                "revision uses incompatible text elements",
            ));
        }
    }
    let keep_content = accept != deletion;
    let expected_text =
        crate::tracked_change::text_after_decision(source, parent, id, keep_content)
            .map_err(document_invalid)?;
    let mut patches = Vec::new();
    if keep_content {
        // Dropping a wrapper must not drop namespace or inherited XML context used
        // by retained content. Unused local prefixes (including ours) are safe.
        let xml = std::str::from_utf8(source.original_bytes()).map_err(document_invalid)?;
        let tag = &xml[start_tag.start + 1..start_tag.end - 1];
        let element = quick_xml::events::BytesStart::from_content(
            tag,
            tag.find(char::is_whitespace).unwrap_or(tag.len()),
        );
        let content = &xml[start_tag.end..end_tag.start];
        for attribute in element.attributes() {
            let attribute = attribute.map_err(document_invalid)?;
            let key = std::str::from_utf8(attribute.key.as_ref()).map_err(document_invalid)?;
            if key == "xmlns"
                || key.starts_with("xml:")
                || key
                    .strip_prefix("xmlns:")
                    .is_some_and(|prefix| content.contains(&format!("{prefix}:")))
            {
                return Err(failure(
                    "UNSUPPORTED_REVISION_TYPE",
                    "retained content depends on wrapper XML context",
                ));
            }
        }
        patches.push(Patch {
            span: *start_tag,
            replacement: Vec::new(),
        });
        patches.push(Patch {
            span: *end_tag,
            replacement: Vec::new(),
        });
        if deletion {
            for child in source
                .node_ids()
                .filter(|child| word_element(source, *child, "delText"))
            {
                let child = source.node(child).unwrap();
                if child.span().start < start_tag.end || child.span().end > end_tag.start {
                    continue;
                }
                if let SourceNodeKind::Element {
                    start_tag, end_tag, ..
                } = child.kind()
                {
                    for tag in std::iter::once(start_tag).chain(end_tag.iter()) {
                        patches.push(Patch {
                            span: *tag,
                            replacement: text::rename_text_tag(&xml[tag.start..tag.end], "t")
                                .into_bytes(),
                        });
                    }
                }
            }
        }
    } else {
        patches.push(Patch {
            span,
            replacement: Vec::new(),
        });
    }
    let xml = apply_patches(source, patches)?;
    let output = package
        .write_replaced_part_to_vec(main, &xml)
        .map_err(document_invalid)?;
    let reopened = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    reopened.verify().map_err(document_invalid)?;
    let (_, after) = crate::open_main_source(&reopened).map_err(document_invalid)?;
    let inspection = inspect_source_revisions(&after, 0, 1);
    let after_paragraph = after
        .node_ids()
        .filter(|id| word_element(&after, *id, "p"))
        .nth(snapshot.paragraph_index.unwrap())
        .ok_or_else(|| failure("DOCUMENT_INVALID", "revision paragraph moved"))?;
    if crate::tracked_change::text_for_view(&after, after_paragraph, RevisionView::Current)
        .map_err(document_invalid)?
        != expected_text
    {
        return Err(failure(
            "DOCUMENT_INVALID",
            "revision decision current text verification failed",
        ));
    }
    let remaining = |source: &SourceDocument, skipped: Option<NodeId>| {
        source
            .node_ids()
            .filter(|id| Some(*id) != skipped && revision_node(source, *id))
            .map(|id| {
                let span = source.node(id).unwrap().span();
                source.original_bytes()[span.start..span.end].to_vec()
            })
            .collect::<Vec<_>>()
    };
    if inspection.total + 1 != before.total
        || remaining(source, Some(id)) != remaining(&after, None)
    {
        return Err(failure(
            "DOCUMENT_INVALID",
            "revision count or unrelated revision preservation failed",
        ));
    }
    Ok(output)
}

fn word_element(source: &SourceDocument, id: NodeId, local: &str) -> bool {
    matches!(source.node(id).map(|n| n.kind()), Some(SourceNodeKind::Element { name, .. }) if name.local_name() == local && name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}

#[cfg(test)]
mod tests {
    use super::*;
    const DATE: &str = "2026-10-04T12:00:00Z";
    fn document(body: &str) -> (Package, Part, SourceDocument) {
        let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
        let (main, _) = crate::open_main_source(&package).unwrap();
        let xml = format!(
            r#"<w:document xmlns:w="{}"><w:body>{body}</w:body></w:document>"#,
            NS[0]
        );
        let package = Package::from_bytes(
            package
                .write_replaced_part_to_vec(&main, xml.as_bytes())
                .unwrap(),
        )
        .unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        (package, main, source)
    }
    #[test]
    fn revision_decisions_preserve_runs_and_other_revisions() {
        let properties = r#"<w:rPr><w:b/><w:color w:val="006400"/></w:rPr>"#;
        for (kind, tag, accept, expected) in [
            ("ins", "t", true, "A oldB"),
            ("ins", "t", false, "AB"),
            ("del", "delText", true, "AB"),
            ("del", "delText", false, "A oldB"),
        ] {
            let other =
                r#"<w:p><w:ins w:id="7" w:author="Lee"><w:r><w:t>other</w:t></w:r></w:ins></w:p>"#;
            let (package, main, source) = document(&format!(
                r#"<w:p><w:r><w:t>A</w:t></w:r><w:{kind} w:id="7" w:author="Sarah"><w:r>{properties}<w:{tag} xml:space="preserve"> old</w:{tag}><w:tab/><w:br/></w:r></w:{kind}><w:r><w:t>B</w:t></w:r></w:p>{other}"#
            ));
            let snapshot = crate::tracked_change::inspect_source_revisions(&source, 0, 20);
            let handle = &snapshot.revisions[0].handle;
            let output = decide_revision(&package, &main, &source, handle, accept).unwrap();
            let after_package = Package::from_bytes(output.clone()).unwrap();
            let (after_main, after) = crate::open_main_source(&after_package).unwrap();
            assert_eq!(
                crate::DocxDocument::new(&after)
                    .unwrap()
                    .paragraphs()
                    .next()
                    .unwrap()
                    .text()
                    .unwrap(),
                expected
            );
            let xml = std::str::from_utf8(after.original_bytes()).unwrap();
            assert!(xml.contains(other));
            if accept == (kind == "ins") {
                assert!(xml.contains(&format!(
                    r#"{properties}<w:t xml:space="preserve"> old</w:t><w:tab/><w:br/>"#
                )));
            }
            assert_eq!(crate::inspect_docx_tracked_changes(output, 0, 20).total, 1);
            assert!(decide_revision(&after_package, &after_main, &after, handle, accept).is_err());
            assert!(decide_revision(&package, &main, &source, "7", accept).is_err());
        }
    }

    #[test]
    fn revision_decisions_fail_without_output_on_unsafe_content() {
        for revision in [
            r#"<w:ins w:id="1"><w:del w:id="2"><w:r><w:delText>nested</w:delText></w:r></w:del></w:ins>"#,
            r#"<w:moveFrom w:id="1"><w:r><w:delText>move</w:delText></w:r></w:moveFrom>"#,
            r#"<w:ins><w:r><w:t>missing ID</w:t></w:r></w:ins>"#,
            r#"<w:ins w:id="1"><w:r><w:rPr><w:rPrChange w:id="2"><w:rPr/></w:rPrChange></w:rPr><w:t>property</w:t></w:r></w:ins>"#,
            r#"<w:ins w:id="1"><w:hyperlink><w:r><w:t>link</w:t></w:r></w:hyperlink></w:ins>"#,
            r#"<w:ins w:id="1" xmlns:x="urn:unknown"><w:r x:flag="yes"><w:t>context</w:t></w:r></w:ins>"#,
        ] {
            let (package, main, source) = document(&format!("<w:p>{revision}</w:p>"));
            let handle = crate::tracked_change::inspect_source_revisions(&source, 0, 1)
                .revisions
                .remove(0)
                .handle;
            let before = source.original_bytes().to_vec();
            assert!(decide_revision(&package, &main, &source, &handle, true).is_err());
            assert_eq!(source.original_bytes(), before);
        }
    }
    #[test]
    fn tracked_replacement_preserves_formatting_and_imported_revisions() {
        let imported = r#"<w:p><w:del w:id="40" w:author="Lee" w:date="2026-10-03T12:00:00Z"><w:r><w:delText>old</w:delText></w:r></w:del><w:ins w:id="41" w:author="Lee" w:date="2026-10-03T12:00:00Z"><w:r><w:t>new</w:t></w:r></w:ins></w:p>"#;
        let properties = r#"<w:rPr><w:b/><w:color w:val="008000"/></w:rPr>"#;
        let (package, main, source) = document(&format!(
            r#"{imported}<w:p><w:r>{properties}<w:t>Revenue was $2.</w:t></w:r><w:r>{properties}<w:t>1M.</w:t></w:r></w:p>"#
        ));
        let op = ReplaceTextWithTrackedChange {
            target: TextTarget {
                text: "$2.1M".into(),
                occurrence: None,
            },
            replacement: "$2.4M".into(),
            author: "Sarah".into(),
            date: DATE.into(),
        };
        let output =
            replace_text_with_tracked_change_to_vec(&package, &main, &source, &op).unwrap();
        let snapshot = crate::inspect_docx_tracked_changes(output.clone(), 0, 20);
        assert_eq!(
            (
                snapshot.total,
                snapshot.deletion_count,
                snapshot.insertion_count
            ),
            (4, 2, 2)
        );
        assert_eq!(snapshot.revisions[2].text.as_deref(), Some("$2.1M"));
        assert_eq!(snapshot.revisions[3].text.as_deref(), Some("$2.4M"));
        assert_eq!(snapshot.revisions[2].id.as_deref(), Some("42"));
        assert_eq!(snapshot.revisions[3].id.as_deref(), Some("43"));
        assert_eq!(snapshot.revisions[3].paragraph_index, Some(1));
        assert_eq!(snapshot.revisions[3].author.as_deref(), Some("Sarah"));
        assert_eq!(snapshot.revisions[3].date.as_deref(), Some(DATE));
        let package = Package::from_bytes(output).unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        let xml = std::str::from_utf8(source.original_bytes()).unwrap();
        assert!(xml.contains(imported));
        assert!(
            xml.contains(&format!(
                r#"{properties}<w:t xml:space="preserve">$2.4M</w:t>"#
            )) || xml.contains(&format!("{properties}<w:t>$2.4M</w:t>"))
        );
        let doc = crate::DocxDocument::new(&source).unwrap();
        let paragraph = doc.paragraphs().nth(1).unwrap();
        assert_eq!(
            paragraph.text_for_view(RevisionView::Current).unwrap(),
            "Revenue was $2.4M."
        );
        assert_eq!(
            paragraph.text_for_view(RevisionView::Original).unwrap(),
            "Revenue was $2.1M."
        );
        assert!(
            !crate::text_search::resolve_text(&source, "$2.1M")
                .unwrap()
                .iter()
                .any(|m| m.text == "$2.1M")
        );
    }
    #[test]
    fn tracked_authoring_rejects_unsafe_targets_and_exhausted_ids() {
        for body in [
            r#"<w:p><w:pPr><w:pPrChange w:id="1"><w:pPr/></w:pPrChange></w:pPr><w:r><w:t>old text</w:t></w:r></w:p>"#,
            r#"<w:p><w:r><w:t>old</w:t></w:r><w:hyperlink><w:r><w:t> text</w:t></w:r></w:hyperlink></w:p>"#,
            r#"<w:p><w:r><w:t>old</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t> text</w:t></w:r></w:p>"#,
            r#"<w:p><w:ins w:id="1"><w:r><w:t>old text</w:t></w:r></w:ins></w:p>"#,
            r#"<w:p><w:r><w:t>old text</w:t></w:r></w:p><w:p><w:ins w:id="2147483647"><w:r><w:t>existing</w:t></w:r></w:ins></w:p>"#,
        ] {
            let (package, main, source) = document(body);
            let op = ReplaceTextWithTrackedChange {
                target: TextTarget {
                    text: "old text".into(),
                    occurrence: None,
                },
                replacement: "new text".into(),
                author: "Sarah".into(),
                date: DATE.into(),
            };
            let before = source.original_bytes().to_vec();
            assert!(
                replace_text_with_tracked_change_to_vec(&package, &main, &source, &op).is_err()
            );
            assert_eq!(source.original_bytes(), before);
        }
    }
}
