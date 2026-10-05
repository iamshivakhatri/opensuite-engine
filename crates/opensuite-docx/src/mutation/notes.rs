//! Plain note text and ordinary body references; all changes use source patches.
use super::*;
use crate::note::{self, NotePart, NoteRecord, NoteSet};
use opensuite_protocol::{DeleteNote, InsertNote, NoteKind, UpdateNote};

fn checked_notes(
    package: &Package,
    main: &Part,
    body: &SourceDocument,
) -> Result<NoteSet, OperationResult> {
    let set = note::load(package, main, body)?;
    if !set.diagnostics.is_empty() {
        return Err(note::error(
            "repair malformed or orphaned notes before editing",
        ));
    }
    Ok(set)
}
fn plain_text(text: &str) -> Result<(), OperationResult> {
    if text.trim().is_empty()
        || text.len() > 32000
        || text
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{fffe}' | '\u{ffff}'))
    {
        return Err(failure(
            "INVALID_OPERATION",
            "note text must be nonempty plain text in one paragraph, at most 32000 bytes",
        ));
    }
    Ok(())
}
fn checked_note<'a>(
    body: &SourceDocument,
    set: &'a NoteSet,
    handle: &str,
) -> Result<(&'a NotePart, &'a NoteRecord), OperationResult> {
    let pieces: Vec<_> = handle.split(':').collect();
    if pieces.len() != 4 || pieces[0] != "note" || pieces[1] != note::stamp(body, set) {
        return Err(failure(
            "STALE_HANDLE",
            "inspect notes again and use a fresh note handle",
        ));
    }
    let part = set
        .parts
        .iter()
        .find(|p| note::name(p.kind) == pieces[2])
        .ok_or_else(|| failure("NOTE_NOT_FOUND", "invalid note kind"))?;
    let id = pieces[3]
        .parse::<i32>()
        .map_err(|_| failure("NOTE_NOT_FOUND", "invalid note ID"))?;
    let record = part
        .records
        .iter()
        .find(|r| r.id == Some(id))
        .ok_or_else(|| {
            failure(
                "NOTE_NOT_FOUND",
                "note handle does not identify an ordinary note",
            )
        })?;
    Ok((part, record))
}
pub fn insert_note_to_vec(
    package: &Package,
    main: &Part,
    body: &SourceDocument,
    op: &InsertNote,
) -> Result<Vec<u8>, OperationResult> {
    plain_text(&op.text)?;
    let set = checked_notes(package, main, body)?;
    let part = set.parts.iter().find(|p| p.kind == op.kind).unwrap();
    let id = part
        .ids
        .iter()
        .copied()
        .max()
        .unwrap_or(1)
        .max(1)
        .checked_add(1)
        .ok_or_else(|| failure("NOTE_ID_CONFLICT", "no note ID available"))?;
    let (matched, paragraph) =
        text::simple_body_text_range(body, &op.target, "UNSUPPORTED_NOTE_REFERENCE", false)?;
    let segment = matched.segments.last().unwrap();
    let run = ordinary_run(body, segment.id).unwrap();
    let at = matched.end - segment.start;
    let (before, _, after) = text::text_run_pieces(body, run, &segment.source_text, at, at, false)?;
    let namespace = match body.node(paragraph).unwrap().kind() {
        SourceNodeKind::Element { name, .. } => name.namespace_uri().unwrap(),
        _ => unreachable!(),
    };
    let local = note::name(op.kind);
    let reference = format!(
        r#"<w:r xmlns:w="{namespace}"><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:{local}Reference w:id="{id}"/></w:r>"#
    );
    let document = apply_patches(
        body,
        vec![Patch {
            span: body.node(run).unwrap().span(),
            replacement: format!("{before}{reference}{after}").into_bytes(),
        }],
    )?;
    // Reuse the document's note style when present; never create or rewrite styles.
    let style_id = match op.kind {
        NoteKind::Footnote => "FootnoteText",
        NoteKind::Endnote => "EndnoteText",
    };
    let styles = crate::load_styles(package, main).map_err(document_invalid)?;
    let paragraph_properties = if styles
        .as_ref()
        .and_then(|s| s.style(&crate::StyleId::new(style_id.into())))
        .is_some_and(|s| s.style_type() == crate::StyleType::Paragraph)
    {
        format!(r#"<w:pPr><w:pStyle w:val="{style_id}"/></w:pPr>"#)
    } else {
        String::new()
    };
    let record = format!(
        r#"<w:{local} xmlns:w="{namespace}" w:id="{id}"><w:p>{paragraph_properties}<w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:{local}Ref/></w:r><w:r><w:tab/><w:t xml:space="preserve">{}</w:t></w:r></w:p></w:{local}>"#,
        escape(&op.text)
    );
    let bytes = if let Some(source) = &part.source {
        apply_patches(
            source,
            vec![element_insertion(
                source,
                source.root(),
                record.into_bytes(),
            )?],
        )?
    } else {
        format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:{local}s xmlns:w="{namespace}"><w:{local} w:type="separator" w:id="0"><w:p><w:r><w:separator/></w:r></w:p></w:{local}><w:{local} w:type="continuationSeparator" w:id="1"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:{local}>{record}</w:{local}s>"#).into_bytes()
    };
    let output = write_related_xml_part(
        package,
        main,
        &document,
        part.part.as_ref(),
        &bytes,
        &note::relationship(op.kind, namespace == NS[1]),
        &note::content_type(op.kind),
    )?;
    verify(
        output,
        body,
        op.kind,
        id,
        Some(&op.text),
        set.parts.iter().map(|p| p.records.len()).sum::<usize>() + 1,
    )
}
pub fn update_note_to_vec(
    package: &Package,
    main: &Part,
    body: &SourceDocument,
    op: &UpdateNote,
) -> Result<Vec<u8>, OperationResult> {
    plain_text(&op.text)?;
    let set = checked_notes(package, main, body)?;
    let (part, record) = checked_note(body, &set, &op.handle)?;
    let source = part.source.as_ref().unwrap();
    let id = record.text_node.ok_or_else(|| {
        unsupported("only one-paragraph notes with one plain text run can be updated")
            .with_reason_code("UNSUPPORTED_NOTE_CONTENT")
    })?;
    let child = text_child(source, id)
        .filter(|c| !is_cdata(source, *c))
        .ok_or_else(|| unsupported("CDATA note text is unsupported"))?;
    let mut patches = vec![Patch {
        span: source.node(child).unwrap().span(),
        replacement: escape(&op.text).into_owned().into_bytes(),
    }];
    if requires_space_preservation(&op.text) && !has_xml_space(source, id) {
        patches.push(Patch {
            span: start_tag_end(source, id)?,
            replacement: b" xml:space=\"preserve\"".to_vec(),
        });
    }
    let bytes = apply_patches(source, patches)?;
    let output = package
        .write_replaced_part_to_vec(part.part.as_ref().unwrap(), &bytes)
        .map_err(document_invalid)?;
    verify(
        output,
        body,
        part.kind,
        record.id.unwrap(),
        Some(&op.text),
        set.parts.iter().map(|p| p.records.len()).sum(),
    )
}
pub fn delete_note_to_vec(
    package: &Package,
    main: &Part,
    body: &SourceDocument,
    op: &DeleteNote,
) -> Result<Vec<u8>, OperationResult> {
    let set = checked_notes(package, main, body)?;
    let (part, record) = checked_note(body, &set, &op.handle)?;
    if record.text_node.is_none() {
        return Err(unsupported("deleting complex note bodies is unsupported")
            .with_reason_code("UNSUPPORTED_NOTE_CONTENT"));
    }
    let reference = record.references[0];
    if body
        .node(reference)
        .unwrap()
        .attribute("customMarkFollows")
        .is_some_and(|v| v != "0" && v != "false")
    {
        return Err(
            unsupported("deleting references with custom note marks is unsupported")
                .with_reason_code("UNSUPPORTED_NOTE_REFERENCE"),
        );
    }
    let run = body
        .node(reference)
        .unwrap()
        .parent()
        .ok_or_else(|| unsupported("note reference has no run"))?;
    let paragraph = body
        .node(run)
        .unwrap()
        .parent()
        .ok_or_else(|| unsupported("note reference has no paragraph"))?;
    if !word(body, run, "r")
        || !word(body, paragraph, "p")
        || !ordinary_body_paragraph(body, paragraph)
    {
        return Err(
            unsupported("note deletion requires an ordinary body reference")
                .with_reason_code("UNSUPPORTED_NOTE_REFERENCE"),
        );
    }
    let span = body.node(reference).unwrap().span();
    // A point at the marker start checks enclosing ranges without rejecting this marker itself.
    protected_range::ensure_safe_source_ranges(
        body,
        &[SourceSpan {
            start: span.start,
            end: span.start,
        }],
        "UNSUPPORTED_NOTE_REFERENCE",
        false,
    )?;
    let document = apply_patches(
        body,
        vec![Patch {
            span,
            replacement: Vec::new(),
        }],
    )?;
    let source = part.source.as_ref().unwrap();
    let bytes = apply_patches(
        source,
        vec![Patch {
            span: source.node(record.source_id).unwrap().span(),
            replacement: Vec::new(),
        }],
    )?;
    let output = package
        .write_package_with_named_changes_to_vec(
            &[
                (main.name.clone(), &document),
                (part.part.as_ref().unwrap().name.clone(), &bytes),
            ],
            &[],
        )
        .map_err(document_invalid)?;
    verify(
        output,
        body,
        part.kind,
        record.id.unwrap(),
        None,
        set.parts.iter().map(|p| p.records.len()).sum::<usize>() - 1,
    )
}
fn verify(
    output: Vec<u8>,
    before: &SourceDocument,
    kind: NoteKind,
    id: i32,
    expected: Option<&str>,
    total: usize,
) -> Result<Vec<u8>, OperationResult> {
    let package = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, body) = crate::open_main_source(&package).map_err(document_invalid)?;
    let set = checked_notes(&package, &main, &body)?;
    let part = set.parts.iter().find(|p| p.kind == kind).unwrap();
    let record = part.records.iter().find(|r| r.id == Some(id));
    let actual = record
        .map(|r| note::text(part.source.as_ref().unwrap(), r.source_id))
        .transpose()?;
    if actual.as_deref() != expected
        || set.parts.iter().map(|p| p.records.len()).sum::<usize>() != total
        || body_texts(before)?
            .iter()
            .map(|(_, t)| t)
            .ne(body_texts(&body)?.iter().map(|(_, t)| t))
    {
        return Err(failure(
            "DOCUMENT_INVALID",
            "note record, reference, text, or body preservation verification failed",
        ));
    }
    Ok(output)
}

