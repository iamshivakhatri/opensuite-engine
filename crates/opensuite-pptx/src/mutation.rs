use std::collections::HashSet;

use quick_xml::{escape::escape, events::Event, reader::NsReader};

use opensuite_opc::{Package, Part, PartName, RelationshipTarget};
use opensuite_protocol::OperationResult;

use crate::{
    DirectRunFormatting, PptxInspection, ShapeGeometry, ShapeKind,
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
pub struct SetShapeGeometry {
    pub shape_handle: String,
    pub expected_current_geometry: ShapeGeometry,
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    pub base_revision: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetTextRunFormatting {
    pub run_handle: String,
    pub expected_current_formatting: DirectRunFormatting,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub font_size: Option<i64>,
    pub typeface: Option<String>,
    pub color: Option<String>,
    pub base_revision: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplacePicture {
    pub shape_handle: String,
    pub replacement_image: Vec<u8>,
    pub expected_current_media_part: String,
    pub expected_current_content_type: String,
    pub base_revision: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsertPicture {
    pub slide_handle: String,
    pub image: Vec<u8>,
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    pub name: Option<String>,
    pub base_revision: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PptxExecutionResult {
    pub operation: OperationResult,
    pub output_artifact: Option<Vec<u8>>,
    pub created_picture_handle: Option<String>,
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
            created_picture_handle: None,
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
        created_picture_handle: None,
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
        created_picture_handle: None,
    }
}

pub fn execute_pptx_set_shape_geometry(
    input_artifact: Vec<u8>,
    operation: &SetShapeGeometry,
) -> PptxExecutionResult {
    if operation.width <= 0 || operation.height <= 0 {
        return failed("INVALID_GEOMETRY", "width and height must be positive")
            .with_target("set_shape_geometry", &operation.shape_handle);
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
    let target = match crate::handles::parse_shape_handle(&operation.shape_handle) {
        Some(value) => value,
        None => return target_not_found(&operation.shape_handle),
    };
    let shape = match before
        .overview
        .as_ref()
        .and_then(|value| value.slides.get(target.slide_index))
        .and_then(|slide| slide.shapes.get(target.shape_index))
    {
        Some(value) => value,
        None => return target_not_found(&operation.shape_handle),
    };
    if !matches!(
        shape.kind,
        ShapeKind::Shape | ShapeKind::Picture | ShapeKind::GraphicFrame
    ) {
        return unsupported(
            &operation.shape_handle,
            "shape type has no supported direct transform",
        );
    }
    if shape.geometry.as_ref() != Some(&operation.expected_current_geometry) {
        return failed(
            "PRECONDITION_FAILED",
            "shape geometry does not match expected current geometry",
        )
        .with_target("set_shape_geometry", &operation.shape_handle);
    }
    if !has_direct_geometry(&operation.expected_current_geometry) {
        return unsupported(
            &operation.shape_handle,
            "shape has no direct transform geometry",
        );
    }
    let main = match package.main_office_document() {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not find main presentation part"),
    };
    let part = match slide_part_for_index(&package, &main, target.slide_index) {
        Ok(value) => value,
        Err(value) => return value,
    };
    let source = match package.read_part(&part) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not read slide part"),
    };
    let spans = match find_geometry_attributes(&source, target) {
        Some(value) => value,
        None => {
            return unsupported(
                &operation.shape_handle,
                "shape direct transform is malformed",
            );
        }
    };
    let mut patched = source;
    for ((start, end), value) in [
        (spans.0, operation.x),
        (spans.1, operation.y),
        (spans.2, operation.width),
        (spans.3, operation.height),
    ]
    .into_iter()
    .rev()
    {
        patched.splice(start..end, value.to_string().bytes());
    }
    let output = match package.write_replaced_part_to_vec(&part, &patched) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), error.to_string()),
    };
    let after = match inspection(output.clone()) {
        Ok(value) => value,
        Err(_) => return geometry_verification_failed(operation),
    };
    let expected = ShapeGeometry {
        x: Some(operation.x),
        y: Some(operation.y),
        width: Some(operation.width),
        height: Some(operation.height),
        rotation: operation.expected_current_geometry.rotation,
        flip_horizontal: operation.expected_current_geometry.flip_horizontal,
        flip_vertical: operation.expected_current_geometry.flip_vertical,
    };
    let actual = after
        .overview
        .as_ref()
        .and_then(|value| value.slides.get(target.slide_index))
        .and_then(|slide| slide.shapes.get(target.shape_index));
    if actual.is_none_or(|value| {
        value.handle != operation.shape_handle || value.geometry.as_ref() != Some(&expected)
    }) {
        return geometry_verification_failed(operation);
    }
    PptxExecutionResult {
        operation: OperationResult::applied("geometry".to_owned(), "geometry".to_owned()),
        output_artifact: Some(output),
        created_picture_handle: None,
    }
}

fn verification_range_failed(operation: &ReplaceParagraphTextRange) -> PptxExecutionResult {
    failed("DOCUMENT_INVALID", "output PPTX artifact failed validation")
        .with_target("replace_paragraph_text_range", &operation.paragraph_handle)
}

pub fn execute_pptx_set_text_run_formatting(
    input_artifact: Vec<u8>,
    operation: &SetTextRunFormatting,
) -> PptxExecutionResult {
    if operation.font_size.is_some_and(|value| value <= 0)
        || operation.color.as_deref().is_some_and(|value| {
            value.len() != 6 || !value.bytes().all(|value| value.is_ascii_hexdigit())
        })
        || operation.typeface.as_deref().is_some_and(invalid_typeface)
    {
        return failed(
            "INVALID_FORMATTING",
            "requested direct formatting is invalid",
        )
        .with_target("set_text_run_formatting", &operation.run_handle);
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
    let target = match semantic_target(&before, &operation.run_handle) {
        Ok(value) => value,
        Err(value) => return value,
    };
    let run = &before.overview.as_ref().unwrap().slides[target.shape.slide_index].shapes
        [target.shape.shape_index]
        .text_frame
        .as_ref()
        .unwrap()
        .paragraphs[target.paragraph_index]
        .runs[target.run_index];
    if run.formatting != operation.expected_current_formatting {
        return failed(
            "PRECONDITION_FAILED",
            "run formatting does not match expected current formatting",
        )
        .with_target("set_text_run_formatting", &operation.run_handle);
    }
    let main = match package.main_office_document() {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not find main presentation part"),
    };
    let part = match slide_part_for_index(&package, &main, target.shape.slide_index) {
        Ok(value) => value,
        Err(value) => return value,
    };
    let source = match package.read_part(&part) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not read slide part"),
    };
    let found = match find_run(&source, &target) {
        Ok(value) => value,
        Err(value) => return value,
    };
    if found.text != run.text {
        return unsupported(&operation.run_handle, "run has unsupported text structure");
    }
    let patched = match patch_run_formatting(source, &found, operation) {
        Some(value) => value,
        None => {
            return unsupported(
                &operation.run_handle,
                "run formatting structure is unsupported",
            );
        }
    };
    let output = match package.write_replaced_part_to_vec(&part, &patched) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), error.to_string()),
    };
    if Package::from_bytes(output.clone())
        .and_then(|value| value.verify())
        .is_err()
    {
        return formatting_verification_failed(operation);
    }
    let after = match inspection(output.clone()) {
        Ok(value) => value,
        Err(_) => return formatting_verification_failed(operation),
    };
    let after_run = after
        .overview
        .as_ref()
        .and_then(|value| value.slides.get(target.shape.slide_index))
        .and_then(|slide| slide.shapes.get(target.shape.shape_index))
        .and_then(|shape| shape.text_frame.as_ref())
        .and_then(|frame| frame.paragraphs.get(target.paragraph_index))
        .and_then(|paragraph| paragraph.runs.get(target.run_index));
    let expected = requested_formatting(&operation.expected_current_formatting, operation);
    if after_run.is_none_or(|value| value.text != run.text || value.formatting != expected) {
        return formatting_verification_failed(operation);
    }
    PptxExecutionResult {
        operation: OperationResult::applied("formatting".to_owned(), "formatting".to_owned()),
        output_artifact: Some(output),
        created_picture_handle: None,
    }
}

