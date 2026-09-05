use std::path::Path;

use quick_xml::escape::escape;

use opensuite_opc::{Package, Part};
use opensuite_protocol::{OperationResult, ReplaceText};

use crate::{NodeId, RevisionView, SemanticError, SourceDocument};

/// Applies one preservation-safe, single-`w:t` text replacement to a new DOCX artifact.
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
    let patched = match patch_text(source, target.id, &operation.replacement) {
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

struct ResolvedText {
    id: NodeId,
    value: String,
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
    if matched.segments.len() != 1 {
        return Err(OperationResult::failed(
            "UNSUPPORTED_OPERATION",
            "replace_text v0 requires text contained in one w:t node",
        ));
    }
    let (start, end) = (matched.start, matched.end);
    let segment = matched.segments.into_iter().next().expect("one segment");
    if segment.inside_tracked_change || segment.start != start || segment.end != end {
        return Err(OperationResult::failed(
            "UNSUPPORTED_OPERATION",
            "replace_text v0 does not edit tracked-change or partial text ranges",
        ));
    }
    if text_child(source, segment.id).is_none_or(|child| is_cdata(source, child)) {
        return Err(OperationResult::failed(
            "UNSUPPORTED_OPERATION",
            "replace_text v0 does not edit CDATA text",
        ));
    }
    Ok(ResolvedText {
        id: segment.id,
        value: segment.source_text,
    })
}

fn patch_text(
    source: &SourceDocument,
    text_id: NodeId,
    replacement: &str,
) -> Result<Vec<u8>, OperationResult> {
    let child = text_child(source, text_id).ok_or_else(|| {
        OperationResult::failed(
            "UNSUPPORTED_OPERATION",
            "text target has no replaceable source region",
        )
    })?;
    if source.children(text_id).nth(1).is_some() {
        return Err(OperationResult::failed(
            "UNSUPPORTED_OPERATION",
            "replace_text v0 requires one text source region",
        ));
    }
    let span = source.node(child).expect("source child exists").span();
    let mut patched = Vec::with_capacity(source.original_bytes().len() + replacement.len());
    patched.extend_from_slice(&source.original_bytes()[..span.start]);
    patched.extend_from_slice(escape(replacement).as_bytes());
    patched.extend_from_slice(&source.original_bytes()[span.end..]);
    Ok(patched)
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Write,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use opensuite_protocol::{ReplaceText, TextTarget};
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
        zip.write_all(format!("<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>OLD UNIQUE TEXT</w:t></w:r></w:p><w:p><w:r><w:t>Duplicate</w:t></w:r><w:r><w:t>Duplicate</w:t></w:r></w:p><w:p><w:r><w:t>Cross </w:t></w:r><w:r><w:t>run</w:t></w:r></w:p><w:p><w:r><w:t xml:space=\"preserve\"> spaced </w:t></w:r></w:p><w:p><w:del><w:r><w:delText>OLD DELETED</w:delText></w:r></w:del><w:ins><w:r><w:t>NEW INSERTED</w:t></w:r></w:ins></w:p></w:body></w:document>").as_bytes()).unwrap();
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
    fn rejects_ambiguous_stale_cross_run_and_non_current_targets() {
        let input = fixture();
        let output = path("failed");
        for (target, expected, code) in [
            ("Duplicate", "Duplicate", "TARGET_AMBIGUOUS"),
            ("OLD UNIQUE TEXT", "stale", "PRECONDITION_FAILED"),
            ("Cross run", "Cross run", "UNSUPPORTED_OPERATION"),
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
}
