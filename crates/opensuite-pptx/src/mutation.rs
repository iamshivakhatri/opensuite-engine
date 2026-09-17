use quick_xml::{escape::escape, events::Event, reader::NsReader};

use opensuite_opc::{Package, Part};
use opensuite_protocol::OperationResult;

use crate::{
    PptxInspection, ShapeKind,
    handles::{ParagraphHandle, RunHandle, parse_paragraph_handle, parse_run_handle},
    inspect_pptx, is_drawing_namespace, is_presentation_namespace, presentation_element,
    presentation_slide_ids, slide_part, top_level_shape_kind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplaceTextRun {
    pub run_handle: String,
    pub expected_current_text: String,
    pub replacement_text: String,
    pub base_revision: Option<String>,
}

/// Replaces UTF-8 byte offsets from `find_pptx_text` within one supported paragraph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplaceParagraphTextRange {
    pub paragraph_handle: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub expected_current_text: String,
    pub expected_paragraph_text: String,
    pub replacement_text: String,
    pub base_revision: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PptxExecutionResult {
    pub operation: OperationResult,
    pub output_artifact: Option<Vec<u8>>,
}

/// Replaces one directly present `a:r/a:t` value and returns reopened, verified PPTX bytes.
pub fn execute_pptx_replace_text_run(
    input_artifact: Vec<u8>,
    operation: &ReplaceTextRun,
) -> PptxExecutionResult {
    let package = match Package::from_bytes(input_artifact.clone()) {
        Ok(package) => package,
        Err(error) => return failed(error.code(), "could not load PPTX artifact"),
    };
    if let Err(error) = package.verify() {
        return failed(error.code(), "PPTX package failed validation");
    }
    let before = match inspection(input_artifact.clone()) {
        Ok(value) => value,
        Err(result) => return result,
    };
    let target = match semantic_target(&before, &operation.run_handle) {
        Ok(target) => target,
        Err(result) => return result,
    };
    let main = match package.main_office_document() {
        Ok(part) => part,
        Err(error) => return failed(error.code(), "could not find main presentation part"),
    };
    let part = match slide_part_for_index(&package, &main, target.shape.slide_index) {
        Ok(part) => part,
        Err(result) => return result,
    };
    let source = match package.read_part(&part) {
        Ok(bytes) => bytes,
        Err(error) => return failed(error.code(), "could not read slide part"),
    };
    let found = match find_run(&source, &target) {
        Ok(found) => found,
        Err(result) => return result,
    };
    if found.text != operation.expected_current_text {
        return PptxExecutionResult {
            operation: OperationResult::failed(
                "PRECONDITION_FAILED",
                "resolved run text does not match expected current text",
            )
            .with_reason_code("EXPECTED_TEXT_MISMATCH")
            .with_operation("replace_text_run")
            .with_target_handle(operation.run_handle.clone()),
            output_artifact: None,
        };
    }
    let mut patched = source;
    patched.splice(
        found.start..found.end,
        escape(&operation.replacement_text).into_owned().bytes(),
    );
    let output = match package.write_replaced_part_to_vec(&part, &patched) {
        Ok(output) => output,
        Err(error) => return failed(error.code(), error.to_string()),
    };
    if let Err(result) = verify_output(&output, &before, operation) {
        return result;
    }
    PptxExecutionResult {
        operation: OperationResult::applied(
            operation.expected_current_text.clone(),
            operation.replacement_text.clone(),
        ),
        output_artifact: Some(output),
    }
}

pub fn execute_pptx_replace_paragraph_text_range(
    input_artifact: Vec<u8>,
    operation: &ReplaceParagraphTextRange,
) -> PptxExecutionResult {
    if operation.replacement_text.contains(['\r', '\n']) {
        return failed(
            "INVALID_RANGE",
            "replacement text must not contain line breaks",
        )
        .with_target("replace_paragraph_text_range", &operation.paragraph_handle);
    }
    let package = match Package::from_bytes(input_artifact.clone()) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not load PPTX artifact"),
    };
    if let Err(error) = package.verify() {
        return failed(error.code(), "PPTX package failed validation");
    }
    let before = match inspection(input_artifact) {
        Ok(value) => value,
        Err(value) => return value,
    };
    let target = match paragraph_target(&before, &operation.paragraph_handle) {
        Ok(value) => value,
        Err(value) => return value,
    };
    let paragraph = &target.1;
    let text = paragraph
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<String>();
    if text != operation.expected_paragraph_text {
        return precondition(operation, "EXPECTED_PARAGRAPH_MISMATCH");
    }
    if operation.start_offset >= operation.end_offset
        || operation.end_offset > text.len()
        || !text.is_char_boundary(operation.start_offset)
        || !text.is_char_boundary(operation.end_offset)
    {
        return failed("INVALID_RANGE", "paragraph range is invalid")
            .with_target("replace_paragraph_text_range", &operation.paragraph_handle);
    }
    if &text[operation.start_offset..operation.end_offset] != operation.expected_current_text {
        return precondition(operation, "EXPECTED_TEXT_MISMATCH");
    }
    let main = match package.main_office_document() {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not find main presentation part"),
    };
    let part = match slide_part_for_index(&package, &main, target.0.shape.slide_index) {
        Ok(value) => value,
        Err(value) => return value,
    };
    let source = match package.read_part(&part) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not read slide part"),
    };
    let mut patches = Vec::new();
    let mut offset = 0usize;
    for (run_index, run) in paragraph.runs.iter().enumerate() {
        let end = offset + run.text.len();
        if offset < operation.end_offset && operation.start_offset < end {
            let handle = RunHandle {
                shape: target.0.shape,
                paragraph_index: target.0.paragraph_index,
                run_index,
            };
            let found = match find_run(&source, &handle) {
                Ok(value) => value,
                Err(value) => return value,
            };
            if found.text != run.text {
                return unsupported(
                    &operation.paragraph_handle,
                    "paragraph has unsupported text structure",
                );
            }
            let start = operation.start_offset.saturating_sub(offset);
            let finish = operation.end_offset.min(end) - offset;
            let mut value = String::new();
            value.push_str(&run.text[..start]);
            if operation.start_offset >= offset && operation.start_offset < end {
                value.push_str(&operation.replacement_text);
            }
            value.push_str(&run.text[finish..]);
            patches.push((found.start, found.end, value));
        }
        offset = end;
    }
    if patches.is_empty() {
        return failed("INVALID_RANGE", "paragraph range has no text runs")
            .with_target("replace_paragraph_text_range", &operation.paragraph_handle);
    }
    let mut patched = source;
    for (start, end, value) in patches.into_iter().rev() {
        patched.splice(start..end, escape(&value).into_owned().bytes());
    }
    let output = match package.write_replaced_part_to_vec(&part, &patched) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), error.to_string()),
    };
    if Package::from_bytes(output.clone())
        .and_then(|value| value.verify())
        .is_err()
    {
        return verification_range_failed(operation);
    }
    let after = match inspection(output.clone()) {
        Ok(value) => value,
        Err(_) => return verification_range_failed(operation),
    };
    let after_text = paragraph_target(&after, &operation.paragraph_handle)
        .ok()
        .map(|(_, value)| {
            value
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>()
        });
    let expected = format!(
        "{}{}{}",
        &text[..operation.start_offset],
        operation.replacement_text,
        &text[operation.end_offset..]
    );
    if after_text.as_deref() != Some(expected.as_str()) {
        return verification_range_failed(operation);
    }
    PptxExecutionResult {
        operation: OperationResult::applied(
            operation.expected_current_text.clone(),
            operation.replacement_text.clone(),
        ),
        output_artifact: Some(output),
    }
}

