use super::super::*;
use super::support::*;

#[test]
fn inspects_and_replaces_text_entirely_in_memory() {
    let input = fixture();
    let bytes = fs::read(&input).unwrap();
    fs::remove_file(input).unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        crate::DocxDocument::new(&source)
            .unwrap()
            .paragraphs()
            .next()
            .unwrap()
            .text_for_view(RevisionView::Current)
            .unwrap(),
        "OLD UNIQUE TEXT"
    );

    let output = replace_text_to_vec(
        &package,
        &main,
        &source,
        &operation("OLD UNIQUE TEXT", "OLD UNIQUE TEXT", "NEW UNIQUE TEXT"),
    )
    .unwrap();
    let reopened = Package::from_bytes(output).unwrap();
    reopened.verify().unwrap();
    let (_, source) = crate::open_main_source(&reopened).unwrap();
    assert_eq!(
        crate::DocxDocument::new(&source)
            .unwrap()
            .paragraphs()
            .next()
            .unwrap()
            .text_for_view(RevisionView::Current)
            .unwrap(),
        "NEW UNIQUE TEXT"
    );
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
    assert!(
        xml.contains("<w:t>Longer</w:t></w:r><w:r><w:t xml:space=\"preserve\"> replacement</w:t>")
    );
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
fn revision_snapshot_and_unrelated_edit_preserve_source() {
    let revision = r#"<w:p><w:r><w:t>Revenue: </w:t></w:r><w:del w:id="7" w:author="Sarah" w:date="2026-10-03T12:00:00Z"><w:r><w:delText>$2.1M</w:delText></w:r></w:del><w:ins w:id="8" w:author="Sarah" w:date="2026-10-03T12:00:00Z"><w:r><w:t>$2.4M</w:t></w:r></w:ins></w:p>"#;
    let xml = format!(
        r#"<w:document xmlns:w="{WORD}"><w:body>{revision}<w:p><w:r><w:t>Prepared for the board.</w:t></w:r></w:p></w:body></w:document>"#
    );
    let path = table_fixture(&xml);
    let input = fs::read(&path).unwrap();
    fs::remove_file(path).unwrap();
    let before = input.clone();
    let snapshot = crate::inspect_docx_tracked_changes(input.clone(), 0, 20);
    assert!(snapshot.ok);
    assert_eq!(
        (
            snapshot.total,
            snapshot.insertion_count,
            snapshot.deletion_count
        ),
        (2, 1, 1)
    );
    assert_eq!(snapshot.revisions[0].text.as_deref(), Some("$2.1M"));
    assert_eq!(snapshot.revisions[1].text.as_deref(), Some("$2.4M"));
    assert_eq!(snapshot.revisions[1].id.as_deref(), Some("8"));
    assert_eq!(snapshot.revisions[1].author.as_deref(), Some("Sarah"));
    assert_eq!(
        snapshot.revisions[1].date.as_deref(),
        Some("2026-10-03T12:00:00Z")
    );
    assert_eq!(snapshot.revisions[1].paragraph_index, Some(0));
    assert_eq!(snapshot.revisions[1].structure, "supported");
    assert_eq!(input, before);
    let package = Package::from_bytes(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert!(
        replace_text_to_vec(
            &package,
            &main,
            &source,
            &operation("$2.4M", "$2.4M", "$3M")
        )
        .is_err()
    );
    let output = replace_text_to_vec(
        &package,
        &main,
        &source,
        &operation(
            "Prepared for the board.",
            "Prepared for the board.",
            "Prepared for review.",
        ),
    )
    .unwrap();
    let reopened = Package::from_bytes(output).unwrap();
    let (_, after) = crate::open_main_source(&reopened).unwrap();
    assert_eq!(
        after.original_bytes(),
        xml.replace("Prepared for the board.", "Prepared for review.")
            .as_bytes()
    );
    assert!(
        std::str::from_utf8(after.original_bytes())
            .unwrap()
            .contains(revision)
    );
}

#[test]
fn ordinary_replacement_refuses_hidden_markup_between_runs() {
    for marker in [
        r#"<w:del w:id="1"><w:r><w:delText>old</w:delText></w:r></w:del>"#,
        r#"<w:moveFrom w:id="2"><w:r><w:t>moved</w:t></w:r></w:moveFrom>"#,
        r#"<w:commentRangeStart w:id="3"/>"#,
        r#"<w:bookmarkStart w:id="4" w:name="reference"/>"#,
        r#"<w:r><w:tab/></w:r>"#,
        r#"<w:customXml/>"#,
    ] {
        let input = table_fixture(&format!(
            r#"<w:document xmlns:w="{WORD}"><w:body><w:p><w:r><w:t>A</w:t></w:r>{marker}<w:r><w:t>B</w:t></w:r></w:p></w:body></w:document>"#
        ));
        let bytes = fs::read(&input).unwrap();
        let result = crate::execute_docx_replace_text(bytes.clone(), &operation("AB", "AB", "C"));
        assert_eq!(
            result.operation.diagnostics[0].code, "UNSUPPORTED_OPERATION",
            "{marker}"
        );
        assert!(result.output_artifact.is_none());
        assert_eq!(fs::read(&input).unwrap(), bytes);
        fs::remove_file(input).unwrap();
    }
}

#[test]
fn replacement_refuses_enclosing_ranges_and_preserves_independent_text() {
    for (start, end) in [
        (
            r#"<w:bookmarkStart w:id="4" w:name="reference"/>"#,
            r#"<w:bookmarkEnd w:id="4"/>"#,
        ),
        (
            r#"<w:commentRangeStart w:id="3"/>"#,
            r#"<w:commentRangeEnd w:id="3"/>"#,
        ),
        (r#"<w:permStart w:id="2"/>"#, r#"<w:permEnd w:id="2"/>"#),
        (
            r#"<w:customXmlInsRangeStart w:id="1"/>"#,
            r#"<w:customXmlInsRangeEnd w:id="1"/>"#,
        ),
    ] {
        let input = table_fixture(&format!(
            r#"<w:document xmlns:w="{WORD}"><w:body>{start}<w:p><w:r><w:t>Protected October</w:t></w:r></w:p>{end}<w:p><w:r><w:t>Independent text</w:t></w:r></w:p></w:body></w:document>"#
        ));
        let bytes = fs::read(&input).unwrap();
        let refused = crate::execute_docx_replace_text(
            bytes.clone(),
            &operation("October", "October", "November"),
        );
        assert!(refused.output_artifact.is_none());
        assert_eq!(
            refused.operation.diagnostics[0].reason_code.as_deref(),
            Some("UNSAFE_TEXT_SELECTION")
        );
        let allowed = crate::execute_docx_replace_text(
            bytes.clone(),
            &operation("Independent text", "Independent text", "Safe edit"),
        );
        assert!(allowed.output_artifact.is_some());
        assert_eq!(fs::read(&input).unwrap(), bytes);
        fs::remove_file(input).unwrap();
    }
}

#[test]
fn replacement_refuses_pending_text_and_paragraph_property_revisions() {
    for properties in [
        r#"<w:pPr><w:pPrChange w:id="1"><w:pPr/></w:pPrChange></w:pPr><w:r>"#,
        r#"<w:r><w:rPr><w:rPrChange w:id="1"><w:rPr/></w:rPrChange></w:rPr>"#,
    ] {
        let input = table_fixture(&format!(
            r#"<w:document xmlns:w="{WORD}"><w:body><w:p>{properties}<w:t>October</w:t></w:r></w:p></w:body></w:document>"#
        ));
        let result = crate::execute_docx_replace_text(
            fs::read(&input).unwrap(),
            &operation("October", "October", "November"),
        );
        assert!(result.output_artifact.is_none());
        fs::remove_file(input).unwrap();
    }
}

#[test]
fn deletes_later_formatted_text_without_changing_any_run_properties() {
    let xml = format!(
        r#"<w:document xmlns:w="{WORD}"><w:body><w:p><w:r><w:t xml:space="preserve">Edit this sentence: </w:t></w:r><w:r><w:rPr><w:b/><w:color w:val="245678"/></w:rPr><w:t>December results</w:t></w:r><w:r><w:t>.</w:t></w:r></w:p></w:body></w:document>"#
    );
    let input = table_fixture(&xml);
    let output = path("deleted-formatting");
    let result = execute(
        &input,
        &output,
        &operation("December results.", "December results.", "January resul"),
    );
    assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
    let mut expected = payloads(&input);
    expected.insert(
        "word/document.xml".to_owned(),
        xml.replace("December results", "January resul")
            .replace("<w:t>.</w:t>", "<w:t></w:t>")
            .into_bytes(),
    );
    assert_eq!(payloads(&output), expected);
    let package = Package::open(&output).unwrap();
    package.verify().unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        crate::DocxDocument::new(&source)
            .unwrap()
            .paragraphs()
            .next()
            .unwrap()
            .text_for_view(RevisionView::Current)
            .unwrap(),
        "Edit this sentence: January resul"
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
}

#[test]
fn longer_cross_format_replacement_keeps_the_first_selected_formatting() {
    let input = fixture();
    let output = path("incompatible-formatting");
    let result = execute(
        &input,
        &output,
        &operation("Styled text", "Styled text", "A longer replacement!"),
    );
    assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
    let mut expected = payloads(&input);
    let xml = String::from_utf8(expected["word/document.xml"].clone())
        .unwrap()
        .replace("<w:t>Styled </w:t>", "<w:t>A longer replacement!</w:t>")
        .replace(
            "<w:rPr><w:b/></w:rPr><w:t>text</w:t>",
            "<w:rPr><w:b/></w:rPr><w:t></w:t>",
        );
    expected.insert("word/document.xml".to_owned(), xml.into_bytes());
    assert_eq!(payloads(&output), expected);
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
}

#[test]
fn unique_context_does_not_restyle_unchanged_text_or_the_changed_run() {
    let xml = format!(
        r#"<w:document xmlns:w="{WORD}"><w:body><w:p><w:r><w:t>Prefix </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>y</w:t></w:r><w:r><w:t> suffix</w:t></w:r></w:p><w:p><w:r><w:t>y</w:t></w:r></w:p></w:body></w:document>"#
    );
    let input = table_fixture(&xml);
    let output = path("styled-context");
    let result = execute(
        &input,
        &output,
        &operation("Prefix y suffix", "Prefix y suffix", "Prefix x suffix"),
    );
    assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
    let mut expected = payloads(&input);
    expected.insert(
        "word/document.xml".to_owned(),
        xml.replacen("<w:t>y</w:t>", "<w:t>x</w:t>", 1).into_bytes(),
    );
    assert_eq!(payloads(&output), expected);
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();
}