pub fn execute_pptx_replace_picture(
    input_artifact: Vec<u8>,
    operation: &ReplacePicture,
) -> PptxExecutionResult {
    let Some((extension, content_type)) = image_format(&operation.replacement_image) else {
        return failed("UNSUPPORTED_IMAGE", "replacement image must be PNG or JPEG")
            .with_target("replace_picture", &operation.shape_handle);
    };
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
    let target = match crate::handles::parse_shape_handle(&operation.shape_handle) {
        Some(value) => value,
        None => return target_not_found(&operation.shape_handle),
    };
    let shape = match before
        .overview
        .as_ref()
        .and_then(|value| value.slides.get(target.slide_index))
        .and_then(|slide| slide.shapes.get(target.shape_index))
    {
        Some(value) => value,
        None => return target_not_found(&operation.shape_handle),
    };
    if shape.kind != ShapeKind::Picture {
        return unsupported(&operation.shape_handle, "target is not a top-level picture");
    }
    let main = match package.main_office_document() {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not find main presentation part"),
    };
    let slide = match slide_part_for_index(&package, &main, target.slide_index) {
        Ok(value) => value,
        Err(value) => return value,
    };
    let source = match package.read_part(&slide) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not read slide part"),
    };
    let (embed_span, old_relationship) = match find_picture_embed(&source, target) {
        Some(value) => value,
        None => {
            return unsupported(
                &operation.shape_handle,
                "picture has no direct embedded image reference",
            );
        }
    };
    let relationships = match package.part_relationships(&slide) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not read slide relationships"),
    };
    let old = match relationships
        .iter()
        .find(|value| value.id.as_str() == old_relationship)
    {
        Some(value) => value,
        None => {
            return failed(
                "MISSING_PICTURE_RELATIONSHIP",
                "picture image relationship is missing",
            );
        }
    };
    let RelationshipTarget::Internal {
        part_name: old_part,
        ..
    } = &old.target
    else {
        return unsupported(&operation.shape_handle, "linked pictures are unsupported");
    };
    let old_content_type = match package.content_type(old_part) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "picture media content type is missing"),
    };
    if old_part.as_str() != operation.expected_current_media_part
        || old_content_type.as_str() != operation.expected_current_content_type
    {
        return failed(
            "PRECONDITION_FAILED",
            "picture media does not match expected current metadata",
        )
        .with_target("replace_picture", &operation.shape_handle);
    }
    let new_part = allocate_media_part(&package, extension);
    let new_id = allocate_relationship_id(&relationships);
    let rel_part = relationship_part_name(&slide.name);
    let rel_source = match package
        .part(&rel_part)
        .and_then(|part| package.read_part(&part))
    {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not read slide relationship part"),
    };
    let rel_patched = match insert_relationship(rel_source, &new_id, new_part.as_str()) {
        Some(value) => value,
        None => {
            return failed(
                "MALFORMED_RELATIONSHIPS",
                "could not add picture relationship",
            );
        }
    };
    let mut slide_patched = source;
    slide_patched.splice(embed_span.0..embed_span.1, new_id.bytes());
    let types_name = PartName::parse("/[Content_Types].xml").expect("constant part name");
    let types_part = match package.part(&types_name) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "content types part is missing"),
    };
    let types_source = match package.read_part(&types_part) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not read content types"),
    };
    let types_patched = ensure_content_type(types_source, extension, content_type);
    let output = match package.write_package_with_named_changes_to_vec(
        &[
            (slide.name.clone(), &slide_patched),
            (rel_part, &rel_patched),
            (types_name, &types_patched),
        ],
        &[(new_part.clone(), &operation.replacement_image)],
    ) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), error.to_string()),
    };
    let reopened = match Package::from_bytes(output.clone()) {
        Ok(value) if value.verify().is_ok() => value,
        _ => return picture_verification_failed(operation),
    };
    let new_media = reopened
        .part(&new_part)
        .and_then(|part| reopened.read_part(&part));
    if new_media.ok().as_deref() != Some(operation.replacement_image.as_slice()) {
        return picture_verification_failed(operation);
    }
    PptxExecutionResult {
        operation: OperationResult::applied(
            operation.expected_current_media_part.clone(),
            new_part.as_str().to_owned(),
        ),
        output_artifact: Some(output),
        created_picture_handle: None,
    }
}

