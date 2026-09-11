use napi::bindgen_prelude::{AsyncTask, Buffer, Task};
use napi::{Env, Result};
use napi_derive::napi;
use opensuite_docx::{
    DocxExecutionResult, execute_docx_delete_picture, execute_docx_insert_picture,
    execute_docx_replace_picture, execute_docx_set_picture_size,
};
use opensuite_protocol::{
    DeletePicture, ImagePayload, InsertPicture, OperationResult, ParagraphPlacement,
    PictureSizeChange, PictureTarget, ReplacePicture, SetPictureSize,
};

use crate::{
    ExecuteDocxReplaceTextOutput, ParagraphPlacementInput, SimpleTask, operation_result_output,
};

#[napi(object)]
pub struct InsertPictureInput {
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
        picture_placement(operation.placement).map(|placement| InsertPicture {
            image_bytes: operation.image_bytes.to_vec(),
            placement,
            alt_text: operation.alt_text,
            base_revision: operation.base_revision,
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
        _ => Err("picture size requires exactly one dimension"),
    }
}
