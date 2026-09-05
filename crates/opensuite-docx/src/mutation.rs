use std::path::Path;

use quick_xml::escape::escape;

use opensuite_opc::{Package, Part};
use opensuite_protocol::{InsertParagraphAfter, OperationResult, ReplaceText, TextTarget};

use crate::{NodeId, RevisionView, SemanticError, SourceDocument, SourceNodeKind, SourceSpan};

const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];

/// Applies one preservation-safe replacement across compatible `w:t` source regions.
pub fn replace_text(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &ReplaceText,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output == package.source_path()
        || output.exists()
            && output.canonicalize().ok() == package.source_path().canonicalize().ok()
    {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let target = match resolve_target(source, operation) {
        Ok(target) => target,
        Err(result) => return result,
    };
    if target.value != operation.expected_current_text {
        return OperationResult::failed(
            "PRECONDITION_FAILED",
            "resolved text does not match expected current text",
        );
    }
    let patched = match apply_patches(source, target.patches) {
        Ok(patched) => patched,
        Err(result) => return result,
    };
    let temporary = output.with_file_name(format!(
        ".opensuite-{}-{}.docx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) = verify_output(&temporary, &operation.replacement) {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::applied(
        operation.expected_current_text.clone(),
        operation.replacement.clone(),
    )
}

/// Inserts one plain paragraph after a safe, direct main-body paragraph anchor.
pub fn insert_paragraph_after(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertParagraphAfter,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output == package.source_path()
        || output.exists()
            && output.canonicalize().ok() == package.source_path().canonicalize().ok()
    {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let (anchor, paragraph) = match resolve_paragraph_anchor(source, &operation.anchor) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let fragment = match paragraph_fragment(source, paragraph, &operation.text) {
        Ok(fragment) => fragment,
        Err(result) => return result,
    };
    let insertion = source
        .node(paragraph)
        .expect("anchor paragraph exists")
        .span()
        .end;
    let patched = match apply_patches(
        source,
        vec![Patch {
            span: SourceSpan {
                start: insertion,
                end: insertion,
            },
            replacement: fragment,
        }],
    ) {
        Ok(patched) => patched,
        Err(result) => return result,
    };
    let temporary = output.with_file_name(format!(
        ".opensuite-{}-{}.docx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if let Err(error) = package.write_replaced_part(main, &patched, &temporary) {
        return OperationResult::failed(error.code(), error.to_string());
    }
    if let Err(result) =
        verify_inserted_output(&temporary, &operation.anchor, &anchor, &operation.text)
    {
        let _ = std::fs::remove_file(&temporary);
        return result;
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::paragraph_inserted(anchor, operation.text.clone())
}

struct ResolvedText {
    value: String,
    patches: Vec<Patch>,
}

struct Patch {
    span: SourceSpan,
    replacement: Vec<u8>,
}

fn resolve_paragraph_anchor(
    source: &SourceDocument,
    target: &TextTarget,
) -> Result<(String, NodeId), OperationResult> {
    let matches = crate::text_search::resolve_text(source, &target.text)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    if matches.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "text target was not found",
        ));
    }
    let matched = if let Some(occurrence) = target.occurrence {
        matches.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "text target occurrence was not found")
        })?
    } else if matches.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "text target matches more than one current semantic range",
        ));
    } else {
        matches.into_iter().next().expect("one match")
    };
    let paragraph = matched
        .segments
        .first()
        .and_then(|segment| paragraph_ancestor(source, segment.id))
        .ok_or_else(|| unsupported("insert_paragraph_after requires a paragraph anchor"))?;
    if matched
        .segments
        .iter()
        .any(|segment| paragraph_ancestor(source, segment.id) != Some(paragraph))
        || !safe_body_paragraph(source, paragraph)
    {
        return Err(unsupported(
            "insert_paragraph_after supports only ordinary direct body paragraphs",
        ));
    }
    Ok((matched.text, paragraph))
}

