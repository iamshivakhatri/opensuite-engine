use super::super::*;
use super::support::*;

#[test]
fn sets_simple_content_control_text_by_tag_or_alias() {
    let xml = format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:sdt><w:sdtPr><w:tag w:val=\"customer_name\"/><w:alias w:val=\"Customer Name\"/><w:id w:val=\"7\"/><w:text/></w:sdtPr><w:sdtContent><w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr><w:r><w:t>Acme</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"
    );
    let input = table_fixture(&xml);
    let output = path("control-output");
    let result = control_execute(
        &input,
        &output,
        &control_operation(
            Some("customer_name"),
            Some("Customer Name"),
            "Acme",
            "New & < >",
        ),
    );
    assert_eq!(result.status, opensuite_protocol::OperationStatus::Applied);
    assert_eq!(result.changes[0].kind, "content_control_text_set");
    assert_eq!(
        entry(&input, "word/media/image.bin"),
        entry(&output, "word/media/image.bin")
    );
    let output_xml = String::from_utf8(entry(&output, "word/document.xml")).unwrap();
    assert!(output_xml.contains("<w:t>New &amp; &lt; &gt;</w:t>"));
    assert!(output_xml.contains("w:tag w:val=\"customer_name\""));
    Package::open(&output).unwrap().verify().unwrap();
    fs::remove_file(output).unwrap();
    let alias_output = path("control-alias");
    assert_eq!(
        control_execute(
            &input,
            &alias_output,
            &control_operation(None, Some("Customer Name"), "Acme", "Alias value")
        )
        .status,
        opensuite_protocol::OperationStatus::Applied
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(alias_output).unwrap();
}

#[test]
fn fills_empty_content_control_and_rejects_unsafe_controls() {
    let empty = format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body><w:sdt><w:sdtPr><w:tag w:val=\"name\"/><w:text/></w:sdtPr><w:sdtContent><w:p/></w:sdtContent></w:sdt></w:body></w:document>"
    );
    let input = table_fixture(&empty);
    let output = path("control-empty");
    assert_eq!(
        control_execute(
            &input,
            &output,
            &control_operation(Some("name"), None, "", " kept ")
        )
        .status,
        opensuite_protocol::OperationStatus::Applied
    );
    assert!(
        String::from_utf8(entry(&output, "word/document.xml"))
            .unwrap()
            .contains("<w:p><w:r><w:t xml:space=\"preserve\"> kept </w:t></w:r></w:p>")
    );
    fs::remove_file(input).unwrap();
    fs::remove_file(output).unwrap();

    for properties in [
        "<w:dataBinding w:xpath=\"/name\"/><w:text/>",
        "<w:lock w:val=\"contentLocked\"/><w:text/>",
        "<w:showingPlcHdr/><w:text/>",
        "<w:dropDownList/><w:text/>",
    ] {
        let xml = format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:sdt><w:sdtPr><w:tag w:val=\"name\"/>{properties}</w:sdtPr><w:sdtContent><w:p><w:r><w:t>Acme</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"
        );
        let input = table_fixture(&xml);
        let output = path("control-unsupported");
        assert_eq!(
            control_execute(
                &input,
                &output,
                &control_operation(Some("name"), None, "Acme", "New")
            )
            .diagnostics[0]
                .code,
            "UNSUPPORTED_OPERATION"
        );
        fs::remove_file(input).unwrap();
    }
}

#[test]
fn content_control_targets_require_occurrence_and_expected_text() {
    let control = "<w:sdt><w:sdtPr><w:tag w:val=\"name\"/><w:text/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Acme</w:t></w:r></w:p></w:sdtContent></w:sdt>";
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body>{control}{control}</w:body></w:document>"
    ));
    let output = path("control-ambiguous");
    let operation = control_operation(Some("name"), None, "Acme", "New");
    assert_eq!(
        control_execute(&input, &output, &operation).diagnostics[0].code,
        "TARGET_AMBIGUOUS"
    );
    let mut selected = operation.clone();
    selected.target.occurrence = Some(1);
    assert_eq!(
        control_execute(&input, &output, &selected).status,
        opensuite_protocol::OperationStatus::Applied
    );
    fs::remove_file(&output).unwrap();
    let mut wrong = control_operation(Some("name"), None, "wrong", "New");
    wrong.target.occurrence = Some(0);
    assert_eq!(
        control_execute(&input, &output, &wrong).diagnostics[0].code,
        "PRECONDITION_FAILED"
    );
    fs::remove_file(input).unwrap();
}

#[test]
fn inspection_supplies_exact_duplicate_targets_and_preserves_all_other_controls() {
    let control = |text| {
        format!(
            "<w:sdt><w:sdtPr><w:tag w:val=\"client\"/><w:alias w:val=\"Client name\"/><w:text/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:sdtContent></w:sdt>"
        )
    };
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body>{}{}</w:body></w:document>",
        control("First"),
        control("Second")
    ));
    let original = fs::read(&input).unwrap();
    let snapshot = crate::inspect_docx_content_controls(original.clone(), 0, 20);
    assert!(snapshot.ok);
    assert_eq!(snapshot.total, 2);
    assert_eq!(snapshot.controls[1].paragraph_index, Some(1));
    assert_eq!(snapshot.controls[1].text.as_deref(), Some("Second"));
    assert!(snapshot.controls[1].text_mutation_supported);
    assert_eq!(snapshot.controls[1].target_match_count, 2);
    assert!(
        snapshot.controls[1]
            .reasons
            .iter()
            .any(|r| r == "OCCURRENCE_REQUIRED")
    );
    let target = snapshot.controls[1].target.as_ref().unwrap();
    let result = crate::execute_docx_set_content_control_text(
        original,
        &SetContentControlText {
            target: ContentControlTarget {
                tag: target.tag.clone(),
                alias: target.alias.clone(),
                occurrence: Some(target.occurrence),
            },
            expected_current_text: snapshot.controls[1].text.clone().unwrap(),
            replacement: "Updated".into(),
            base_revision: None,
        },
    );
    let output = result.output_artifact.unwrap();
    let reopened = crate::inspect_docx_content_controls(output.clone(), 0, 20);
    assert_eq!(reopened.controls[0].text.as_deref(), Some("First"));
    assert_eq!(reopened.controls[1].text.as_deref(), Some("Updated"));
    let package = Package::from_bytes(output).unwrap();
    let before = Package::open(&input).unwrap();
    for name in before
        .part_names_with_prefix("/")
        .filter(|n| n.as_str() != "/word/document.xml")
    {
        assert_eq!(
            before.read_part_by_name(name).unwrap(),
            package.read_part_by_name(name).unwrap()
        );
    }
    fs::remove_file(input).unwrap();
}

