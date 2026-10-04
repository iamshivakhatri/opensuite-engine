use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
use opensuite_docx::{
    DocxExecutionResult, execute_docx_delete_picture, execute_docx_insert_picture,
    execute_docx_replace_picture, execute_docx_set_picture_layout, execute_docx_set_picture_size,
};
use opensuite_protocol::{
    DeletePicture, ImageAlignment, ImageAxisPosition, ImagePayload, ImagePosition,
    ImagePositionReference, ImageTextDistance, ImageWrap, InsertPicture, OperationResult,
    ParagraphPlacement, PictureLayoutPatch, PictureSizeChange, PictureTarget, ReplacePicture,
    SetPictureLayout, SetPictureSize,
};

use crate::{
    ExecuteDocxReplaceTextOutput, ParagraphPlacementInput, SimpleTask, operation_result_output,
};

#[napi(object)]
pub struct InsertPictureInput {
    pub width_emu: Option<i64>,
    pub height_emu: Option<i64>,
    pub layout: Option<PictureLayoutInput>,
    pub image_bytes: Buffer,
    pub placement: ParagraphPlacementInput,
    pub alt_text: Option<String>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct DeletePictureInput {
    pub handle: String,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct SetPictureSizeInput {
    pub handle: String,
    pub width_emu: Option<i64>,
    pub height_emu: Option<i64>,
    pub base_revision: Option<String>,
}

#[napi(object)]
pub struct ReplacePictureInput {
    pub handle: String,
    pub replacement_bytes: Buffer,
    pub content_type: String,
    pub base_revision: Option<String>,
}

#[napi(js_name = "executeDocxInsertPicture")]
pub fn execute_docx_insert_picture_node(
    input: Buffer,
    operation: InsertPictureInput,
) -> AsyncTask<PictureTask<InsertPicture>> {
    PictureTask::new(
        input,
        picture_placement(operation.placement).and_then(|placement| {
            Ok(InsertPicture {
                size: optional_picture_size(operation.width_emu, operation.height_emu)?,
                layout: operation.layout.map(layout_patch).transpose()?,
                image_bytes: operation.image_bytes.to_vec(),
                placement,
                alt_text: operation.alt_text,
                base_revision: operation.base_revision,
            })
        }),
        execute_docx_insert_picture,
    )
}

#[napi(js_name = "executeDocxDeletePicture")]
pub fn execute_docx_delete_picture_node(
    input: Buffer,
    operation: DeletePictureInput,
) -> AsyncTask<SimpleTask<DeletePicture>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: DeletePicture {
            target: picture_target(operation.handle),
            base_revision: operation.base_revision,
        },
        run: execute_docx_delete_picture,
    })
}

#[napi(js_name = "executeDocxSetPictureSize")]
pub fn execute_docx_set_picture_size_node(
    input: Buffer,
    operation: SetPictureSizeInput,
) -> AsyncTask<PictureTask<SetPictureSize>> {
    PictureTask::new(
        input,
        picture_size(operation.width_emu, operation.height_emu).map(|size| SetPictureSize {
            target: picture_target(operation.handle),
            size,
            base_revision: operation.base_revision,
        }),
        execute_docx_set_picture_size,
    )
}

#[napi(js_name = "executeDocxReplacePicture")]
pub fn execute_docx_replace_picture_node(
    input: Buffer,
    operation: ReplacePictureInput,
) -> AsyncTask<SimpleTask<ReplacePicture>> {
    AsyncTask::new(SimpleTask {
        input: input.to_vec(),
        operation: ReplacePicture {
            target: picture_target(operation.handle),
            replacement: ImagePayload {
                content_type: operation.content_type,
                bytes: operation.replacement_bytes.to_vec(),
            },
            base_revision: operation.base_revision,
        },
        run: execute_docx_replace_picture,
    })
}

pub struct PictureTask<T> {
    input: Vec<u8>,
    operation: std::result::Result<T, &'static str>,
    run: fn(Vec<u8>, &T) -> DocxExecutionResult,
}

impl<T: Send> PictureTask<T> {
    fn new(
        input: Buffer,
        operation: std::result::Result<T, &'static str>,
        run: fn(Vec<u8>, &T) -> DocxExecutionResult,
    ) -> AsyncTask<Self> {
        AsyncTask::new(Self {
            input: input.to_vec(),
            operation,
            run,
        })
    }
}

impl<T: Send> Task for PictureTask<T> {
    type Output = DocxExecutionResult;
    type JsValue = ExecuteDocxReplaceTextOutput;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(match &self.operation {
            Ok(operation) => (self.run)(std::mem::take(&mut self.input), operation),
            Err(message) => DocxExecutionResult {
                operation: OperationResult::failed("INVALID_OPERATION", *message),
                output_artifact: None,
            },
        })
    }

    fn resolve(&mut self, _env: Env, result: Self::Output) -> Result<Self::JsValue> {
        Ok(ExecuteDocxReplaceTextOutput {
            result: operation_result_output(result.operation),
            output: result.output_artifact.map(Buffer::from),
        })
    }
}