fn paragraph_ancestor(source: &SourceDocument, mut id: NodeId) -> Option<NodeId> {
    loop {
        if word(source, id, "p") {
            return Some(id);
        }
        id = source.node(id)?.parent()?;
    }
}

fn safe_body_paragraph(source: &SourceDocument, paragraph: NodeId) -> bool {
    let Some(body) = source.node(paragraph).and_then(|node| node.parent()) else {
        return false;
    };
    word(source, body, "body")
        && source
            .node(body)
            .and_then(|node| node.parent())
            .is_some_and(|document| word(source, document, "document"))
        && !source.children(paragraph).any(|id| {
            word(source, id, "pPr")
                && source
                    .children(id)
                    .any(|child| word(source, child, "sectPr"))
        })
        && !has_revision_wrapper(source, paragraph)
}

fn has_revision_wrapper(source: &SourceDocument, id: NodeId) -> bool {
    word(source, id, "ins")
        || word(source, id, "del")
        || word(source, id, "moveFrom")
        || word(source, id, "moveTo")
        || source
            .children(id)
            .any(|child| has_revision_wrapper(source, child))
}

fn paragraph_fragment(
    source: &SourceDocument,
    paragraph: NodeId,
    text: &str,
) -> Result<Vec<u8>, OperationResult> {
    let prefix = word_prefix(source, paragraph)?;
    let name = |local: &str| {
        if prefix.is_empty() {
            local.to_owned()
        } else {
            format!("{prefix}:{local}")
        }
    };
    if text.is_empty() {
        return Ok(format!("<{}></{}>", name("p"), name("p")).into_bytes());
    }
    let space = if requires_space_preservation(text) {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    Ok(format!(
        "<{}><{}><{}{}>{}</{}></{}></{}>",
        name("p"),
        name("r"),
        name("t"),
        space,
        escape(text),
        name("t"),
        name("r"),
        name("p")
    )
    .into_bytes())
}

fn word_prefix(source: &SourceDocument, id: NodeId) -> Result<&str, OperationResult> {
    let SourceNodeKind::Element { start_tag, .. } =
        source.node(id).expect("source node exists").kind()
    else {
        return Err(unsupported("anchor paragraph has no source tag"));
    };
    let tag = std::str::from_utf8(&source.original_bytes()[start_tag.start..start_tag.end])
        .map_err(|_| unsupported("anchor paragraph tag is not UTF-8"))?;
    let name = tag
        .strip_prefix('<')
        .and_then(|value| {
            value
                .split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
                .next()
        })
        .ok_or_else(|| unsupported("anchor paragraph tag is invalid"))?;
    let prefix = name
        .strip_suffix("p")
        .and_then(|value| value.strip_suffix(':'))
        .unwrap_or("");
    if name != "p" && !name.ends_with(":p") {
        return Err(unsupported("anchor paragraph tag is invalid"));
    }
    Ok(prefix)
}

fn resolve_target(
    source: &SourceDocument,
    operation: &ReplaceText,
) -> Result<ResolvedText, OperationResult> {
    let matches = crate::text_search::resolve_text(source, &operation.target.text)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    if matches.is_empty() {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "text target was not found",
        ));
    }
    let matched = if let Some(occurrence) = operation.target.occurrence {
        matches.into_iter().nth(occurrence).ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "text target occurrence was not found")
        })?
    } else if matches.len() != 1 {
        return Err(OperationResult::failed(
            "TARGET_AMBIGUOUS",
            "text target matches more than one current semantic range",
        ));
    } else {
        matches.into_iter().next().expect("one match")
    };
    let patches = patches_for_match(source, &matched, &operation.replacement)?;
    Ok(ResolvedText {
        value: matched.text,
        patches,
    })
}