#[test]
fn inspection_reports_unsupported_and_unnamed_controls_without_changing_xml() {
    for (properties, content) in [
        ("<w:tag w:val=\"x\"/><w:date/>", "<w:p/>"),
        (
            "<w:tag w:val=\"x\"/><w:text/><w:lock w:val=\"contentLocked\"/>",
            "<w:p/>",
        ),
        ("<w:tag w:val=\"x\"/><w:text/><w:dataBinding/>", "<w:p/>"),
        ("<w:tag w:val=\"x\"/><w:text/><w:showingPlcHdr/>", "<w:p/>"),
        ("<w:tag w:val=\"x\"/><w:text/>", "<w:p/><w:p/>"),
        (
            "<w:tag w:val=\"x\"/><w:text/>",
            "<w:r><w:t>Inline</w:t></w:r>",
        ),
        ("<w:text/>", "<w:p/>"),
    ] {
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:sdt><w:sdtPr>{properties}</w:sdtPr><w:sdtContent>{content}</w:sdtContent></w:sdt></w:body></w:document>"
        ));
        let original = fs::read(&input).unwrap();
        let snapshot = crate::inspect_docx_content_controls(original.clone(), 0, 20);
        assert!(snapshot.ok);
        assert!(
            !snapshot.controls[0].text_mutation_supported,
            "{properties}"
        );
        assert!(!snapshot.controls[0].reasons.is_empty());
        assert_eq!(fs::read(&input).unwrap(), original);
        fs::remove_file(input).unwrap();
    }
    assert!(!crate::inspect_docx_content_controls(vec![], 0, 20).ok);
}

#[test]
fn inspection_bounds_unicode_values_pages_and_uses_current_text() {
    let mut controls = String::new();
    for i in 0..105 {
        write!(controls, "<w:sdt><w:sdtPr><w:tag w:val=\"{i}\"/><w:text/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>{}</w:t></w:r></w:p></w:sdtContent></w:sdt>", "界".repeat(2001)).unwrap();
    }
    let input = table_fixture(&format!(
        "<w:document xmlns:w=\"{WORD}\"><w:body>{}<w:sdt><w:sdtPr><w:alias w:val=\"Only alias\"/><w:text/></w:sdtPr><w:sdtContent><w:p><w:del><w:r><w:t>Hidden</w:t></w:r></w:del><w:r><w:t>Visible</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>",
        controls
    ));
    let bytes = fs::read(&input).unwrap();
    let snapshot = crate::inspect_docx_content_controls(bytes.clone(), 0, usize::MAX);
    assert_eq!(snapshot.controls.len(), 100);
    assert!(snapshot.has_more);
    assert_eq!(
        snapshot.controls[0].text.as_ref().unwrap().chars().count(),
        2000
    );
    assert!(snapshot.controls[0].truncated);
    assert!(!snapshot.controls[0].text_mutation_supported);
    let page = crate::inspect_docx_content_controls(bytes.clone(), 105, 20);
    assert!(!page.has_more);
    assert_eq!(page.controls[0].text.as_deref(), Some("Visible"));
    assert_eq!(
        page.controls[0].target.as_ref().unwrap().alias.as_deref(),
        Some("Only alias")
    );
    assert!(
        crate::inspect_docx_content_controls(bytes, usize::MAX, 20)
            .controls
            .is_empty()
    );
    fs::remove_file(input).unwrap();
}

#[test]
fn inspection_checks_the_actual_text_patch_guards_for_mixed_formatting_and_hidden_boundaries() {
    for content in [
        "<w:r><w:rPr><w:b/></w:rPr><w:t>Mixed</w:t></w:r><w:r><w:t> text</w:t></w:r>",
        "<w:r><w:t>Mixed</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t> text</w:t></w:r>",
    ] {
        let input = table_fixture(&format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:sdt><w:sdtPr><w:tag w:val=\"mixed\"/><w:richText/></w:sdtPr><w:sdtContent><w:p>{content}</w:p></w:sdtContent></w:sdt></w:body></w:document>"
        ));
        let bytes = fs::read(&input).unwrap();
        let inspected = crate::inspect_docx_content_controls(bytes.clone(), 0, 20);
        assert!(!inspected.controls[0].text_mutation_supported);
        assert!(!inspected.controls[0].reasons.is_empty());
        let result = crate::execute_docx_set_content_control_text(
            bytes,
            &control_operation(Some("mixed"), None, "Mixed text", "New"),
        );
        assert!(result.output_artifact.is_none());
        fs::remove_file(input).unwrap();
    }
}