fn verification_range_failed(operation: &ReplaceParagraphTextRange) -> PptxExecutionResult {
    failed("DOCUMENT_INVALID", "output PPTX artifact failed validation")
        .with_target("replace_paragraph_text_range", &operation.paragraph_handle)
}

fn semantic_target(
    inspection: &PptxInspection,
    handle: &str,
) -> Result<RunHandle, PptxExecutionResult> {
    let target = parse_run_handle(handle).ok_or_else(|| target_not_found(handle))?;
    let overview = inspection
        .overview
        .as_ref()
        .ok_or_else(|| target_not_found(handle))?;
    let slide = overview
        .slides
        .get(target.shape.slide_index)
        .ok_or_else(|| target_not_found(handle))?;
    let shape = slide
        .shapes
        .get(target.shape.shape_index)
        .ok_or_else(|| target_not_found(handle))?;
    if shape.kind != ShapeKind::Shape {
        return Err(unsupported(handle, "target is not a normal text shape"));
    }
    let frame = shape
        .text_frame
        .as_ref()
        .ok_or_else(|| unsupported(handle, "target shape has no directly present text body"))?;
    let paragraph = frame
        .paragraphs
        .get(target.paragraph_index)
        .ok_or_else(|| target_not_found(handle))?;
    paragraph
        .runs
        .get(target.run_index)
        .ok_or_else(|| target_not_found(handle))?;
    Ok(target)
}