fn patches_for_match(
    source: &SourceDocument,
    matched: &crate::text_search::ResolvedTextMatch,
    replacement: &str,
) -> Result<Vec<Patch>, OperationResult> {
    let mut runs = Vec::new();
    for segment in &matched.segments {
        if segment.inside_tracked_change {
            return Err(unsupported(
                "replace_text does not edit tracked revision text",
            ));
        }
        let run = ordinary_run(source, segment.id)
            .ok_or_else(|| unsupported("replace_text does not cross inline wrapper boundaries"))?;
        let child = text_child(source, segment.id)
            .filter(|child| !is_cdata(source, *child))
            .ok_or_else(|| unsupported("replace_text requires replaceable text source regions"))?;
        if source.children(segment.id).nth(1).is_some() {
            return Err(unsupported(
                "replace_text requires one source region per text node",
            ));
        }
        runs.push((segment, run, child));
    }
    let formatting = run_properties(source, runs[0].1);
    if runs
        .iter()
        .any(|(_, run, _)| run_properties(source, *run) != formatting)
    {
        return Err(unsupported(
            "replace_text spans incompatible run formatting regions",
        ));
    }

    let mut remaining = replacement.chars();
    let mut patches = Vec::with_capacity(runs.len() * 2);
    for (index, (segment, _, child)) in runs.into_iter().enumerate() {
        let start = matched.start.max(segment.start) - segment.start;
        let end = matched.end.min(segment.end) - segment.start;
        let prefix = &segment.source_text[..start];
        let suffix = &segment.source_text[end..];
        let matched_chars = segment.source_text[start..end].chars().count();
        let distributed: String = if index + 1 == matched.segments.len() {
            remaining.by_ref().collect()
        } else {
            remaining.by_ref().take(matched_chars).collect()
        };
        let text = format!("{prefix}{distributed}{suffix}");
        let span = source.node(child).expect("source child exists").span();
        patches.push(Patch {
            span,
            replacement: escape(&text).into_owned().into_bytes(),
        });
        if requires_space_preservation(&text) && !has_xml_space(source, segment.id) {
            patches.push(Patch {
                span: start_tag_end(source, segment.id)?,
                replacement: b" xml:space=\"preserve\"".to_vec(),
            });
        }
    }
    Ok(patches)
}

fn apply_patches(
    source: &SourceDocument,
    mut patches: Vec<Patch>,
) -> Result<Vec<u8>, OperationResult> {
    patches.sort_by_key(|patch| (patch.span.start, patch.span.end));
    if patches
        .windows(2)
        .any(|pair| pair[0].span.end > pair[1].span.start)
    {
        return Err(unsupported(
            "replace_text generated overlapping source patches",
        ));
    }
    let mut result = source.original_bytes().to_vec();
    for patch in patches.into_iter().rev() {
        result.splice(patch.span.start..patch.span.end, patch.replacement);
    }
    Ok(result)
}

fn ordinary_run(source: &SourceDocument, text: NodeId) -> Option<NodeId> {
    let run = source.node(text)?.parent()?;
    (word(source, run, "r")
        && source
            .node(run)?
            .parent()
            .is_some_and(|parent| word(source, parent, "p")))
    .then_some(run)
}

fn run_properties(source: &SourceDocument, run: NodeId) -> Vec<u8> {
    source
        .children(run)
        .find(|child| word(source, *child, "rPr"))
        .map_or_else(Vec::new, |properties| {
            let span = source.node(properties).expect("source node exists").span();
            source.original_bytes()[span.start..span.end].to_vec()
        })
}

fn requires_space_preservation(text: &str) -> bool {
    text.starts_with(char::is_whitespace) || text.ends_with(char::is_whitespace)
}

fn has_xml_space(source: &SourceDocument, text: NodeId) -> bool {
    let SourceNodeKind::Element { start_tag, .. } =
        source.node(text).expect("source node exists").kind()
    else {
        return false;
    };
    source.original_bytes()[start_tag.start..start_tag.end]
        .windows(b"xml:space".len())
        .any(|window| window == b"xml:space")
}

