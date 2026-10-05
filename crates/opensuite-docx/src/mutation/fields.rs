//! Typed known-field authoring. Cached text is a placeholder, never a calculation.
use super::*;
use opensuite_protocol::{FieldContent, InsertFields, InsertToc};

// Qualify new field markup with the standard `w` prefix. Casual (and other
// editors) look up field attributes on the `w` prefix; a custom prefix with the
// same URI is valid OOXML but is dropped on editor round-trip.
const FIELD_PREFIX: &str = "w";
fn field_namespace(source: &SourceDocument, id: NodeId) -> Result<String, OperationResult> {
    match source.node(id).map(|n| n.kind()) {
        Some(SourceNodeKind::Element { name, .. })
            if name.namespace_uri().is_some_and(|ns| NS.contains(&ns)) =>
        {
            Ok(format!(
                " xmlns:{FIELD_PREFIX}=\"{}\"",
                name.namespace_uri().unwrap()
            ))
        }
        _ => Err(unsupported("field paragraph requires a Word namespace")),
    }
}

// Shared with the existing canonical header/footer PAGE writer. Instructions
// come only from typed operations inside Rust, never from arbitrary user codes.
pub(super) fn simple_field_xml(
    prefix: &str,
    instruction: &str,
    result: &str,
    dirty: bool,
) -> String {
    let field = qualify(prefix, "fldSimple");
    let attr = attr_prefix(prefix);
    let flag = if dirty {
        format!(" {attr}dirty=\"true\"")
    } else {
        String::new()
    };
    let content = if result.is_empty() {
        String::new()
    } else {
        plain_run(prefix, result)
    };
    format!(
        "<{field} {attr}instr=\"{}\"{flag}>{content}</{field}>",
        escape(instruction)
    )
}
fn plain_run(prefix: &str, text: &str) -> String {
    let r = qualify(prefix, "r");
    let t = qualify(prefix, "t");
    format!(
        "<{r}><{t} xml:space=\"preserve\">{}</{t}></{r}>",
        escape(text)
    )
}
fn valid_text(text: &str) -> bool {
    text.len() <= 32000
        && !text
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{fffe}' | '\u{ffff}'))
}
fn content_xml(prefix: &str, content: &[FieldContent]) -> String {
    content
        .iter()
        .map(|item| match item {
            FieldContent::Text(text) => plain_run(prefix, text),
            FieldContent::Page => simple_field_xml(prefix, " PAGE ", "?", true),
            FieldContent::NumPages => simple_field_xml(prefix, " NUMPAGES ", "?", true),
        })
        .collect()
}
pub fn insert_fields_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    op: &InsertFields,
) -> Result<Vec<u8>, OperationResult> {
    if op.content.is_empty()
        || op.content.len() > 20
        || !op
            .content
            .iter()
            .any(|c| !matches!(c, FieldContent::Text(_)))
        || op
            .content
            .iter()
            .any(|c| matches!(c,FieldContent::Text(t) if !valid_text(t)))
    {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "provide 1–20 text/PAGE/NUMPAGES items, including a field; text must be plain and at most 32000 bytes per item",
        ));
    }
    let expected: Vec<_> = op
        .content
        .iter()
        .filter_map(|item| match item {
            FieldContent::Page => Some((" PAGE ", "?")),
            FieldContent::NumPages => Some((" NUMPAGES ", "?")),
            FieldContent::Text(_) => None,
        })
        .collect();
    if let Some(placement) = &op.placement {
        let (body, blocks) = direct_body_blocks(source)?;
        let index = placement_index(&blocks, resolve_paragraph_placement(source, placement)?)?;
        let at = body_insertion(source, body, &blocks, index)?;
        ensure_outside_fields(source, at)?;
        let prefix = FIELD_PREFIX;
        let namespace = field_namespace(source, body)?;
        let p = qualify(prefix, "p");
        let fragment = format!("<{p}{namespace}>{}</{p}>", content_xml(prefix, &op.content));
        let output = write_patches_to_vec(
            package,
            main,
            source,
            vec![Patch {
                span: SourceSpan { start: at, end: at },
                replacement: fragment.into_bytes(),
            }],
        )?;
        verify_new_fields(&output, &main.name, source, &expected, at)?;
        return Ok(output);
    }
    // Reuse the single-section footer relationship/part writer. Append rather
    // than replacing imported footer content, including unknown fields.
    let section = main_section(source, true)?;
    let kind = HeaderFooterKind::Footer;
    let references = default_header_footer_references(source, section, kind);
    if references.len() > 1 {
        return Err(unsupported("default footer reference is ambiguous"));
    }
    let mut replaced = Vec::new();
    let mut added = Vec::new();
    let (part_name, before, at) = if let Some(reference) = references.first() {
        let part = header_footer_part(package, main, source, *reference, kind)?;
        for other in source
            .node_ids()
            .filter(|id| *id != *reference && word(source, *id, "footerReference"))
        {
            if header_footer_part(package, main, source, other, kind)?.name == part.name {
                return Err(unsupported(
                    "default footer shares its part with another footer variant",
                ));
            }
        }
        let before = SourceDocument::parse(package.read_part(&part).map_err(package_failure)?)
            .map_err(document_invalid)?;
        let at = source_end_tag_start(&before, before.root())?;
        ensure_outside_fields(&before, at)?;
        if !word(&before, before.root(), "ftr") {
            return Err(unsupported("footer root is invalid"));
        }
        let prefix = FIELD_PREFIX;
        let namespace = field_namespace(&before, before.root())?;
        let p = qualify(prefix, "p");
        let fragment = format!("<{p}{namespace}>{}</{p}>", content_xml(prefix, &op.content));
        replaced.push((
            part.name.clone(),
            apply_patches(
                &before,
                vec![Patch {
                    span: SourceSpan { start: at, end: at },
                    replacement: fragment.into_bytes(),
                }],
            )?,
        ));
        (part.name, before, at)
    } else {
        let before =
            SourceDocument::parse(format!("<w:ftr xmlns:w=\"{}\"></w:ftr>", NS[0]).into_bytes())
                .map_err(document_invalid)?;
        let at = source_end_tag_start(&before, before.root())?;
        let fragment = format!("<w:p>{}</w:p>", content_xml("w", &op.content));
        let bytes = apply_patches(
            &before,
            vec![Patch {
                span: SourceSpan { start: at, end: at },
                replacement: fragment.into_bytes(),
            }],
        )?;
        let id = add_header_footer_part(package, main, kind, bytes, &mut replaced, &mut added)?;
        let part_name = added.last().expect("footer part was added").0.clone();
        replaced.push((
            main.name.clone(),
            apply_patches(
                source,
                vec![section_reference_patch(
                    source,
                    section,
                    kind,
                    header_footer_reference_xml(source, section, kind, &id)?,
                )?],
            )?,
        ));
        (part_name, before, at)
    };
    let replaced_refs: Vec<_> = replaced
        .iter()
        .map(|(n, b)| (n.clone(), b.as_slice()))
        .collect();
    let added_refs: Vec<_> = added
        .iter()
        .map(|(n, b)| (n.clone(), b.as_slice()))
        .collect();
    let output = package
        .write_package_with_named_changes_to_vec(&replaced_refs, &added_refs)
        .map_err(package_failure)?;
    verify_new_fields(&output, &part_name, &before, &expected, at)?;
    Ok(output)
}

