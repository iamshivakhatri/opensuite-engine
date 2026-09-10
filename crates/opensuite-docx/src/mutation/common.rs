use super::*;

/// The effective page layout of a safe, single-section main document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageSetupInspection {
    pub margins: PageMargins,
    pub paper_size: Option<PaperSize>,
    pub orientation: PageOrientation,
}

/// The bounded semantic state of one effective default header/footer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeaderFooterInspection {
    None,
    SimpleText(String),
    SimplePageNumber(PageNumberAlignment),
    SimpleTextAndPageNumber(String, PageNumberAlignment),
    Unsupported,
}

/// The bounded semantic state of one basic PAGE field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageNumberInspection {
    None,
    Header(PageNumberAlignment),
    Footer(PageNumberAlignment),
    Unsupported,
}

pub(super) fn write_operation_output(
    package: &Package,
    output: &Path,
    result: Result<Vec<u8>, OperationResult>,
) -> OperationResult {
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let bytes = match result {
        Ok(bytes) => bytes,
        Err(error) => return error,
    };
    std::fs::write(output, bytes)
        .map(|_| OperationResult::applied(String::new(), String::new()))
        .unwrap_or_else(|error| OperationResult::failed("SERIALIZATION_FAILED", error.to_string()))
}

pub(super) fn qualify(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_owned()
    } else {
        format!("{prefix}:{local}")
    }
}

pub(super) fn attr_prefix(prefix: &str) -> String {
    if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}:")
    }
}

pub(super) fn qualified_name(
    source: &SourceDocument,
    paragraph: NodeId,
    local: &str,
) -> Result<String, OperationResult> {
    let prefix = word_prefix(source, paragraph)?;
    Ok(if prefix.is_empty() {
        local.to_owned()
    } else {
        format!("{prefix}:{local}")
    })
}