fn start_tag_end(source: &SourceDocument, text: NodeId) -> Result<SourceSpan, OperationResult> {
    let SourceNodeKind::Element { start_tag, .. } =
        source.node(text).expect("source node exists").kind()
    else {
        return Err(unsupported("replace_text text node has no start tag"));
    };
    let end = start_tag
        .end
        .checked_sub(1)
        .ok_or_else(|| unsupported("replace_text text tag is invalid"))?;
    Ok(SourceSpan { start: end, end })
}

fn unsupported(message: impl Into<String>) -> OperationResult {
    OperationResult::failed("UNSUPPORTED_OPERATION", message)
}

fn verify_output(output: &Path, replacement: &str) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let document = crate::DocxDocument::new(&source).map_err(document_invalid)?;
    if !document
        .blocks()
        .map(|block| match block {
            crate::BodyBlock::Paragraph(paragraph) => {
                paragraph.text_for_view(RevisionView::Current)
            }
            crate::BodyBlock::Table(table) => table_current_text(table),
        })
        .collect::<Result<String, SemanticError>>()
        .map_err(document_invalid)?
        .contains(replacement)
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            format!(
                "output package {0} does not contain the replacement",
                main.name
            ),
        ));
    }
    Ok(())
}

fn verify_inserted_output(
    output: &Path,
    target: &TextTarget,
    anchor_text: &str,
    inserted_text: &str,
) -> Result<(), OperationResult> {
    let package = Package::open(output).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (resolved_anchor, anchor) = resolve_paragraph_anchor(&source, target)?;
    if resolved_anchor != anchor_text {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output anchor text changed",
        ));
    }
    let document = crate::DocxDocument::new(&source).map_err(document_invalid)?;
    let mut blocks = document.blocks();
    while let Some(block) = blocks.next() {
        if matches!(block, crate::BodyBlock::Paragraph(ref paragraph) if paragraph.source_id() == anchor)
        {
            let Some(crate::BodyBlock::Paragraph(inserted)) = blocks.next() else {
                return Err(OperationResult::failed(
                    "DOCUMENT_INVALID",
                    "inserted paragraph is not immediately after anchor",
                ));
            };
            return (inserted
                .text_for_view(RevisionView::Current)
                .map_err(document_invalid)?
                == inserted_text)
                .then_some(())
                .ok_or_else(|| {
                    OperationResult::failed(
                        "DOCUMENT_INVALID",
                        "inserted paragraph text does not match request",
                    )
                });
        }
    }
    Err(OperationResult::failed(
        "DOCUMENT_INVALID",
        "output anchor paragraph was not found",
    ))
}

fn table_current_text(table: crate::Table<'_>) -> Result<String, SemanticError> {
    let mut text = String::new();
    for row in table.rows() {
        for cell in row.cells() {
            text.push_str(&cell.text_for_view(RevisionView::Current)?);
        }
    }
    Ok(text)
}

fn document_invalid(error: impl std::fmt::Display) -> OperationResult {
    OperationResult::failed("DOCUMENT_INVALID", error.to_string())
}

fn text_child(source: &SourceDocument, id: NodeId) -> Option<NodeId> {
    source.children(id).next()
}

fn is_cdata(source: &SourceDocument, id: NodeId) -> bool {
    let span = source.node(id).expect("source node exists").span();
    source.original_bytes()[..span.start].ends_with(b"<![CDATA[")
}