pub fn insert_toc_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    op: &InsertToc,
) -> Result<Vec<u8>, OperationResult> {
    if !(1..=9).contains(&op.max_heading_level) || op.title.as_ref().is_some_and(|s| !valid_text(s))
    {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "TOC heading level must be 1–9 and title must be plain text of at most 32000 bytes",
        ));
    }
    let (body, blocks) = direct_body_blocks(source)?;
    let index = placement_index(&blocks, resolve_paragraph_placement(source, &op.placement)?)?;
    let at = body_insertion(source, body, &blocks, index)?;
    ensure_outside_fields(source, at)?;
    let prefix = FIELD_PREFIX;
    let namespace = field_namespace(source, body)?;
    let p = qualify(prefix, "p");
    let r = qualify(prefix, "r");
    let fld = qualify(prefix, "fldChar");
    let instr = qualify(prefix, "instrText");
    let attr = attr_prefix(prefix);
    let mut fragment = String::new();
    if let Some(title) = &op.title {
        fragment.push_str(&format!(
            "<{p}{namespace}>{}</{p}>",
            plain_run(prefix, title)
        ));
    }
    let instruction = format!(r#" TOC \o "1-{}" \h \z \u "#, op.max_heading_level);
    fragment.push_str(&format!(r#"<{p}{namespace}><{r}><{fld} {attr}fldCharType="begin" {attr}dirty="true"/></{r}><{r}><{instr} xml:space="preserve">{instruction}</{instr}></{r}><{r}><{fld} {attr}fldCharType="separate"/></{r}>{}<{r}><{fld} {attr}fldCharType="end"/></{r}></{p}>"#,plain_run(prefix,"Update this table of contents in Word.")));
    let output = write_patches_to_vec(
        package,
        main,
        source,
        vec![Patch {
            span: SourceSpan { start: at, end: at },
            replacement: fragment.into_bytes(),
        }],
    )?;
    verify_new_fields(
        &output,
        &main.name,
        source,
        &[(
            instruction.as_str(),
            "Update this table of contents in Word.",
        )],
        at,
    )?;
    Ok(output)
}
fn ensure_outside_fields(source: &SourceDocument, at: usize) -> Result<(), OperationResult> {
    let fields = crate::field::fields(source);
    if fields.errors().next().is_some()
        || fields.iter().any(|f| {
            f.bounded_text(true, 2000)
                .map_or(true, |(text, _)| text.is_none_or(|s| s.trim().is_empty()))
                || f.state() == crate::FieldState::Unterminated
                || (f.span().start < at && f.span().end > at)
        })
    {
        return Err(unsupported(
            "insertion crosses an existing field or the part has malformed field markers",
        )
        .with_reason_code("UNSAFE_FIELD_STRUCTURE"));
    }
    Ok(())
}
fn verify_new_fields(
    output: &[u8],
    name: &PartName,
    before: &SourceDocument,
    expected: &[(&str, &str)],
    at: usize,
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    crate::open_main_source(&package).map_err(document_invalid)?;
    let after = SourceDocument::parse(package.read_part_by_name(name).map_err(package_failure)?)
        .map_err(document_invalid)?;
    let fields = crate::field::fields(&after);
    let added: Vec<_> = fields
        .iter()
        .filter(|f| {
            f.span().start >= at
                && f.span().start
                    < at + after.original_bytes().len() - before.original_bytes().len()
        })
        .collect();
    let growth = after
        .original_bytes()
        .len()
        .checked_sub(before.original_bytes().len())
        .ok_or_else(|| document_invalid("field insertion shrank the part"))?;
    if after.original_bytes()[..at] != before.original_bytes()[..at]
        || after.original_bytes()[at + growth..] != before.original_bytes()[at..]
        || fields.len() != crate::field::fields(before).len() + expected.len()
        || fields.errors().next().is_some()
        || added.len() != expected.len()
        || added
            .iter()
            .zip(expected)
            .any(|(f, (instruction, result))| {
                f.state() != crate::FieldState::Complete
                    || f.instruction().ok().flatten().as_deref() != Some(*instruction)
                    || f.result_text().ok().flatten().as_deref() != Some(*result)
                    || after.node(f.source_id()).unwrap().attribute("dirty") != Some("true")
            })
    {
        return Err(document_invalid("authored field did not survive reopening"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fields_round_trip_in_body_and_footer() {
        for placement in [Some(ParagraphPlacement::End), None] {
            let bytes = crate::execute_docx_insert_fields(
                crate::create_blank_docx(),
                &InsertFields {
                    placement,
                    content: vec![
                        FieldContent::Text("Page ".into()),
                        FieldContent::Page,
                        FieldContent::Text(" of ".into()),
                        FieldContent::NumPages,
                    ],
                },
            )
            .output_artifact
            .unwrap();
            let snapshot = crate::inspect_docx_fields(bytes, 0, 20);
            assert!(snapshot.ok);
            assert_eq!(snapshot.total, 2);
            assert_eq!(
                snapshot
                    .fields
                    .iter()
                    .map(|f| (f.kind, f.cached_result.as_deref(), f.dirty))
                    .collect::<Vec<_>>(),
                vec![
                    ("page", Some("?"), Some(true)),
                    ("numPages", Some("?"), Some(true))
                ]
            );
        }
    }
    #[test]
    fn toc_round_trip_and_heading_range() {
        let bytes = crate::execute_docx_insert_toc(
            crate::create_blank_docx(),
            &InsertToc {
                placement: ParagraphPlacement::Start,
                max_heading_level: 3,
                title: Some("Table of Contents".into()),
            },
        )
        .output_artifact
        .unwrap();
        let snapshot = crate::inspect_docx_fields(bytes.clone(), 0, 20);
        let toc = &snapshot.fields[0];
        assert_eq!(
            (
                toc.kind,
                toc.representation,
                toc.structure,
                toc.dirty,
                toc.heading_levels
            ),
            ("toc", "complex", "complete", Some(true), Some([1, 3]))
        );
        assert_eq!(
            toc.instruction.as_deref(),
            Some(" TOC \\o \"1-3\" \\h \\z \\u ")
        );
        let (_, source) = crate::open_main_source(&Package::from_bytes(bytes).unwrap()).unwrap();
        let xml = String::from_utf8_lossy(source.original_bytes());
        assert!(
            xml.contains("w:fldChar"),
            "TOC must use standard w: field markup"
        );
        assert!(
            !xml.contains("opensuiteField"),
            "custom field prefixes break Casual editor round-trip"
        );
    }
    #[test]
    fn unknown_fields_survive_edits_and_malformed_markers_refuse_insertion() {
        let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        let xml = String::from_utf8(source.original_bytes().to_vec()).unwrap();
        let field = "<w:p><w:fldSimple w:instr=\" CORPORATE \\x &quot;custom&quot; \" w:fldLock=\"true\"><w:r><w:t>Cached</w:t></w:r></w:fldSimple></w:p>";
        let xml = xml.replace(
            "<w:body>",
            &format!("<w:body>{field}<w:p><w:r><w:t>Old text</w:t></w:r></w:p>"),
        );
        let bytes = package
            .write_replaced_part_to_vec(&main, xml.as_bytes())
            .unwrap();
        let edited = crate::execute_docx_replace_text(
            bytes.clone(),
            &ReplaceText {
                target: TextTarget {
                    text: "Old text".into(),
                    occurrence: None,
                },
                expected_current_text: "Old text".into(),
                replacement: "New text".into(),
                base_revision: None,
            },
        )
        .output_artifact
        .unwrap();
        let (_, edited_source) =
            crate::open_main_source(&Package::from_bytes(edited).unwrap()).unwrap();
        assert!(String::from_utf8_lossy(edited_source.original_bytes()).contains(field));
        let protected = crate::execute_docx_replace_text(
            bytes,
            &ReplaceText {
                target: TextTarget {
                    text: "Cached".into(),
                    occurrence: None,
                },
                expected_current_text: "Cached".into(),
                replacement: "changed".into(),
                base_revision: None,
            },
        );
        assert!(protected.output_artifact.is_none());
        let malformed = xml.replace(field,"<w:p><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText>TOC</w:instrText></w:r></w:p>");
        let bytes = package
            .write_replaced_part_to_vec(&main, malformed.as_bytes())
            .unwrap();
        assert_eq!(
            crate::inspect_docx_fields(bytes.clone(), 0, 20).fields[0].structure,
            "malformed"
        );
        let failed = crate::execute_docx_insert_toc(
            bytes,
            &InsertToc {
                placement: ParagraphPlacement::End,
                max_heading_level: 3,
                title: None,
            },
        );
        assert_eq!(
            failed.operation.status,
            opensuite_protocol::OperationStatus::Failed
        );
        assert!(failed.output_artifact.is_none());
    }
}