fn failure(code: &str, message: &str) -> OperationResult {
    OperationResult::failed(code, message).with_reason_code(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> Vec<u8> {
        let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
        let (main, _) = crate::open_main_source(&package).unwrap();
        package.write_replaced_part_to_vec(&main,format!(r#"<w:document xmlns:w="{}"><w:body><w:p><w:r><w:t>Alpha Beta Gamma</w:t></w:r></w:p></w:body></w:document>"#,NS[0]).as_bytes()).unwrap()
    }
    fn insert(bytes: Vec<u8>, kind: NoteKind, target: &str, text: &str) -> Vec<u8> {
        crate::execute_docx_insert_note(
            bytes,
            &InsertNote {
                kind,
                target: TextTarget {
                    text: target.into(),
                    occurrence: None,
                },
                text: text.into(),
            },
        )
        .output_artifact
        .unwrap()
    }
    fn part(bytes: &[u8], kind: NoteKind) -> Vec<u8> {
        Package::from_bytes(bytes.to_vec())
            .unwrap()
            .read_part_by_name(
                &PartName::parse(format!("/word/{}s.xml", note::name(kind))).unwrap(),
            )
            .unwrap()
    }
    #[test]
    fn first_notes_create_valid_parts_references_and_protected_separators() {
        let mut bytes = document();
        for kind in [NoteKind::Footnote, NoteKind::Endnote] {
            bytes = insert(bytes, kind, "Alpha", "Supporting report.");
            let snapshot = crate::inspect_docx_notes(bytes.clone(), 0, 20);
            assert!(snapshot.ok);
            assert!(snapshot.diagnostics.is_empty());
            assert_eq!(snapshot.notes.last().unwrap().text, "Supporting report.");
            let xml = String::from_utf8(part(&bytes, kind)).unwrap();
            assert!(xml.contains("w:type=\"separator\" w:id=\"0\""));
            assert!(xml.contains("w:type=\"continuationSeparator\" w:id=\"1\""));
            let package = Package::from_bytes(bytes.clone()).unwrap();
            package.verify().unwrap();
            let (main, body) = crate::open_main_source(&package).unwrap();
            assert_eq!(body_texts(&body).unwrap()[0].1, "Alpha Beta Gamma");
            assert!(
                package
                    .part_relationships(&main)
                    .unwrap()
                    .iter()
                    .any(|r| r.relationship_type.as_str() == note::relationship(kind, false))
            );
            let fake = format!(
                "note:{}:{}:0",
                note::stamp(&body, &note::load(&package, &main, &body).unwrap()),
                note::name(kind)
            );
            assert!(
                crate::execute_docx_delete_note(bytes.clone(), &DeleteNote { handle: fake })
                    .output_artifact
                    .is_none()
            );
        }
        assert_eq!(crate::inspect_docx_notes(bytes, 0, 20).total, 2);
    }
    #[test]
    fn update_and_delete_preserve_ids_other_kind_and_stale_handle_checks() {
        let mut bytes = insert(document(), NoteKind::Footnote, "Alpha", "First");
        bytes = insert(bytes, NoteKind::Footnote, "Beta", "Second");
        bytes = insert(bytes, NoteKind::Endnote, "Gamma", "End");
        let end = part(&bytes, NoteKind::Endnote);
        let snapshot = crate::inspect_docx_notes(bytes.clone(), 0, 20);
        let old = snapshot.notes[0].handle.clone().unwrap();
        bytes = crate::execute_docx_update_note(
            bytes,
            &UpdateNote {
                handle: old.clone(),
                text: "  Revised & checked  ".into(),
            },
        )
        .output_artifact
        .unwrap();
        assert_eq!(part(&bytes, NoteKind::Endnote), end);
        assert!(
            crate::execute_docx_delete_note(bytes.clone(), &DeleteNote { handle: old })
                .output_artifact
                .is_none()
        );
        let snapshot = crate::inspect_docx_notes(bytes.clone(), 0, 20);
        assert_eq!(snapshot.notes[0].text, "  Revised & checked  ");
        bytes = crate::execute_docx_delete_note(
            bytes,
            &DeleteNote {
                handle: snapshot.notes[0].handle.clone().unwrap(),
            },
        )
        .output_artifact
        .unwrap();
        let snapshot = crate::inspect_docx_notes(bytes.clone(), 0, 20);
        assert_eq!(snapshot.notes[0].id, Some(3));
        assert_eq!(snapshot.notes[0].text, "Second");
        assert_eq!(snapshot.total, 2);
        assert!(snapshot.ok);
        assert_eq!(part(&bytes, NoteKind::Endnote), end);
    }
    #[test]
    fn imported_note_xml_is_preserved_and_orphan_references_refuse() {
        let mut bytes = insert(document(), NoteKind::Footnote, "Alpha", "First");
        bytes = insert(bytes, NoteKind::Footnote, "Beta", "Second");
        let package = Package::from_bytes(bytes.clone()).unwrap();
        let name = PartName::parse("/word/footnotes.xml").unwrap();
        let imported = String::from_utf8(part(&bytes, NoteKind::Footnote))
            .unwrap()
            .replace("w:id=\"3\"", "data-producer='keep' w:id=\"3\"")
            .replace("<w:separator/>", "<w:separator data-keep='exact'/>");
        bytes = package
            .write_package_with_named_changes_to_vec(&[(name, imported.as_bytes())], &[])
            .unwrap();
        let snapshot = crate::inspect_docx_notes(bytes.clone(), 0, 20);
        bytes = crate::execute_docx_update_note(
            bytes,
            &UpdateNote {
                handle: snapshot.notes[0].handle.clone().unwrap(),
                text: "Updated".into(),
            },
        )
        .output_artifact
        .unwrap();
        assert_eq!(
            String::from_utf8(part(&bytes, NoteKind::Footnote)).unwrap(),
            imported.replace(">First<", ">Updated<")
        );
        let package = Package::from_bytes(document()).unwrap();
        let (main, body) = crate::open_main_source(&package).unwrap();
        let xml = String::from_utf8(body.original_bytes().to_vec())
            .unwrap()
            .replace(
                "</w:p>",
                "<w:r><w:footnoteReference w:id=\"99\"/></w:r></w:p>",
            );
        let input = package
            .write_replaced_part_to_vec(&main, xml.as_bytes())
            .unwrap();
        assert!(!crate::inspect_docx_notes(input.clone(), 0, 20).ok);
        assert!(
            crate::execute_docx_insert_note(
                input,
                &InsertNote {
                    kind: NoteKind::Footnote,
                    target: TextTarget {
                        text: "Alpha".into(),
                        occurrence: None
                    },
                    text: "Refuse".into()
                }
            )
            .output_artifact
            .is_none()
        );
    }
}