fn word(source: &SourceDocument, id: NodeId, local_name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name, .. }) if name.local_name() == local_name && name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Write,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use opensuite_protocol::{InsertParagraphAfter, ReplaceText, TextTarget};
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    const OFFICE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
    static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

    fn path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "opensuite-mutation-{name}-{}-{}.docx",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn fixture() -> std::path::PathBuf {
        let path = path("input");
        let file = fs::File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(
            b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>",
        )
        .unwrap();
        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"word/document.xml\"/></Relationships>").as_bytes()).unwrap();
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(format!("<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>OLD UNIQUE TEXT</w:t></w:r></w:p><w:p><w:r><w:t>Duplicate</w:t></w:r><w:r><w:t>Duplicate</w:t></w:r></w:p><w:p><w:r><w:t>Cross </w:t></w:r><w:r><w:t>run</w:t></w:r></w:p><w:p><w:r><w:t>Styled </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>text</w:t></w:r></w:p><w:p><w:r><w:t>Linked </w:t></w:r><w:hyperlink><w:r><w:t>text</w:t></w:r></w:hyperlink></w:p><w:p><w:r><w:t>FY2026 Revenue </w:t></w:r><w:r><w:t>was $10M according</w:t></w:r></w:p><w:p><w:r><w:t xml:space=\"preserve\"> spaced </w:t></w:r></w:p><w:p><w:del><w:r><w:delText>OLD DELETED</w:delText></w:r></w:del><w:ins><w:r><w:t>NEW INSERTED</w:t></w:r></w:ins></w:p></w:body></w:document>").as_bytes()).unwrap();
        zip.start_file("word/media/image.bin", options).unwrap();
        zip.write_all(&[1, 2, 3]).unwrap();
        zip.finish().unwrap();
        path
    }

    fn operation(target: &str, expected: &str, replacement: &str) -> ReplaceText {
        ReplaceText {
            target: TextTarget {
                text: target.to_owned(),
                occurrence: None,
            },
            expected_current_text: expected.to_owned(),
            replacement: replacement.to_owned(),
            base_revision: Some("caller-version-7".to_owned()),
        }
    }

    fn execute(input: &Path, output: &Path, operation: &ReplaceText) -> OperationResult {
        let package = Package::open(input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        replace_text(&package, &main, &source, operation, output)
    }

    fn insert_operation(anchor: &str, text: &str) -> InsertParagraphAfter {
        InsertParagraphAfter {
            anchor: TextTarget {
                text: anchor.to_owned(),
                occurrence: None,
            },
            text: text.to_owned(),
            base_revision: Some("caller-version-7".to_owned()),
        }
    }

    fn insert_execute(
        input: &Path,
        output: &Path,
        operation: &InsertParagraphAfter,
    ) -> OperationResult {
        let package = Package::open(input).unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        insert_paragraph_after(&package, &main, &source, operation, output)
    }

    fn entry(path: &Path, name: &str) -> Vec<u8> {
        let file = fs::File::open(path).unwrap();
        let mut zip = ZipArchive::new(file).unwrap();
        let mut value = Vec::new();
        std::io::Read::read_to_end(&mut zip.by_name(name).unwrap(), &mut value).unwrap();
        value
    }

    #[test]
    fn replaces_one_current_text_node_without_changing_other_parts() {
        let input = fixture();
        let output = path("output");
        let result = execute(
            &input,
            &output,
            &operation("OLD UNIQUE TEXT", "OLD UNIQUE TEXT", "NEW & < >"),
        );

        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert_eq!(result.changes[0].before, "OLD UNIQUE TEXT");
        assert_eq!(result.changes[0].after, "NEW & < >");
        assert_eq!(result.to_json()["changes"][0]["kind"], "text_replaced");
        assert!(!result.to_json().to_string().contains("NodeId"));
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        assert_eq!(entry(&input, "_rels/.rels"), entry(&output, "_rels/.rels"));
        assert!(
            std::str::from_utf8(&entry(&output, "word/document.xml"))
                .unwrap()
                .contains("NEW &amp; &lt; &gt;")
        );
        assert!(
            std::str::from_utf8(&entry(&input, "word/document.xml"))
                .unwrap()
                .contains("OLD UNIQUE TEXT")
        );
        let package = Package::open(&output).unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        let document = crate::DocxDocument::new(&source).unwrap();
        assert!(
            document
                .paragraphs()
                .next()
                .unwrap()
                .text_for_view(RevisionView::Current)
                .unwrap()
                .contains("NEW & < >")
        );

        let whitespace_output = path("whitespace");
        let whitespace = execute(
            &input,
            &whitespace_output,
            &operation(" spaced ", " spaced ", " kept "),
        );
        assert_eq!(
            whitespace.status,
            opensuite_protocol::OperationStatus::Applied
        );
        assert!(
            std::str::from_utf8(&entry(&whitespace_output, "word/document.xml"))
                .unwrap()
                .contains("xml:space=\"preserve\"> kept </w:t>")
        );

        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
        fs::remove_file(whitespace_output).unwrap();
    }

    #[test]
    fn rejects_ambiguous_stale_unsafe_and_non_current_targets() {
        let input = fixture();
        let output = path("failed");
        for (target, expected, code) in [
            ("Duplicate", "Duplicate", "TARGET_AMBIGUOUS"),
            ("OLD UNIQUE TEXT", "stale", "PRECONDITION_FAILED"),
            ("Styled text", "Styled text", "UNSUPPORTED_OPERATION"),
            ("Linked text", "Linked text", "UNSUPPORTED_OPERATION"),
            ("OLD DELETED", "OLD DELETED", "TARGET_NOT_FOUND"),
            ("NEW INSERTED", "NEW INSERTED", "UNSUPPORTED_OPERATION"),
            ("missing", "missing", "TARGET_NOT_FOUND"),
        ] {
            let result = execute(&input, &output, &operation(target, expected, "new"));
            assert_eq!(result.status, opensuite_protocol::OperationStatus::Failed);
            assert_eq!(result.diagnostics[0].code, code);
            assert!(!output.exists());
        }
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn replaces_cross_run_text_with_distribution_partial_ranges_and_xml_space() {
        let input = fixture();
        let output = path("cross-run");
        let result = execute(
            &input,
            &output,
            &operation("Cross run", "Cross run", "Longer replacement"),
        );

        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
        assert!(xml.contains(
            "<w:t>Longer</w:t></w:r><w:r><w:t xml:space=\"preserve\"> replacement</w:t>"
        ));
        let package = Package::open(&output).unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        assert!(
            crate::find_text(
                &source,
                &opensuite_protocol::FindText {
                    text: "Longer replacement".to_owned()
                }
            )
            .unwrap()
            .matches
            .len()
                == 1
        );

        let partial_output = path("partial");
        let partial = execute(
            &input,
            &partial_output,
            &operation(
                "Revenue was $10M",
                "Revenue was $10M",
                "Profit was $12M & more",
            ),
        );
        assert_eq!(partial.status, opensuite_protocol::OperationStatus::Applied);
        let partial_xml = String::from_utf8(entry(&partial_output, "word/document.xml")).unwrap();
        assert!(partial_xml.contains("FY2026 Profit w"));
        assert!(partial_xml.contains("as $12M &amp; more according"));

        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
        fs::remove_file(partial_output).unwrap();
    }

    #[test]
    fn permits_shorter_cross_run_replacement_without_partial_output() {
        let input = fixture();
        let output = path("shorter");
        let result = execute(&input, &output, &operation("Cross run", "Cross run", "X"));

        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert!(
            String::from_utf8(entry(&output, "word/document.xml"))
                .unwrap()
                .contains("<w:t>X</w:t></w:r><w:r><w:t></w:t>")
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn uses_shared_occurrence_order_for_replacement() {
        let input = fixture();
        let output = path("occurrence");
        let mut request = operation("Duplicate", "Duplicate", "Changed");
        request.target.occurrence = Some(1);

        let result = execute(&input, &output, &request);

        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert!(
            std::str::from_utf8(&entry(&output, "word/document.xml"))
                .unwrap()
                .contains("<w:t>Duplicate</w:t></w:r><w:r><w:t>Changed</w:t>")
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn inserts_plain_paragraph_after_a_safe_anchor_and_preserves_other_parts() {
        let input = fixture();
        let output = path("inserted");
        let result = insert_execute(
            &input,
            &output,
            &insert_operation("OLD UNIQUE TEXT", "New & < > paragraph"),
        );

        assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
        assert_eq!(result.changes[0].kind, "paragraph_inserted");
        assert!(!result.to_json().to_string().contains("NodeId"));
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        let xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
        assert!(xml.contains("<w:t>OLD UNIQUE TEXT</w:t></w:r></w:p><w:p><w:r><w:t>New &amp; &lt; &gt; paragraph</w:t></w:r></w:p>"));
        let package = Package::open(&output).unwrap();
        package.verify().unwrap();
        let (_, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(
            crate::DocxDocument::new(&source)
                .unwrap()
                .paragraphs()
                .nth(1)
                .unwrap()
                .text()
                .unwrap(),
            "New & < > paragraph"
        );

        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn inserts_whitespace_unicode_and_empty_paragraphs() {
        for (text, expected) in [
            (" hello ", " xml:space=\"preserve\"> hello "),
            ("你好", ">你好<"),
            ("", "<w:p></w:p>"),
        ] {
            let input = fixture();
            let output = path("insert-text");
            let result =
                insert_execute(&input, &output, &insert_operation("OLD UNIQUE TEXT", text));
            assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
            assert!(
                String::from_utf8(entry(&output, "word/document.xml"))
                    .unwrap()
                    .contains(expected)
            );
            fs::remove_file(input).unwrap();
            fs::remove_file(output).unwrap();
        }
    }

    #[test]
    fn rejects_ambiguous_and_unsafe_structural_anchors() {
        let input = fixture();
        let output = path("insert-failed");
        assert_eq!(
            insert_execute(&input, &output, &insert_operation("Duplicate", "new")).diagnostics[0]
                .code,
            "TARGET_AMBIGUOUS"
        );
        let mut occurrence = insert_operation("Duplicate", "new");
        occurrence.anchor.occurrence = Some(1);
        assert_eq!(
            insert_execute(&input, &output, &occurrence).status,
            opensuite_protocol::OperationStatus::Applied
        );
        fs::remove_file(&output).unwrap();
        fs::remove_file(input).unwrap();

        for xml in [
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>needle</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr><w:sectPr/></w:pPr><w:r><w:t>needle</w:t></w:r></w:p></w:body></w:document>"
            ),
            format!(
                "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:ins><w:r><w:t>needle</w:t></w:r></w:ins></w:p></w:body></w:document>"
            ),
        ] {
            let source = SourceDocument::parse(xml.into_bytes()).unwrap();
            assert_eq!(
                resolve_paragraph_anchor(
                    &source,
                    &TextTarget {
                        text: "needle".to_owned(),
                        occurrence: None
                    }
                )
                .unwrap_err()
                .diagnostics[0]
                    .code,
                "UNSUPPORTED_OPERATION"
            );
        }
    }

    #[test]
    fn uses_the_anchor_prefix_for_strict_wordprocessingml() {
        let strict = "http://purl.oclc.org/ooxml/wordprocessingml/main";
        let source = SourceDocument::parse(format!("<word:document xmlns:word=\"{strict}\"><word:body><word:p><word:r><word:t>needle</word:t></word:r></word:p><word:sectPr/></word:body></word:document>").into_bytes()).unwrap();
        let (_, paragraph) = resolve_paragraph_anchor(
            &source,
            &TextTarget {
                text: "needle".to_owned(),
                occurrence: None,
            },
        )
        .unwrap();
        assert_eq!(
            paragraph_fragment(&source, paragraph, "new").unwrap(),
            b"<word:p><word:r><word:t>new</word:t></word:r></word:p>"
        );
    }
}