fn paragraph_target<'a>(
    inspection: &'a PptxInspection,
    handle: &str,
) -> Result<(ParagraphHandle, &'a crate::TextParagraphOverview), PptxExecutionResult> {
    let target = parse_paragraph_handle(handle).ok_or_else(|| target_not_found(handle))?;
    let overview = inspection
        .overview
        .as_ref()
        .ok_or_else(|| target_not_found(handle))?;
    let shape = overview
        .slides
        .get(target.shape.slide_index)
        .and_then(|slide| slide.shapes.get(target.shape.shape_index))
        .ok_or_else(|| target_not_found(handle))?;
    if shape.kind != ShapeKind::Shape {
        return Err(unsupported(handle, "target is not a normal text shape"));
    }
    let paragraph = shape
        .text_frame
        .as_ref()
        .and_then(|frame| frame.paragraphs.get(target.paragraph_index))
        .ok_or_else(|| target_not_found(handle))?;
    if paragraph.runs.is_empty() {
        return Err(unsupported(
            handle,
            "target paragraph has no directly present text runs",
        ));
    }
    Ok((target, paragraph))
}

fn precondition(operation: &ReplaceParagraphTextRange, reason: &str) -> PptxExecutionResult {
    PptxExecutionResult {
        operation: OperationResult::failed(
            "PRECONDITION_FAILED",
            "paragraph text does not match expected current text",
        )
        .with_reason_code(reason)
        .with_operation("replace_paragraph_text_range")
        .with_target_handle(operation.paragraph_handle.clone()),
        output_artifact: None,
    }
}

fn slide_part_for_index(
    package: &Package,
    main: &Part,
    slide_index: usize,
) -> Result<Part, PptxExecutionResult> {
    let ids = presentation_slide_ids(package, main).map_err(inspect_failure)?;
    let id = ids
        .get(slide_index)
        .ok_or_else(|| target_not_found("slide"))?;
    let relationships = package
        .part_relationships(main)
        .map_err(|error| failed(error.code(), "could not read presentation relationships"))?;
    slide_part(package, &relationships, id).map_err(inspect_failure)
}

struct LocatedRun {
    text: String,
    start: usize,
    end: usize,
}

