//! Author ordinary text revisions using the existing exact selection and run splitter.
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