pub(super) fn temporary_path(output: &Path) -> std::path::PathBuf {
    output.with_file_name(format!(
        ".opensuite-{}-{}.docx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ))
}

pub(super) struct Patch {
    pub(super) span: SourceSpan,
    pub(super) replacement: Vec<u8>,
}

pub(super) fn placement_index(
    blocks: &[NodeId],
    placement: ResolvedParagraphPlacement,
) -> Result<usize, OperationResult> {
    match placement {
        ResolvedParagraphPlacement::Start => Ok(0),
        ResolvedParagraphPlacement::End => Ok(blocks.len()),
        ResolvedParagraphPlacement::Before(anchor) => {
            blocks.iter().position(|id| *id == anchor).ok_or_else(|| {
                OperationResult::failed("TARGET_NOT_FOUND", "body block handle was not found")
            })
        }
        ResolvedParagraphPlacement::After(anchor) => blocks
            .iter()
            .position(|id| *id == anchor)
            .map(|index| index + 1)
            .ok_or_else(|| {
                OperationResult::failed("TARGET_NOT_FOUND", "body block handle was not found")
            }),
    }
}

pub(super) fn body_insertion(
    source: &SourceDocument,
    body: NodeId,
    blocks: &[NodeId],
    index: usize,
) -> Result<usize, OperationResult> {
    if index < blocks.len() {
        Ok(source
            .node(blocks[index])
            .expect("body block exists")
            .span()
            .start)
    } else if let Some(section) = terminal_section_properties(source, body)? {
        Ok(source
            .node(section)
            .expect("section properties exist")
            .span()
            .start)
    } else {
        body_closing_start(source, body)
    }
}

pub(super) fn resolve_body_block_handle(
    source: &SourceDocument,
    handle: &str,
) -> Result<NodeId, OperationResult> {
    let index = handle
        .strip_prefix('b')
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| {
            OperationResult::failed("PRECONDITION_FAILED", "body block handle is malformed")
        })?;
    let (_, blocks) = direct_body_blocks(source)?;
    blocks.get(index).copied().ok_or_else(|| {
        OperationResult::failed("TARGET_NOT_FOUND", "body block handle was not found")
    })
}

pub(super) fn direct_body_blocks(
    source: &SourceDocument,
) -> Result<(NodeId, Vec<NodeId>), OperationResult> {
    let document = crate::DocxDocument::new(source).map_err(document_invalid)?;
    let body = document.body_id();
    Ok((
        body,
        source
            .children(body)
            .filter(|id| word(source, *id, "p") || word(source, *id, "tbl"))
            .collect(),
    ))
}

pub(super) fn terminal_section_properties(
    source: &SourceDocument,
    body: NodeId,
) -> Result<Option<NodeId>, OperationResult> {
    let children = source.children(body).collect::<Vec<_>>();
    let Some(section) = children
        .iter()
        .copied()
        .find(|id| word(source, *id, "sectPr"))
    else {
        return Ok(None);
    };
    (children.last() == Some(&section))
        .then_some(Some(section))
        .ok_or_else(|| unsupported("insert_paragraph requires terminal body section properties"))
}

pub(super) fn body_closing_start(
    source: &SourceDocument,
    body: NodeId,
) -> Result<usize, OperationResult> {
    let span = source.node(body).expect("body exists").span();
    source.original_bytes()[span.start..span.end]
        .iter()
        .rposition(|byte| *byte == b'<')
        .map(|offset| span.start + offset)
        .ok_or_else(|| unsupported("document body has no closing tag"))
}

pub(super) fn picture_fragment_for_body(
    source: &SourceDocument,
    body: NodeId,
    width_emu: i64,
    height_emu: i64,
    picture_id: u32,
    relationship_id: &str,
    alt_text: Option<&str>,
) -> Result<Vec<u8>, OperationResult> {
    let prefix = word_prefix_for(source, body, "body")?;
    let name = |local: &str| qualify(&prefix, local);
    let description = alt_text
        .filter(|value| !value.is_empty())
        .map(|value| format!(r#" descr="{}""#, escape(value)))
        .unwrap_or_default();
    let picture_name = format!("Picture {picture_id}");
    Ok(format!(
        r#"<{}><{}><{}><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{width_emu}" cy="{height_emu}"/><wp:docPr id="{picture_id}" name="{picture_name}"{description}/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="{picture_id}" name="{picture_name}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:embed="{relationship_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{width_emu}" cy="{height_emu}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></{}></{}></{}>"#,
        name("p"), name("r"), name("drawing"), name("drawing"), name("r"), name("p")
    ).into_bytes())
}

pub(super) fn word_prefix_for(
    source: &SourceDocument,
    id: NodeId,
    expected_local: &str,
) -> Result<String, OperationResult> {
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
        .strip_suffix(expected_local)
        .and_then(|value| value.strip_suffix(':'))
        .unwrap_or("");
    if name != expected_local && !name.ends_with(&format!(":{expected_local}")) {
        return Err(unsupported("source tag is invalid"));
    }
    Ok(prefix.to_owned())
}

pub(super) fn word_prefix(source: &SourceDocument, id: NodeId) -> Result<&str, OperationResult> {
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

pub(super) fn apply_patches(
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

pub(super) fn write_patches_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    patches: Vec<Patch>,
) -> Result<Vec<u8>, OperationResult> {
    let patched = apply_patches(source, patches)?;
    let output = package
        .write_replaced_part_to_vec(main, &patched)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    let verified = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    verified.verify().map_err(document_invalid)?;
    crate::open_main_source(&verified).map_err(document_invalid)?;
    Ok(output)
}

pub(super) fn unsupported(message: impl Into<String>) -> OperationResult {
    OperationResult::failed("UNSUPPORTED_OPERATION", message)
}

pub(super) fn output_matches_input(package: &Package, output: &Path) -> bool {
    package.source_path().is_some_and(|input| {
        output == input
            || output.exists() && output.canonicalize().ok() == input.canonicalize().ok()
    })
}

pub(super) fn document_invalid(error: impl std::fmt::Display) -> OperationResult {
    OperationResult::failed("DOCUMENT_INVALID", error.to_string())
}

pub(super) fn word(source: &SourceDocument, id: NodeId, local_name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name, .. }) if name.local_name() == local_name && name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}
