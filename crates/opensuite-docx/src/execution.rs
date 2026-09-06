use opensuite_opc::Package;
use opensuite_protocol::{
    Diagnostic, DiagnosticSeverity, FindText, FindTextResult, InspectTextContext,
    InspectTextContextResult, OperationResult, ReplaceText,
};

use crate::{open_main_source, replace_text_to_vec};

/// The result of executing one DOCX operation against an immutable artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocxExecutionResult {
    pub operation: OperationResult,
    pub output_artifact: Option<Vec<u8>>,
}

/// Executes `ReplaceText` against owned DOCX bytes and returns verified output bytes on success.
pub fn execute_docx_replace_text(
    input_artifact: Vec<u8>,
    operation: &ReplaceText,
) -> DocxExecutionResult {
    let package = match Package::from_bytes(input_artifact) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    let (main, source) = match open_main_source(&package) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load DOCX artifact"),
    };
    match replace_text_to_vec(&package, &main, &source, operation) {
        Ok(output_artifact) => DocxExecutionResult {
            operation: OperationResult::applied(
                operation.expected_current_text.clone(),
                operation.replacement.clone(),
            ),
            output_artifact: Some(output_artifact),
        },
        Err(mut operation) => {
            for diagnostic in &mut operation.diagnostics {
                if diagnostic.code == "DOCUMENT_INVALID" {
                    diagnostic.message = "output DOCX artifact failed validation".to_owned();
                }
            }
            DocxExecutionResult {
                operation,
                output_artifact: None,
            }
        }
    }
}

/// Finds exact Current-view text in an immutable DOCX artifact.
pub fn find_docx_text(input_artifact: Vec<u8>, request: &FindText) -> FindTextResult {
    let source = match source_from_artifact(input_artifact) {
        Ok(source) => source,
        Err(code) => return failed_find(request, code),
    };
    crate::find_text(&source, request).unwrap_or_else(|error| failed_find(request, error.code()))
}

/// Inspects bounded text context in an immutable DOCX artifact.
pub fn inspect_docx(
    input_artifact: Vec<u8>,
    request: &InspectTextContext,
) -> InspectTextContextResult {
    let source = match source_from_artifact(input_artifact) {
        Ok(source) => source,
        Err(code) => {
            return InspectTextContextResult::failed(
                request.target.clone(),
                code,
                "could not load DOCX artifact",
            );
        }
    };
    crate::inspect_text_context(&source, request).unwrap_or_else(|error| {
        InspectTextContextResult::failed(
            request.target.clone(),
            error.code(),
            "could not inspect DOCX artifact",
        )
    })
}

fn source_from_artifact(input_artifact: Vec<u8>) -> Result<crate::SourceDocument, &'static str> {
    let package = Package::from_bytes(input_artifact).map_err(|error| error.code())?;
    let (_, source) = open_main_source(&package).map_err(|error| error.code())?;
    Ok(source)
}

fn failed_find(request: &FindText, code: impl Into<String>) -> FindTextResult {
    FindTextResult {
        query: request.text.clone(),
        matches: Vec::new(),
        diagnostics: vec![Diagnostic::new(
            code,
            DiagnosticSeverity::Error,
            "could not load DOCX artifact",
        )],
    }
}

fn failed(code: impl Into<String>, message: impl Into<String>) -> DocxExecutionResult {
    DocxExecutionResult {
        operation: OperationResult::failed(code, message),
        output_artifact: None,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read, Write};

    use opensuite_protocol::{OperationStatus, TextTarget};
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

    use super::*;

    const OFFICE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    fn fixture() -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        writer
            .write_all(
                b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>",
            )
            .unwrap();
        writer.start_file("_rels/.rels", options).unwrap();
        writer.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"word/document.xml\"/></Relationships>").as_bytes()).unwrap();
        writer.start_file("word/document.xml", options).unwrap();
        writer.write_all(format!("<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>old text</w:t></w:r></w:p><w:p><w:r><w:t>duplicate</w:t></w:r><w:r><w:t>duplicate</w:t></w:r></w:p></w:body></w:document>").as_bytes()).unwrap();
        writer.start_file("word/media/image.bin", options).unwrap();
        writer.write_all(b"unchanged image").unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn operation(target: &str, expected: &str) -> ReplaceText {
        ReplaceText {
            target: TextTarget {
                text: target.to_owned(),
                occurrence: None,
            },
            expected_current_text: expected.to_owned(),
            replacement: "new text".to_owned(),
            base_revision: Some("application-owned-version".to_owned()),
        }
    }

    fn entry(bytes: &[u8], name: &str) -> Vec<u8> {
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut entry = archive.by_name(name).unwrap();
        let mut value = Vec::new();
        entry.read_to_end(&mut value).unwrap();
        value
    }

    #[test]
    fn returns_verified_output_bytes_for_replace_text() {
        let input = fixture();
        let result = execute_docx_replace_text(input.clone(), &operation("old text", "old text"));

        assert_eq!(result.operation.status, OperationStatus::Applied);
        let output = result.output_artifact.unwrap();
        assert_eq!(
            entry(&input, "word/media/image.bin"),
            entry(&output, "word/media/image.bin")
        );
        let package = Package::from_bytes(output).unwrap();
        package.verify().unwrap();
        let (_, source) = open_main_source(&package).unwrap();
        assert_eq!(
            crate::DocxDocument::new(&source)
                .unwrap()
                .paragraphs()
                .next()
                .unwrap()
                .text()
                .unwrap(),
            "new text"
        );
    }

    #[test]
    fn returns_no_output_for_unresolved_or_ambiguous_targets() {
        for request in [
            operation("missing", "missing"),
            operation("duplicate", "duplicate"),
        ] {
            let result = execute_docx_replace_text(fixture(), &request);
            assert_eq!(result.operation.status, OperationStatus::Failed);
            assert!(result.output_artifact.is_none());
        }
    }

    #[test]
    fn returns_no_output_for_invalid_input_or_failed_precondition() {
        let invalid =
            execute_docx_replace_text(b"not a zip".to_vec(), &operation("old text", "old text"));
        assert_eq!(invalid.operation.diagnostics[0].code, "INVALID_ZIP");
        assert!(invalid.output_artifact.is_none());

        let stale = execute_docx_replace_text(fixture(), &operation("old text", "wrong text"));
        assert_eq!(stale.operation.diagnostics[0].code, "PRECONDITION_FAILED");
        assert!(stale.output_artifact.is_none());
    }
}