/// Inserts one top-level picture as the last drawable child of a slide shape tree.
pub fn execute_pptx_insert_picture(
    input_artifact: Vec<u8>,
    operation: &InsertPicture,
) -> PptxExecutionResult {
    let Some((extension, content_type)) = image_format(&operation.image) else {
        return failed("UNSUPPORTED_IMAGE", "picture must be PNG or JPEG")
            .with_target("insert_picture", &operation.slide_handle);
    };
    if operation.width <= 0 || operation.height <= 0 {
        return failed("INVALID_GEOMETRY", "width and height must be positive")
            .with_target("insert_picture", &operation.slide_handle);
    }
    if operation.name.as_deref().is_some_and(invalid_picture_name) {
        return failed("INVALID_PICTURE_NAME", "picture name is invalid")
            .with_target("insert_picture", &operation.slide_handle);
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
    let Some(slide_index) = parse_slide_handle(&operation.slide_handle) else {
        return insert_picture_target_not_found(operation);
    };
    let Some(before_slide) = before
        .overview
        .as_ref()
        .and_then(|value| value.slides.get(slide_index))
        .filter(|slide| slide.handle == operation.slide_handle)
    else {
        return insert_picture_target_not_found(operation);
    };
    let main = match package.main_office_document() {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not find main presentation part"),
    };
    let slide = match slide_part_for_index(&package, &main, slide_index) {
        Ok(value) => value,
        Err(value) => return value,
    };
    let source = match package.read_part(&slide) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not read slide part"),
    };
    let insertion = match slide_picture_insertion(&source) {
        Some(value) => value,
        None => {
            return unsupported(
                &operation.slide_handle,
                "slide has no supported editable shape tree",
            );
        }
    };
    let metadata = match slide_nonvisual_metadata(&source) {
        Some(value) => value,
        None => {
            return unsupported(
                &operation.slide_handle,
                "slide nonvisual properties cannot be read",
            );
        }
    };
    let object_id = allocate_object_id(&metadata.ids);
    let name = operation
        .name
        .clone()
        .unwrap_or_else(|| allocate_picture_name(&metadata.names));
    let relationships = match package.part_relationships(&slide) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not read slide relationships"),
    };
    let new_part = allocate_media_part(&package, extension);
    let relationship_id = allocate_relationship_id(&relationships);
    let rel_part = relationship_part_name(&slide.name);
    let rel_source = match package
        .part(&rel_part)
        .and_then(|part| package.read_part(&part))
    {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not read slide relationship part"),
    };
    let rel_patched = match insert_relationship(rel_source, &relationship_id, new_part.as_str()) {
        Some(value) => value,
        None => {
            return failed(
                "MALFORMED_RELATIONSHIPS",
                "could not add picture relationship",
            );
        }
    };
    let picture = picture_xml(
        object_id,
        &name,
        &relationship_id,
        operation.x,
        operation.y,
        operation.width,
        operation.height,
    );
    let mut slide_patched = source;
    slide_patched.splice(insertion.position..insertion.position, picture.bytes());
    let types_name = PartName::parse("/[Content_Types].xml").expect("constant part name");
    let types_part = match package.part(&types_name) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "content types part is missing"),
    };
    let types_source = match package.read_part(&types_part) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), "could not read content types"),
    };
    let types_patched = ensure_content_type(types_source, extension, content_type);
    let output = match package.write_package_with_named_changes_to_vec(
        &[
            (slide.name.clone(), &slide_patched),
            (rel_part, &rel_patched),
            (types_name, &types_patched),
        ],
        &[(new_part.clone(), &operation.image)],
    ) {
        Ok(value) => value,
        Err(error) => return failed(error.code(), error.to_string()),
    };
    let handle = format!("s{slide_index}:sh{}", before_slide.shapes.len());
    if !verify_inserted_picture(
        &output,
        &before,
        operation,
        &slide,
        &new_part,
        &relationship_id,
        &handle,
        object_id,
        &name,
        content_type,
    ) {
        return insert_picture_verification_failed(operation);
    }
    PptxExecutionResult {
        operation: OperationResult::applied(operation.slide_handle.clone(), handle.clone()),
        output_artifact: Some(output),
        created_picture_handle: Some(handle),
    }
}

