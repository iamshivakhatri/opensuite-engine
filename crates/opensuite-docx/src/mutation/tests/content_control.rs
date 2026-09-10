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
