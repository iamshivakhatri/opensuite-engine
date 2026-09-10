use super::*;

/// Inserts one canonical explicit page-break paragraph at a direct body placement.
pub fn insert_page_break_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertPageBreak,
) -> Result<Vec<u8>, OperationResult> {
    let placement = resolve_paragraph_placement(source, &operation.placement)?;
    let (body, blocks) = direct_body_blocks(source)?;
    let index = placement_index(&blocks, placement)?;
    let expected = body_block_signatures(source)?;
    let insertion = body_insertion(source, body, &blocks, index)?;
    let prefix = word_prefix_for(source, body, "body")?;
    let name = |local: &str| qualify(&prefix, local);
    let fragment = format!(
        "<{}><{}><{} {}=\"page\"/></{}></{}>",
        name("p"),
        name("r"),
        name("br"),
        name("type"),
        name("r"),
        name("p")
    );
    let output = write_patches_to_vec(
        package,
        main,
        source,
        vec![Patch {
            span: SourceSpan {
                start: insertion,
                end: insertion,
            },
            replacement: fragment.into_bytes(),
        }],
    )?;
    verify_inserted_page_break_bytes(&output, index, &expected)?;
    Ok(output)
}

/// Deletes one supported canonical explicit page-break paragraph.
pub fn delete_page_break_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &DeletePageBreak,
) -> Result<Vec<u8>, OperationResult> {
    let paragraph = resolve_page_break_target(source, &operation.target)?;
    let (_, blocks) = direct_body_blocks(source)?;
    let index = blocks
        .iter()
        .position(|id| *id == paragraph)
        .ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "page break handle was not found")
        })?;
    let mut expected = body_block_signatures(source)?;
    expected.remove(index);
    let output = write_patches_to_vec(
        package,
        main,
        source,
        vec![Patch {
            span: source.node(paragraph).expect("page break exists").span(),
            replacement: Vec::new(),
        }],
    )?;
    verify_deleted_page_break_bytes(&output, &expected)?;
    Ok(output)
}

pub fn insert_page_break(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertPageBreak,
    output: impl AsRef<Path>,
) -> OperationResult {
    write_operation_output(
        package,
        output.as_ref(),
        insert_page_break_to_vec(package, main, source, operation),
    )
}

pub fn delete_page_break(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &DeletePageBreak,
    output: impl AsRef<Path>,
) -> OperationResult {
    write_operation_output(
        package,
        output.as_ref(),
        delete_page_break_to_vec(package, main, source, operation),
    )
}

pub(super) fn resolve_page_break_target(
    source: &SourceDocument,
    target: &PageBreakTarget,
) -> Result<NodeId, OperationResult> {
    let paragraph = resolve_body_block_handle(source, &target.handle)?;
    crate::inspection::page_break_paragraph(source, paragraph)
        .then_some(paragraph)
        .ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "page break handle was not found")
        })
}

pub(super) fn body_block_signatures(
    source: &SourceDocument,
) -> Result<Vec<(String, String)>, OperationResult> {
    body_texts(source)?
        .into_iter()
        .map(|(id, text)| {
            if crate::inspection::page_break_paragraph(source, id) {
                Ok(("page_break".to_owned(), String::new()))
            } else if word(source, id, "p") {
                Ok(("paragraph".to_owned(), text))
            } else {
                Ok(("table".to_owned(), text))
            }
        })
        .collect()
}

pub(super) fn verify_inserted_page_break_bytes(
    output: &[u8],
    index: usize,
    before: &[(String, String)],
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (_, blocks) = direct_body_blocks(&source)?;
    if !blocks
        .get(index)
        .is_some_and(|id| crate::inspection::page_break_paragraph(&source, *id))
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "page break is not at the requested body position",
        ));
    }
    let mut expected = before.to_vec();
    expected.insert(index, ("page_break".to_owned(), String::new()));
    (body_block_signatures(&source)? == expected)
        .then_some(())
        .ok_or_else(|| OperationResult::failed("DOCUMENT_INVALID", "output body ordering changed"))
}

pub(super) fn verify_deleted_page_break_bytes(
    output: &[u8],
    expected: &[(String, String)],
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    (body_block_signatures(&source)? == expected)
        .then_some(())
        .ok_or_else(|| OperationResult::failed("DOCUMENT_INVALID", "output body ordering changed"))
}
