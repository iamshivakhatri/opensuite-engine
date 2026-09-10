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