fn has_direct_geometry(value: &ShapeGeometry) -> bool {
    value.x.is_some() && value.y.is_some() && value.width.is_some() && value.height.is_some()
}

fn find_geometry_attributes(
    source: &[u8],
    target: crate::handles::ShapeHandle,
) -> Option<(
    (usize, usize),
    (usize, usize),
    (usize, usize),
    (usize, usize),
)> {
    let mut reader = NsReader::from_reader(source);
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut tree = None;
    let mut index = 0usize;
    let mut shape = None;
    let mut xfrm = None;
    let mut values = [None; 4];
    loop {
        let (namespace, event) = reader.read_resolved_event_into(&mut buffer).ok()?;
        let presentation = is_presentation_namespace(&namespace);
        match event {
            Event::Start(element) => {
                depth += 1;
                if presentation_element(presentation, &element, "spTree") {
                    tree = Some(depth);
                } else if tree == Some(depth - 1) {
                    if let Some(kind) = top_level_shape_kind(presentation, &element) {
                        if index == target.shape_index
                            && matches!(
                                kind,
                                ShapeKind::Shape | ShapeKind::Picture | ShapeKind::GraphicFrame
                            )
                        {
                            shape = Some(depth);
                        }
                        index += 1;
                    }
                } else if shape.is_some()
                    && element.local_name().as_ref() == b"xfrm"
                    && depth <= shape? + 2
                {
                    xfrm = Some(depth);
                }
            }
            Event::Empty(element) => {
                if tree == Some(depth) {
                    if let Some(kind) = top_level_shape_kind(presentation, &element) {
                        if index == target.shape_index
                            && matches!(
                                kind,
                                ShapeKind::Shape | ShapeKind::Picture | ShapeKind::GraphicFrame
                            )
                        {
                            shape = Some(depth);
                        }
                        index += 1;
                    }
                }
                if xfrm == Some(depth) && element.local_name().as_ref() == b"off" {
                    let end = reader.buffer_position() as usize;
                    let start = end.checked_sub(element.as_ref().len())?;
                    values[0] = raw_attribute_span(&source[start..end], start, b"x");
                    values[1] = raw_attribute_span(&source[start..end], start, b"y");
                }
                if xfrm == Some(depth) && element.local_name().as_ref() == b"ext" {
                    let end = reader.buffer_position() as usize;
                    let start = end.checked_sub(element.as_ref().len())?;
                    values[2] = raw_attribute_span(&source[start..end], start, b"cx");
                    values[3] = raw_attribute_span(&source[start..end], start, b"cy");
                }
            }
            Event::End(_) => {
                if xfrm == Some(depth) {
                    xfrm = None;
                }
                if shape == Some(depth) {
                    shape = None;
                }
                if tree == Some(depth) {
                    tree = None;
                }
                depth = depth.saturating_sub(1);
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Some((values[0]?, values[1]?, values[2]?, values[3]?))
}

fn raw_attribute_span(raw: &[u8], absolute: usize, name: &[u8]) -> Option<(usize, usize)> {
    let needle = [name, b"=\""].concat();
    let start = raw
        .windows(needle.len())
        .position(|value| value == needle)?
        + needle.len();
    let end = start + raw[start..].iter().position(|byte| *byte == b'\"')?;
    Some((absolute + start, absolute + end))
}

fn geometry_verification_failed(operation: &SetShapeGeometry) -> PptxExecutionResult {
    failed("DOCUMENT_INVALID", "output PPTX geometry failed validation")
        .with_target("set_shape_geometry", &operation.shape_handle)
}

fn invalid_typeface(value: &str) -> bool {
    value.is_empty()
        || value
            .chars()
            .any(|value| value.is_control() || matches!(value, '<' | '>' | '&' | '\"' | '\''))
}

fn image_format(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    bytes
        .starts_with(b"\x89PNG\r\n\x1a\n")
        .then_some(("png", "image/png"))
        .or_else(|| (bytes.starts_with(&[0xff, 0xd8, 0xff])).then_some(("jpeg", "image/jpeg")))
}
fn allocate_media_part(package: &Package, extension: &str) -> PartName {
    for index in 1usize.. {
        let name = PartName::parse(format!("/ppt/media/image{index}.{extension}"))
            .expect("generated media name");
        if package.part(&name).is_err() {
            return name;
        }
    }
    unreachable!()
}
fn allocate_relationship_id(relationships: &[opensuite_opc::Relationship]) -> String {
    for index in 1usize.. {
        let value = format!("rId{index}");
        if relationships.iter().all(|item| item.id.as_str() != value) {
            return value;
        }
    }
    unreachable!()
}
fn relationship_part_name(part: &PartName) -> PartName {
    let value = part.as_str().trim_start_matches('/');
    let (parent, name) = value.rsplit_once('/').unwrap_or(("", value));
    PartName::parse(format!("/{parent}/_rels/{name}.rels")).expect("known slide part")
}
fn insert_relationship(source: Vec<u8>, id: &str, media_part: &str) -> Option<Vec<u8>> {
    let mut value = String::from_utf8(source).ok()?;
    let insertion = format!(
        "<Relationship Id=\"{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/{}\"/>",
        media_part.rsplit('/').next().expect("media name")
    );
    if let Some(position) = value.rfind("</Relationships>") {
        value.insert_str(position, &insertion);
    } else {
        let position = value.rfind("/>")?;
        value.replace_range(
            position..position + 2,
            &format!(">{insertion}</Relationships>"),
        );
    }
    Some(value.into_bytes())
}
fn ensure_content_type(source: Vec<u8>, extension: &str, content_type: &str) -> Vec<u8> {
    let value = String::from_utf8(source).expect("content types XML is UTF-8");
    if value.contains(&format!("Extension=\"{extension}\"")) {
        return value.into_bytes();
    }
    let position = value.rfind("</Types>").expect("well-formed content types");
    let mut result = value;
    result.insert_str(
        position,
        &format!("<Default Extension=\"{extension}\" ContentType=\"{content_type}\"/>"),
    );
    result.into_bytes()
}
fn find_picture_embed(
    source: &[u8],
    target: crate::handles::ShapeHandle,
) -> Option<((usize, usize), String)> {
    let mut reader = NsReader::from_reader(source);
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut tree = None;
    let mut index = 0usize;
    let mut picture = None;
    loop {
        let (namespace, event) = reader.read_resolved_event_into(&mut buffer).ok()?;
        let presentation = is_presentation_namespace(&namespace);
        let drawing = is_drawing_namespace(&namespace);
        match event {
            Event::Start(element) => {
                depth += 1;
                if presentation_element(presentation, &element, "spTree") {
                    tree = Some(depth);
                } else if tree == Some(depth - 1) {
                    if let Some(kind) = top_level_shape_kind(presentation, &element) {
                        if index == target.shape_index && kind == ShapeKind::Picture {
                            picture = Some(depth);
                        }
                        index += 1;
                    }
                }
            }
            Event::Empty(element)
                if picture.is_some() && drawing && element.local_name().as_ref() == b"blip" =>
            {
                let end = reader.buffer_position() as usize;
                let start = end.checked_sub(element.as_ref().len())?;
                let raw = &source[start..end];
                let span = raw_attribute_span(raw, start, b"r:embed")
                    .or_else(|| raw_attribute_span(raw, start, b"embed"))?;
                return Some((
                    span,
                    String::from_utf8(raw[span.0 - start..span.1 - start].to_vec()).ok()?,
                ));
            }
            Event::End(_) => {
                if picture == Some(depth) {
                    picture = None;
                }
                if tree == Some(depth) {
                    tree = None;
                }
                depth = depth.saturating_sub(1);
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    None
}
fn picture_verification_failed(operation: &ReplacePicture) -> PptxExecutionResult {
    failed("DOCUMENT_INVALID", "output PPTX picture failed validation")
        .with_target("replace_picture", &operation.shape_handle)
}

fn parse_slide_handle(value: &str) -> Option<usize> {
    value.strip_prefix('s')?.parse().ok()
}

fn invalid_picture_name(value: &str) -> bool {
    value.is_empty() || value.chars().any(char::is_control)
}

struct SlideInsertion {
    position: usize,
}

struct SlideNonvisualMetadata {
    ids: HashSet<u32>,
    names: HashSet<String>,
}

fn slide_picture_insertion(source: &[u8]) -> Option<SlideInsertion> {
    let mut reader = NsReader::from_reader(source);
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut tree = None;
    let mut has_nonvisual_group = false;
    let mut has_group_properties = false;
    let mut extension_start = None;
    let mut drawable_after_extension = false;
    loop {
        let (namespace, event) = reader.read_resolved_event_into(&mut buffer).ok()?;
        let presentation = is_presentation_namespace(&namespace);
        match event {
            Event::Start(element) => {
                depth += 1;
                if presentation_element(presentation, &element, "spTree") {
                    tree = Some(depth);
                } else if tree == Some(depth - 1) {
                    let end = reader.buffer_position() as usize;
                    let start = end.checked_sub(element.as_ref().len() + 2)?;
                    if presentation_element(presentation, &element, "nvGrpSpPr") {
                        has_nonvisual_group = true;
                    } else if presentation_element(presentation, &element, "grpSpPr") {
                        has_group_properties = true;
                    } else if presentation_element(presentation, &element, "extLst") {
                        extension_start = Some(start);
                    } else if extension_start.is_some() {
                        drawable_after_extension = true;
                    }
                }
            }
            Event::Empty(element) if tree == Some(depth) => {
                let end = reader.buffer_position() as usize;
                let start = end.checked_sub(element.as_ref().len() + 3)?;
                if presentation_element(presentation, &element, "nvGrpSpPr") {
                    has_nonvisual_group = true;
                } else if presentation_element(presentation, &element, "grpSpPr") {
                    has_group_properties = true;
                } else if presentation_element(presentation, &element, "extLst") {
                    extension_start = Some(start);
                } else if extension_start.is_some() {
                    drawable_after_extension = true;
                }
            }
            Event::End(element) => {
                if tree == Some(depth) && presentation && element.local_name().as_ref() == b"spTree"
                {
                    let end = reader.buffer_position() as usize;
                    let close_start = end.checked_sub(element.as_ref().len() + 3)?;
                    return (has_nonvisual_group
                        && has_group_properties
                        && !drawable_after_extension)
                        .then_some(SlideInsertion {
                            position: extension_start.unwrap_or(close_start),
                        });
                }
                if tree == Some(depth) {
                    tree = None;
                }
                depth = depth.saturating_sub(1);
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    None
}

fn slide_nonvisual_metadata(source: &[u8]) -> Option<SlideNonvisualMetadata> {
    let mut reader = NsReader::from_reader(source);
    let mut buffer = Vec::new();
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    loop {
        let (namespace, event) = reader.read_resolved_event_into(&mut buffer).ok()?;
        let presentation = is_presentation_namespace(&namespace);
        let element = match event {
            Event::Start(element) | Event::Empty(element) => element,
            Event::Eof => break,
            _ => {
                buffer.clear();
                continue;
            }
        };
        if presentation_element(presentation, &element, "cNvPr") {
            if let Some(id) = crate::optional_attribute(&element, b"id")
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|value| *value > 0)
            {
                ids.insert(id);
            }
            if let Some(name) = crate::optional_attribute(&element, b"name") {
                names.insert(name);
            }
        }
        buffer.clear();
    }
    Some(SlideNonvisualMetadata { ids, names })
}

fn allocate_object_id(ids: &HashSet<u32>) -> u32 {
    (1..).find(|value| !ids.contains(value)).unwrap_or(u32::MAX)
}

fn allocate_picture_name(names: &HashSet<String>) -> String {
    for index in 1usize.. {
        let name = format!("Picture {index}");
        if !names.contains(&name) {
            return name;
        }
    }
    unreachable!()
}

fn picture_xml(
    object_id: u32,
    name: &str,
    relationship_id: &str,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
) -> String {
    let name = escape(name).into_owned();
    format!(
        "<p:pic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><p:nvPicPr><p:cNvPr id=\"{object_id}\" name=\"{name}\"/><p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed=\"{relationship_id}\"/><a:srcRect/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{width}\" cy=\"{height}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>"
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_inserted_picture(
    output: &[u8],
    before: &PptxInspection,
    operation: &InsertPicture,
    slide: &Part,
    media_part: &PartName,
    relationship_id: &str,
    handle: &str,
    object_id: u32,
    name: &str,
    content_type: &str,
) -> bool {
    let Ok(package) = Package::from_bytes(output.to_vec()) else {
        return false;
    };
    if package.verify().is_err() {
        return false;
    }
    let Ok(after) = inspection(output.to_vec()) else {
        return false;
    };
    let Some(before_overview) = before.overview.as_ref() else {
        return false;
    };
    let Some(after_overview) = after.overview.as_ref() else {
        return false;
    };
    if before_overview
        .slides
        .iter()
        .map(|value| &value.part_name)
        .ne(after_overview.slides.iter().map(|value| &value.part_name))
    {
        return false;
    }
    let Some(slide_index) = parse_slide_handle(&operation.slide_handle) else {
        return false;
    };
    let Some(before_slide) = before_overview.slides.get(slide_index) else {
        return false;
    };
    let Some(after_slide) = after_overview.slides.get(slide_index) else {
        return false;
    };
    if after_slide.shapes.len() != before_slide.shapes.len() + 1
        || after_slide.shapes[..before_slide.shapes.len()] != before_slide.shapes
    {
        return false;
    }
    let Some(picture) = after_slide.shapes.last() else {
        return false;
    };
    if picture.handle != handle
        || picture.kind != ShapeKind::Picture
        || picture.object_id.as_deref() != Some(&object_id.to_string())
        || picture.name.as_deref() != Some(name)
        || picture.geometry
            != Some(ShapeGeometry {
                x: Some(operation.x),
                y: Some(operation.y),
                width: Some(operation.width),
                height: Some(operation.height),
                rotation: None,
                flip_horizontal: None,
                flip_vertical: None,
            })
    {
        return false;
    }
    if package
        .content_type(media_part)
        .map(|value| value.as_str() == content_type)
        .unwrap_or(false)
        && package
            .part(media_part)
            .and_then(|part| package.read_part(&part))
            .ok()
            .as_deref()
            == Some(operation.image.as_slice())
        && package
            .part_relationships(slide)
            .ok()
            .and_then(|items| {
                items.into_iter().find(|item| {
                    item.id.as_str() == relationship_id
                        && matches!(
                            &item.target,
                            RelationshipTarget::Internal { part_name, .. } if part_name == media_part
                        )
                })
            })
            .is_some()
    {
        return true;
    }
    false
}

fn insert_picture_target_not_found(operation: &InsertPicture) -> PptxExecutionResult {
    failed("TARGET_NOT_FOUND", "slide handle was not found")
        .with_target("insert_picture", &operation.slide_handle)
}

fn insert_picture_verification_failed(operation: &InsertPicture) -> PptxExecutionResult {
    failed(
        "DOCUMENT_INVALID",
        "output PPTX picture insertion failed validation",
    )
    .with_target("insert_picture", &operation.slide_handle)
}

fn requested_formatting(
    current: &DirectRunFormatting,
    operation: &SetTextRunFormatting,
) -> DirectRunFormatting {
    DirectRunFormatting {
        bold: operation.bold.or(current.bold),
        italic: operation.italic.or(current.italic),
        font_size: operation.font_size.or(current.font_size),
        typeface: operation
            .typeface
            .clone()
            .or_else(|| current.typeface.clone()),
        color: operation
            .color
            .clone()
            .map(|value| value.to_ascii_uppercase())
            .or_else(|| current.color.clone()),
    }
}

fn patch_run_formatting(
    mut source: Vec<u8>,
    found: &LocatedRun,
    operation: &SetTextRunFormatting,
) -> Option<Vec<u8>> {
    let run_start = source[..found.start]
        .windows(5)
        .rposition(|value| value == b"<a:r>")?;
    let text_start = source[..found.start]
        .windows(5)
        .rposition(|value| value == b"<a:t>")?;
    let run = &source[run_start..text_start];
    let requested = requested_formatting(&operation.expected_current_formatting, operation);
    let rpr_start = run
        .windows(6)
        .position(|value| value == b"<a:rPr")
        .map(|value| run_start + value);
    if let Some(rpr_start) = rpr_start {
        let open_end = source[rpr_start..]
            .iter()
            .position(|value| *value == b'>')?
            + rpr_start;
        let close_start = source[open_end + 1..text_start]
            .windows(8)
            .position(|value| value == b"</a:rPr>")
            .map(|value| open_end + 1 + value)?;
        let mut replacement = String::from_utf8(source[rpr_start..=open_end].to_vec()).ok()?;
        if let Some(value) = requested.bold {
            replacement = set_xml_attribute(replacement, "b", if value { "1" } else { "0" });
        }
        if let Some(value) = requested.italic {
            replacement = set_xml_attribute(replacement, "i", if value { "1" } else { "0" });
        }
        if let Some(value) = requested.font_size {
            replacement = set_xml_attribute(replacement, "sz", &value.to_string());
        }
        let mut children = String::from_utf8(source[open_end + 1..close_start].to_vec()).ok()?;
        if operation.typeface.is_some() {
            children = set_or_add_latin(children, requested.typeface.as_deref()?);
        }
        if operation.color.is_some() {
            children = set_or_add_color(children, requested.color.as_deref()?);
        }
        source.splice(
            rpr_start..close_start + 8,
            format!("{replacement}{children}</a:rPr>").bytes(),
        );
    } else {
        let mut attributes = String::new();
        if let Some(value) = requested.bold {
            attributes.push_str(if value { " b=\"1\"" } else { " b=\"0\"" });
        }
        if let Some(value) = requested.italic {
            attributes.push_str(if value { " i=\"1\"" } else { " i=\"0\"" });
        }
        if let Some(value) = requested.font_size {
            attributes.push_str(&format!(" sz=\"{value}\""));
        }
        let latin = requested
            .typeface
            .map(|value| format!("<a:latin typeface=\"{value}\"/> "))
            .unwrap_or_default();
        let color = requested
            .color
            .map(|value| {
                format!(
                    "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
                    value.to_ascii_uppercase()
                )
            })
            .unwrap_or_default();
        source.splice(
            run_start + 5..run_start + 5,
            format!("<a:rPr{attributes}>{latin}{color}</a:rPr>").bytes(),
        );
    }
    Some(source)
}

fn set_xml_attribute(mut element: String, name: &str, value: &str) -> String {
    let needle = format!(" {name}=\"");
    if let Some(start) = element.find(&needle) {
        let value_start = start + needle.len();
        if let Some(end) = element[value_start..].find('\"') {
            element.replace_range(value_start..value_start + end, value);
            return element;
        }
    }
    let position = element.rfind('>').unwrap_or(element.len());
    element.insert_str(position, &format!(" {name}=\"{value}\""));
    element
}
fn set_or_add_latin(mut children: String, value: &str) -> String {
    if let Some(start) = children.find("<a:latin") {
        if let Some(end) = children[start..].find('>') {
            let item = set_xml_attribute(
                children[start..start + end + 1].to_owned(),
                "typeface",
                value,
            );
            children.replace_range(start..start + end + 1, &item);
            return children;
        }
    }
    children.push_str(&format!("<a:latin typeface=\"{value}\"/>"));
    children
}
fn set_or_add_color(mut children: String, value: &str) -> String {
    if let Some(start) = children.find("<a:srgbClr") {
        if let Some(end) = children[start..].find('>') {
            let item = set_xml_attribute(children[start..start + end + 1].to_owned(), "val", value);
            children.replace_range(start..start + end + 1, &item);
            return children;
        }
    }
    children.push_str(&format!(
        "<a:solidFill><a:srgbClr val=\"{value}\"/></a:solidFill>"
    ));
    children
}
fn formatting_verification_failed(operation: &SetTextRunFormatting) -> PptxExecutionResult {
    failed(
        "DOCUMENT_INVALID",
        "output PPTX formatting failed validation",
    )
    .with_target("set_text_run_formatting", &operation.run_handle)
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
        created_picture_handle: None,
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
        created_picture_handle: None,
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