fn find_run(source: &[u8], target: &RunHandle) -> Result<LocatedRun, PptxExecutionResult> {
    let mut reader = NsReader::from_reader(source);
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut shape_tree_depth = None;
    let mut shape_index = 0usize;
    let mut target_shape_depth = None;
    let mut text_body_depth = None;
    let mut paragraph_index = 0usize;
    let mut target_paragraph_depth = None;
    let mut run_index = 0usize;
    let mut target_run_depth = None;
    let mut target_text_depth = None;
    let mut text_count = 0usize;
    let mut located = None;
    loop {
        let (namespace, event) = reader
            .read_resolved_event_into(&mut buffer)
            .map_err(|_| unsupported_handle(target, "target slide XML cannot be parsed"))?;
        let presentation = is_presentation_namespace(&namespace);
        let drawing = is_drawing_namespace(&namespace);
        match event {
            Event::Start(element) => {
                depth += 1;
                if presentation_element(presentation, &element, "spTree") {
                    shape_tree_depth = Some(depth);
                } else if shape_tree_depth == Some(depth - 1) {
                    if let Some(kind) = top_level_shape_kind(presentation, &element) {
                        if shape_index == target.shape.shape_index && kind == ShapeKind::Shape {
                            target_shape_depth = Some(depth);
                        }
                        shape_index += 1;
                    }
                } else if target_shape_depth.is_some() {
                    if presentation_element(presentation, &element, "txBody") {
                        text_body_depth = Some(depth);
                    }
                    if text_body_depth.is_some() && drawing && element.local_name().as_ref() == b"p"
                    {
                        if paragraph_index == target.paragraph_index {
                            target_paragraph_depth = Some(depth);
                        }
                        paragraph_index += 1;
                        run_index = 0;
                    }
                    if target_paragraph_depth.is_some()
                        && drawing
                        && element.local_name().as_ref() == b"r"
                    {
                        if run_index == target.run_index {
                            target_run_depth = Some(depth);
                        }
                        run_index += 1;
                    }
                    if target_run_depth.is_some()
                        && drawing
                        && element.local_name().as_ref() == b"t"
                        && depth == target_run_depth.expect("target run depth") + 1
                    {
                        text_count += 1;
                        target_text_depth = Some(depth);
                    }
                }
            }
            Event::Text(text) if target_text_depth == Some(depth) => {
                text_count += 1;
                let end = reader.buffer_position() as usize;
                let start = end
                    .checked_sub(text.len())
                    .ok_or_else(|| unsupported_handle(target, "target text span is invalid"))?;
                let value = text
                    .unescape()
                    .map_err(|_| unsupported_handle(target, "target text is invalid"))?
                    .into_owned();
                located = Some(LocatedRun {
                    text: value,
                    start,
                    end,
                });
            }
            Event::CData(_) if target_text_depth == Some(depth) => {
                return Err(unsupported_handle(
                    target,
                    "target text uses unsupported CDATA",
                ));
            }
            Event::End(_) => {
                if target_text_depth == Some(depth) {
                    target_text_depth = None;
                }
                if target_run_depth == Some(depth) {
                    target_run_depth = None;
                }
                if target_paragraph_depth == Some(depth) {
                    target_paragraph_depth = None;
                }
                if text_body_depth == Some(depth) {
                    text_body_depth = None;
                }
                if target_shape_depth == Some(depth) {
                    target_shape_depth = None;
                }
                if shape_tree_depth == Some(depth) {
                    shape_tree_depth = None;
                }
                depth = depth.saturating_sub(1);
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if text_count != 2 {
        return Err(unsupported_handle(
            target,
            "target run must contain exactly one ordinary a:t text node",
        ));
    }
    located.ok_or_else(|| unsupported_handle(target, "target run has no replaceable text"))
}

fn verify_output(
    output: &[u8],
    before: &PptxInspection,
    operation: &ReplaceTextRun,
) -> Result<(), PptxExecutionResult> {
    let package =
        Package::from_bytes(output.to_vec()).map_err(|_| verification_failed(operation))?;
    package
        .verify()
        .map_err(|_| verification_failed(operation))?;
    let after = inspection(output.to_vec()).map_err(|_| verification_failed(operation))?;
    let before_overview = before
        .overview
        .as_ref()
        .ok_or_else(|| verification_failed(operation))?;
    let after_overview = after
        .overview
        .as_ref()
        .ok_or_else(|| verification_failed(operation))?;
    if before_overview
        .slides
        .iter()
        .map(|slide| &slide.part_name)
        .ne(after_overview.slides.iter().map(|slide| &slide.part_name))
    {
        return Err(verification_failed(operation));
    }
    let target =
        parse_run_handle(&operation.run_handle).ok_or_else(|| verification_failed(operation))?;
    let before_shape = before_overview
        .slides
        .get(target.shape.slide_index)
        .and_then(|slide| slide.shapes.get(target.shape.shape_index));
    let after_shape = after_overview
        .slides
        .get(target.shape.slide_index)
        .and_then(|slide| slide.shapes.get(target.shape.shape_index));
    if before_shape.zip(after_shape).is_none_or(|(before, after)| {
        before.handle != after.handle || before.object_id != after.object_id
    }) {
        return Err(verification_failed(operation));
    }
    let text = after_shape
        .and_then(|shape| shape.text_frame.as_ref())
        .and_then(|frame| frame.paragraphs.get(target.paragraph_index))
        .and_then(|paragraph| paragraph.runs.get(target.run_index))
        .map(|run| run.text.as_str());
    (text == Some(operation.replacement_text.as_str()))
        .then_some(())
        .ok_or_else(|| verification_failed(operation))
}

fn inspection(input: Vec<u8>) -> Result<PptxInspection, PptxExecutionResult> {
    let result = inspect_pptx(input);
    if result.diagnostics.is_empty() {
        Ok(result)
    } else {
        let diagnostic = &result.diagnostics[0];
        Err(failed(diagnostic.code.clone(), diagnostic.message.clone()))
    }
}

fn failed(code: impl Into<String>, message: impl Into<String>) -> PptxExecutionResult {
    PptxExecutionResult {
        operation: OperationResult::failed(code, message),
        output_artifact: None,
    }
}

fn target_not_found(handle: &str) -> PptxExecutionResult {
    failed("TARGET_NOT_FOUND", "text run handle was not found")
        .with_target("replace_text_run", handle)
}

fn unsupported(handle: &str, message: &str) -> PptxExecutionResult {
    failed("UNSUPPORTED_OPERATION", message).with_target("replace_text_run", handle)
}

fn unsupported_handle(target: &RunHandle, message: &str) -> PptxExecutionResult {
    unsupported(
        &format!(
            "s{}:sh{}:p{}:r{}",
            target.shape.slide_index,
            target.shape.shape_index,
            target.paragraph_index,
            target.run_index
        ),
        message,
    )
}

fn inspection_failure(value: PptxInspection) -> PptxExecutionResult {
    let diagnostic = value.diagnostics.into_iter().next();
    failed(
        diagnostic
            .as_ref()
            .map_or("DOCUMENT_INVALID", |item| item.code.as_str()),
        "PPTX inspection failed",
    )
}

fn inspect_failure(value: PptxInspection) -> PptxExecutionResult {
    inspection_failure(value)
}

fn verification_failed(operation: &ReplaceTextRun) -> PptxExecutionResult {
    failed("DOCUMENT_INVALID", "output PPTX artifact failed validation")
        .with_target("replace_text_run", &operation.run_handle)
}

trait ResultTarget {
    fn with_target(self, operation: &str, handle: &str) -> Self;
}

impl ResultTarget for PptxExecutionResult {
    fn with_target(mut self, operation: &str, handle: &str) -> Self {
        self.operation = self
            .operation
            .with_operation(operation)
            .with_target_handle(handle);
        self
    }
}