fn picture_target(handle: String) -> PictureTarget {
    PictureTarget {
        handle: Some(handle),
        name: None,
        description: None,
        occurrence: None,
    }
}

fn picture_placement(
    input: ParagraphPlacementInput,
) -> std::result::Result<ParagraphPlacement, &'static str> {
    match input.kind.as_str() {
        "start" => Ok(ParagraphPlacement::Start),
        "end" => Ok(ParagraphPlacement::End),
        "before" => input
            .handle
            .map(|handle| ParagraphPlacement::Before { handle })
            .ok_or("before placement requires a body block handle"),
        "after" => input
            .handle
            .map(|handle| ParagraphPlacement::After { handle })
            .ok_or("after placement requires a body block handle"),
        _ => Err("picture placement is not supported"),
    }
}

fn picture_size(
    width_emu: Option<i64>,
    height_emu: Option<i64>,
) -> std::result::Result<PictureSizeChange, &'static str> {
    match (width_emu, height_emu) {
        (Some(width), None) => Ok(PictureSizeChange::WidthEmu(width)),
        (None, Some(height)) => Ok(PictureSizeChange::HeightEmu(height)),
        (Some(width), Some(height)) => Ok(PictureSizeChange::ExactEmu { width, height }),
        _ => Err("picture size requires at least one dimension"),
    }
}

fn optional_picture_size(
    width: Option<i64>,
    height: Option<i64>,
) -> std::result::Result<Option<PictureSizeChange>, &'static str> {
    if width.is_none() && height.is_none() {
        Ok(None)
    } else {
        picture_size(width, height).map(Some)
    }
}
#[napi(object)]
pub struct ImagePositionInput {
    pub reference: String,
    /// start/end mean left/right horizontally, top/bottom vertically.
    pub alignment: Option<String>,
    pub offset_emu: Option<i64>,
}
#[napi(object)]
pub struct ImageDistanceInput {
    pub top_emu: Option<i64>,
    pub bottom_emu: Option<i64>,
    pub left_emu: Option<i64>,
    pub right_emu: Option<i64>,
}
#[napi(object)]
pub struct PictureLayoutInput {
    pub horizontal: Option<ImagePositionInput>,
    pub vertical: Option<ImagePositionInput>,
    pub wrap: Option<String>,
    pub distance: Option<ImageDistanceInput>,
}
#[napi(object)]
pub struct SetPictureLayoutInput {
    pub handle: String,
    pub layout: PictureLayoutInput,
    pub base_revision: Option<String>,
}
fn axis(input: ImagePositionInput) -> std::result::Result<ImageAxisPosition, &'static str> {
    let reference = match input.reference.as_str() {
        "page" => ImagePositionReference::Page,
        "margin" => ImagePositionReference::Margin,
        "column" => ImagePositionReference::Column,
        "paragraph" => ImagePositionReference::Paragraph,
        _ => return Err("unsupported image position reference"),
    };
    let position = match (input.alignment.as_deref(), input.offset_emu) {
        (Some(value), None) => ImagePosition::Align(match value {
            "start" => ImageAlignment::Start,
            "center" => ImageAlignment::Center,
            "end" => ImageAlignment::End,
            _ => return Err("alignment must be start/center/end"),
        }),
        (None, Some(value)) => ImagePosition::OffsetEmu(value),
        _ => return Err("image position requires exactly one of alignment or offsetEmu"),
    };
    Ok(ImageAxisPosition {
        reference,
        position,
    })
}
fn layout_patch(
    input: PictureLayoutInput,
) -> std::result::Result<PictureLayoutPatch, &'static str> {
    Ok(PictureLayoutPatch {
        horizontal: input.horizontal.map(axis).transpose()?,
        vertical: input.vertical.map(axis).transpose()?,
        wrap: input
            .wrap
            .map(|v| match v.as_str() {
                "square" => Ok(ImageWrap::Square),
                "topAndBottom" => Ok(ImageWrap::TopAndBottom),
                "behindText" => Ok(ImageWrap::BehindText),
                "inFrontOfText" => Ok(ImageWrap::InFrontOfText),
                _ => Err("unsupported image wrap mode"),
            })
            .transpose()?,
        distance: input
            .distance
            .map(|v| ImageTextDistance {
                top_emu: v.top_emu,
                bottom_emu: v.bottom_emu,
                left_emu: v.left_emu,
                right_emu: v.right_emu,
            })
            .unwrap_or_default(),
    })
}
#[napi(js_name = "executeDocxSetPictureLayout")]
pub fn set_picture_layout(
    input: Buffer,
    operation: SetPictureLayoutInput,
) -> AsyncTask<PictureTask<SetPictureLayout>> {
    PictureTask::new(
        input,
        layout_patch(operation.layout).map(|layout| SetPictureLayout {
            target: picture_target(operation.handle),
            layout,
            base_revision: operation.base_revision,
        }),
        execute_docx_set_picture_layout,
    )
}
