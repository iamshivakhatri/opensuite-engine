use super::super::*;
use super::support::*;

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
        let result = insert_execute(&input, &output, &insert_operation("OLD UNIQUE TEXT", text));
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
        insert_execute(&input, &output, &insert_operation("Duplicate", "new")).diagnostics[0].code,
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

#[test]
fn deletes_one_body_paragraph_without_changing_neighbor_or_other_part_bytes() {
    let input = fixture();
    let output = path("deleted");
    let package = Package::open(&input).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    let (_, paragraph) = resolve_paragraph_anchor(
        &source,
        &TextTarget {
            text: "OLD UNIQUE TEXT".to_owned(),
            occurrence: None,
        },
    )
    .unwrap();
    let span = source.node(paragraph).unwrap().span();
    let mut expected = source.original_bytes()[..span.start].to_vec();
    expected.extend_from_slice(&source.original_bytes()[span.end..]);

    let result = delete_execute(&input, &output, &delete_operation("OLD UNIQUE TEXT"));

    assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
    assert_eq!(result.changes[0].kind, "paragraph_deleted");
    assert_eq!(result.changes[0].before, "OLD UNIQUE TEXT");
    assert!(!result.to_json().to_string().contains("SourceSpan"));
    assert_eq!(entry(&output, "word/document.xml"), expected);
    assert_eq!(
        entry(&input, "word/media/image.bin"),
        entry(&output, "word/media/image.bin")
    );
    let output_package = Package::open(&output).unwrap();
    output_package.verify().unwrap();
    let (_, output_source) = crate::open_main_source(&output_package).unwrap();
    let body = body_texts(&output_source).unwrap();
    assert_eq!(body[0].1, "DuplicateDuplicate");
    assert!(
        !body
            .iter()
            .any(|(_, text)| text.contains("OLD UNIQUE TEXT"))
    );

    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
}

#[test]
fn deletion_uses_shared_occurrence_and_rejects_unsafe_ranges_or_wrappers() {
    let input = fixture();
    let output = path("delete-duplicate");
    assert_eq!(
        delete_execute(&input, &output, &delete_operation("Duplicate")).diagnostics[0].code,
        "TARGET_AMBIGUOUS"
    );
    let mut operation = delete_operation("Duplicate");
    operation.target.occurrence = Some(1);
    assert_eq!(
        delete_execute(&input, &output, &operation).status,
        opensuite_protocol::OperationStatus::Applied
    );
    fs::remove_file(&output).unwrap();
    fs::remove_file(input).unwrap();

    for xml in [
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>needle</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"
        ),
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr><w:sectPr/></w:pPr><w:r><w:t>needle</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"
        ),
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:sdt><w:sdtContent><w:p><w:r><w:t>needle</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"
        ),
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:ins><w:r><w:t>needle</w:t></w:r></w:ins></w:p></w:body></w:document>"
        ),
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:bookmarkStart w:id=\"1\" w:name=\"mark\"/><w:r><w:t>needle</w:t></w:r><w:bookmarkEnd w:id=\"1\"/></w:p></w:body></w:document>"
        ),
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:commentRangeStart w:id=\"1\"/><w:r><w:t>needle</w:t></w:r><w:commentRangeEnd w:id=\"1\"/><w:r><w:commentReference w:id=\"1\"/></w:r></w:p></w:body></w:document>"
        ),
        format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:fldSimple w:instr=\"DATE\"><w:r><w:t>needle</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"
        ),
    ] {
        let source = SourceDocument::parse(xml.into_bytes()).unwrap();
        let result = resolve_paragraph_anchor(
            &source,
            &TextTarget {
                text: "needle".to_owned(),
                occurrence: None,
            },
        );
        if let Ok((_, paragraph)) = result {
            assert!(!safe_to_delete(&source, paragraph));
        } else {
            assert_eq!(
                result.unwrap_err().diagnostics[0].code,
                "UNSUPPORTED_OPERATION"
            );
        }
    }
}
