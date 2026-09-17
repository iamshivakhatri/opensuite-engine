//! Preservation-first, read-only PresentationML inspection.

use quick_xml::{
    events::{BytesStart, Event},
    name::ResolveResult,
    reader::NsReader,
};

use opensuite_opc::{Package, Part, Relationship, RelationshipTarget};
use opensuite_protocol::{Diagnostic, DiagnosticSeverity};

mod handles;
mod mutation;
mod search;

pub use mutation::{
    PptxExecutionResult, ReplaceParagraphTextRange, ReplaceTextRun, SetShapeGeometry,
    execute_pptx_replace_paragraph_text_range, execute_pptx_replace_text_run,
    execute_pptx_set_shape_geometry,
};
pub use search::{
    FindPptxText, PptxShapeInspection, PptxTextMatch, PptxTextSearchResult, find_pptx_text,
    inspect_pptx_shape,
};

pub const LAYER: &str = "pptx";

const PRESENTATION_NAMESPACES: [&str; 2] = [
    "http://schemas.openxmlformats.org/presentationml/2006/main",
    "http://purl.oclc.org/ooxml/presentationml/main",
];
const DRAWING_NAMESPACES: [&str; 2] = [
    "http://schemas.openxmlformats.org/drawingml/2006/main",
    "http://purl.oclc.org/ooxml/drawingml/main",
];
const PRESENTATION_CONTENT_TYPES: [&str; 2] = [
    "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
    "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
];
const SLIDE_RELATIONSHIP_TYPES: [&str; 2] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide",
    "http://purl.oclc.org/ooxml/officeDocument/relationships/slide",
];
const PREVIEW_LIMIT: usize = 160;
const SHAPE_PREVIEW_LIMIT: usize = 80;
const TEXT_VALUE_LIMIT: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PptxInspection {
    pub overview: Option<PresentationOverview>,
    pub diagnostics: Vec<Diagnostic>,
}

impl PptxInspection {
    fn failed(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            overview: None,
            diagnostics: vec![Diagnostic::new(code, DiagnosticSeverity::Error, message)],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationOverview {
    pub main_part_name: String,
    pub slides: Vec<SlideOverview>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlideOverview {
    pub handle: String,
    pub index: usize,
    pub part_name: String,
    pub shape_count: usize,
    pub text_shape_count: usize,
    pub text_preview: String,
    pub shapes: Vec<ShapeOverview>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShapeOverview {
    pub handle: String,
    pub kind: ShapeKind,
    /// OOXML metadata only; callers must target this object through `handle`.
    pub object_id: Option<String>,
    pub name: Option<String>,
    pub placeholder: Option<PlaceholderOverview>,
    pub geometry: Option<ShapeGeometry>,
    pub text_frame: Option<TextFrameOverview>,
    pub graphic_frame_kind: Option<GraphicFrameKind>,
    pub group_child_count: Option<usize>,
    pub text_preview: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeKind {
    Shape,
    Picture,
    GraphicFrame,
    Group,
    Connector,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaceholderOverview {
    pub placeholder_type: Option<String>,
    pub index: Option<u32>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShapeGeometry {
    pub x: Option<i64>,
    pub y: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub rotation: Option<i64>,
    pub flip_horizontal: Option<bool>,
    pub flip_vertical: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextFrameOverview {
    pub paragraphs: Vec<TextParagraphOverview>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextParagraphOverview {
    pub handle: String,
    pub text: String,
    pub runs: Vec<TextRunOverview>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextRunOverview {
    pub handle: String,
    pub text: String,
    pub formatting: DirectRunFormatting,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DirectRunFormatting {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub font_size: Option<i64>,
    pub typeface: Option<String>,
    pub color: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphicFrameKind {
    Table,
    Chart,
    Diagram,
    Other,
}

/// Opens owned PPTX bytes, verifies their OPC structure, and inspects slides in presentation order.
pub fn inspect_pptx(input: Vec<u8>) -> PptxInspection {
    let package = match Package::from_bytes(input) {
        Ok(package) => package,
        Err(error) => return PptxInspection::failed(error.code(), "could not load PPTX artifact"),
    };
    if let Err(error) = package.verify() {
        return PptxInspection::failed(error.code(), "PPTX package failed validation");
    }
    let main = match package.main_office_document() {
        Ok(part) => part,
        Err(error) => {
            return PptxInspection::failed(error.code(), "could not find main presentation part");
        }
    };
    if !PRESENTATION_CONTENT_TYPES.contains(&main.content_type.as_str()) {
        return PptxInspection::failed(
            "INVALID_PRESENTATION",
            "main part is not a PPTX presentation",
        );
    }
    let slide_ids = match presentation_slide_ids(&package, &main) {
        Ok(ids) => ids,
        Err(error) => return error,
    };
    let relationships = match package.part_relationships(&main) {
        Ok(values) => values,
        Err(error) => {
            return PptxInspection::failed(
                error.code(),
                "could not read presentation relationships",
            );
        }
    };
    let mut slides = Vec::with_capacity(slide_ids.len());
    for (index, relationship_id) in slide_ids.iter().enumerate() {
        let part = match slide_part(&package, &relationships, relationship_id) {
            Ok(part) => part,
            Err(error) => return error,
        };
        let bytes = match package.read_part(&part) {
            Ok(bytes) => bytes,
            Err(error) => return PptxInspection::failed(error.code(), "could not read slide part"),
        };
        match inspect_slide(&bytes, index, part.name.as_str()) {
            Ok(slide) => slides.push(slide),
            Err(error) => return error,
        }
    }
    PptxInspection {
        overview: Some(PresentationOverview {
            main_part_name: main.name.as_str().to_owned(),
            slides,
        }),
        diagnostics: Vec::new(),
    }
}

pub(crate) fn presentation_slide_ids(
    package: &Package,
    main: &Part,
) -> Result<Vec<String>, PptxInspection> {
    let bytes = package.read_part(main).map_err(|error| {
        PptxInspection::failed(error.code(), "could not read main presentation part")
    })?;
    let mut reader = NsReader::from_reader(bytes.as_slice());
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut root_seen = false;
    let mut slide_list_depth = None;
    let mut ids = Vec::new();
    loop {
        let (namespace, event) = reader.read_resolved_event_into(&mut buffer).map_err(|_| {
            PptxInspection::failed(
                "MALFORMED_PRESENTATION",
                "main presentation XML is malformed",
            )
        })?;
        let presentation = is_presentation_namespace(&namespace);
        match event {
            Event::Start(element) => {
                depth += 1;
                if !root_seen {
                    root_seen = true;
                    require_element(
                        presentation,
                        &element,
                        "presentation",
                        "INVALID_PRESENTATION",
                    )?;
                } else if presentation_element(presentation, &element, "sldIdLst") {
                    slide_list_depth = Some(depth);
                } else if slide_list_depth == Some(depth - 1)
                    && presentation_element(presentation, &element, "sldId")
                {
                    ids.push(required_relationship_id(&element)?);
                }
            }
            Event::Empty(element) => {
                if !root_seen {
                    return Err(PptxInspection::failed(
                        "INVALID_PRESENTATION",
                        "presentation root is empty",
                    ));
                }
                if slide_list_depth == Some(depth)
                    && presentation_element(presentation, &element, "sldId")
                {
                    ids.push(required_relationship_id(&element)?);
                }
            }
            Event::End(_) => {
                if slide_list_depth == Some(depth) {
                    slide_list_depth = None;
                }
                depth = depth.saturating_sub(1);
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    root_seen.then_some(ids).ok_or_else(|| {
        PptxInspection::failed("INVALID_PRESENTATION", "presentation has no root element")
    })
}

pub(crate) fn slide_part(
    package: &Package,
    relationships: &[Relationship],
    id: &str,
) -> Result<Part, PptxInspection> {
    let relationship = relationships
        .iter()
        .find(|relationship| relationship.id.as_str() == id)
        .ok_or_else(|| {
            PptxInspection::failed(
                "MISSING_SLIDE_RELATIONSHIP",
                "presentation slide relationship is missing",
            )
        })?;
    if !SLIDE_RELATIONSHIP_TYPES.contains(&relationship.relationship_type.as_str()) {
        return Err(PptxInspection::failed(
            "INVALID_SLIDE_RELATIONSHIP",
            "presentation relationship is not a slide",
        ));
    }
    let RelationshipTarget::Internal { part_name, .. } = &relationship.target else {
        return Err(PptxInspection::failed(
            "INVALID_SLIDE_RELATIONSHIP",
            "slide relationship target must be internal",
        ));
    };
    package.part(part_name).map_err(|error| {
        PptxInspection::failed(error.code(), "slide relationship target is missing")
    })
}

fn inspect_slide(
    bytes: &[u8],
    index: usize,
    part_name: &str,
) -> Result<SlideOverview, PptxInspection> {
    let mut reader = NsReader::from_reader(bytes);
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut root_seen = false;
    let mut shape_tree_depth = None;
    let mut shapes = Vec::new();
    let mut active: Option<ActiveShape> = None;
    let mut text_body_depth = None;
    let mut drawing_text_depth = None;
    let mut paragraph: Option<ActiveParagraph> = None;
    let mut run: Option<ActiveRun> = None;
    let mut run_properties_depth = None;
    let mut solid_fill_depth = None;
    let mut transform_depth = None;
    loop {
        let (namespace, event) = reader
            .read_resolved_event_into(&mut buffer)
            .map_err(|_| PptxInspection::failed("MALFORMED_SLIDE", "slide XML is malformed"))?;
        let presentation = is_presentation_namespace(&namespace);
        let drawing = is_drawing_namespace(&namespace);
        match event {
            Event::Start(element) => {
                depth += 1;
                if !root_seen {
                    root_seen = true;
                    require_element(presentation, &element, "sld", "INVALID_SLIDE")?;
                } else if presentation_element(presentation, &element, "spTree") {
                    shape_tree_depth = Some(depth);
                } else if shape_tree_depth == Some(depth - 1) {
                    if let Some(kind) = top_level_shape_kind(presentation, &element) {
                        active = Some(ActiveShape::new(kind, depth));
                    }
                } else if let Some(shape) = &mut active {
                    if presentation_element(presentation, &element, "cNvPr") {
                        shape.name = optional_attribute(&element, b"name");
                        shape.object_id = optional_attribute(&element, b"id");
                    }
                    if presentation_element(presentation, &element, "ph")
                        && shape.kind == ShapeKind::Shape
                    {
                        shape.placeholder = Some(PlaceholderOverview {
                            placeholder_type: optional_attribute(&element, b"type"),
                            index: optional_attribute(&element, b"idx")
                                .and_then(|value| value.parse().ok()),
                        });
                    }
                    if element.local_name().as_ref() == b"xfrm"
                        && (shape.kind != ShapeKind::Group || depth <= shape.depth + 2)
                    {
                        shape.geometry = Some(ShapeGeometry {
                            rotation: optional_attribute(&element, b"rot")
                                .and_then(|value| value.parse().ok()),
                            flip_horizontal: optional_attribute(&element, b"flipH")
                                .as_deref()
                                .and_then(bool_value),
                            flip_vertical: optional_attribute(&element, b"flipV")
                                .as_deref()
                                .and_then(bool_value),
                            ..Default::default()
                        });
                        transform_depth = Some(depth);
                    }
                    if let Some(geometry) = &mut shape.geometry {
                        if transform_depth == Some(depth - 1)
                            && element.local_name().as_ref() == b"off"
                        {
                            geometry.x = optional_attribute(&element, b"x")
                                .and_then(|value| value.parse().ok());
                            geometry.y = optional_attribute(&element, b"y")
                                .and_then(|value| value.parse().ok());
                        }
                        if transform_depth == Some(depth - 1)
                            && element.local_name().as_ref() == b"ext"
                        {
                            geometry.width = optional_attribute(&element, b"cx")
                                .and_then(|value| value.parse().ok());
                            geometry.height = optional_attribute(&element, b"cy")
                                .and_then(|value| value.parse().ok());
                        }
                    }
                    if shape.kind == ShapeKind::GraphicFrame
                        && element.local_name().as_ref() == b"graphicData"
                    {
                        shape.graphic_frame_kind = Some(graphic_frame_kind(
                            optional_attribute(&element, b"uri").as_deref(),
                        ));
                    }
                    if shape.kind == ShapeKind::Group
                        && shape_kind(presentation, &element).is_some()
                        && depth == shape.depth + 1
                    {
                        shape.group_child_count += 1;
                    }
                    if shape.kind == ShapeKind::Shape
                        && presentation_element(presentation, &element, "txBody")
                    {
                        shape.text_frame = Some(TextFrameOverview {
                            paragraphs: Vec::new(),
                        });
                        text_body_depth = Some(depth);
                    }
                    if text_body_depth.is_some() && drawing && element.local_name().as_ref() == b"p"
                    {
                        paragraph = Some(ActiveParagraph::new(depth));
                    }
                    if paragraph.is_some() && drawing && element.local_name().as_ref() == b"r" {
                        run = Some(ActiveRun::new(depth));
                    }
                    if drawing && element.local_name().as_ref() == b"t" {
                        drawing_text_depth = Some(depth);
                    }
                    if run.is_some() && drawing && element.local_name().as_ref() == b"rPr" {
                        run_properties_depth = Some(depth);
                        if let Some(run) = &mut run {
                            run.formatting.bold = optional_attribute(&element, b"b")
                                .as_deref()
                                .and_then(bool_value);
                            run.formatting.italic = optional_attribute(&element, b"i")
                                .as_deref()
                                .and_then(bool_value);
                            run.formatting.font_size = optional_attribute(&element, b"sz")
                                .and_then(|value| value.parse().ok());
                        }
                    }
                    if run_properties_depth.is_some()
                        && drawing
                        && element.local_name().as_ref() == b"latin"
                    {
                        if let Some(run) = &mut run {
                            run.formatting.typeface = optional_attribute(&element, b"typeface");
                        }
                    }
                    if run_properties_depth.is_some()
                        && drawing
                        && element.local_name().as_ref() == b"solidFill"
                    {
                        solid_fill_depth = Some(depth);
                    }
                    if solid_fill_depth.is_some()
                        && drawing
                        && element.local_name().as_ref() == b"srgbClr"
                    {
                        if let Some(run) = &mut run {
                            run.formatting.color = optional_attribute(&element, b"val");
                        }
                    }
                }
            }
            Event::Empty(element) => {
                if shape_tree_depth == Some(depth) {
                    if let Some(kind) = top_level_shape_kind(presentation, &element) {
                        shapes
                            .push(ActiveShape::new(kind, depth).into_overview(index, shapes.len()));
                    }
                } else if let Some(shape) = &mut active {
                    if presentation_element(presentation, &element, "cNvPr") {
                        shape.name = optional_attribute(&element, b"name");
                        shape.object_id = optional_attribute(&element, b"id");
                    }
                    if presentation_element(presentation, &element, "ph")
                        && shape.kind == ShapeKind::Shape
                    {
                        shape.placeholder = Some(PlaceholderOverview {
                            placeholder_type: optional_attribute(&element, b"type"),
                            index: optional_attribute(&element, b"idx")
                                .and_then(|value| value.parse().ok()),
                        });
                    }
                    if let Some(geometry) = &mut shape.geometry {
                        if transform_depth == Some(depth) && element.local_name().as_ref() == b"off"
                        {
                            geometry.x = optional_attribute(&element, b"x")
                                .and_then(|value| value.parse().ok());
                            geometry.y = optional_attribute(&element, b"y")
                                .and_then(|value| value.parse().ok());
                        }
                        if transform_depth == Some(depth) && element.local_name().as_ref() == b"ext"
                        {
                            geometry.width = optional_attribute(&element, b"cx")
                                .and_then(|value| value.parse().ok());
                            geometry.height = optional_attribute(&element, b"cy")
                                .and_then(|value| value.parse().ok());
                        }
                    }
                    if shape.kind == ShapeKind::GraphicFrame
                        && element.local_name().as_ref() == b"graphicData"
                    {
                        shape.graphic_frame_kind = Some(graphic_frame_kind(
                            optional_attribute(&element, b"uri").as_deref(),
                        ));
                    }
                    if run_properties_depth.is_some() && element.local_name().as_ref() == b"latin" {
                        if let Some(run) = &mut run {
                            run.formatting.typeface = optional_attribute(&element, b"typeface");
                        }
                    }
                    if solid_fill_depth.is_some() && element.local_name().as_ref() == b"srgbClr" {
                        if let Some(run) = &mut run {
                            run.formatting.color = optional_attribute(&element, b"val");
                        }
                    }
                }
            }
            Event::Text(text) => {
                if drawing_text_depth.is_some() && element_text_context(text_body_depth, &paragraph)
                {
                    let value = text.unescape().map_err(|_| {
                        PptxInspection::failed("MALFORMED_SLIDE", "slide text is invalid")
                    })?;
                    if let Some(paragraph) = &mut paragraph {
                        append_text(&mut paragraph.text, &value);
                    }
                    if let Some(run) = &mut run {
                        append_text(&mut run.text, &value);
                    }
                }
            }
            Event::End(_) => {
                if solid_fill_depth == Some(depth) {
                    solid_fill_depth = None;
                }
                if transform_depth == Some(depth) {
                    transform_depth = None;
                }
                if run_properties_depth == Some(depth) {
                    run_properties_depth = None;
                }
                if run.as_ref().is_some_and(|run| run.depth == depth) {
                    let run = run.take().expect("active run exists");
                    if let Some(paragraph) = &mut paragraph {
                        paragraph.runs.push(run);
                    }
                }
                if paragraph
                    .as_ref()
                    .is_some_and(|paragraph| paragraph.depth == depth)
                {
                    let paragraph = paragraph.take().expect("active paragraph exists");
                    if let Some(shape) = &mut active {
                        if let Some(frame) = &mut shape.text_frame {
                            frame.paragraphs.push(paragraph.into_overview(
                                index,
                                shapes.len(),
                                frame.paragraphs.len(),
                            ));
                        }
                    }
                }
                if text_body_depth == Some(depth) {
                    text_body_depth = None;
                }
                if drawing_text_depth == Some(depth) {
                    drawing_text_depth = None;
                }
                if let Some(shape) = &active {
                    if shape.depth == depth {
                        let shape = active.take().expect("active shape exists");
                        shapes.push(shape.into_overview(index, shapes.len()));
                    }
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
    if !root_seen {
        return Err(PptxInspection::failed(
            "INVALID_SLIDE",
            "slide has no root element",
        ));
    }
    let mut text_preview = String::new();
    let mut text_shape_count = 0;
    for shape in &shapes {
        if let Some(text) = &shape.text_preview {
            text_shape_count += 1;
            append_preview(&mut text_preview, text, PREVIEW_LIMIT);
        }
    }
    Ok(SlideOverview {
        handle: format!("s{index}"),
        index,
        part_name: part_name.to_owned(),
        shape_count: shapes.len(),
        text_shape_count,
        text_preview,
        shapes,
    })
}

struct ActiveShape {
    kind: ShapeKind,
    depth: usize,
    object_id: Option<String>,
    name: Option<String>,
    placeholder: Option<PlaceholderOverview>,
    geometry: Option<ShapeGeometry>,
    text_frame: Option<TextFrameOverview>,
    graphic_frame_kind: Option<GraphicFrameKind>,
    group_child_count: usize,
}

impl ActiveShape {
    fn new(kind: ShapeKind, depth: usize) -> Self {
        Self {
            kind,
            depth,
            object_id: None,
            name: None,
            placeholder: None,
            geometry: None,
            text_frame: None,
            graphic_frame_kind: None,
            group_child_count: 0,
        }
    }

    fn into_overview(self, slide_index: usize, shape_index: usize) -> ShapeOverview {
        let text_preview = self.text_frame.as_ref().and_then(|frame| {
            let mut preview = String::new();
            for paragraph in &frame.paragraphs {
                append_preview(&mut preview, &paragraph.text, SHAPE_PREVIEW_LIMIT);
            }
            (!preview.is_empty()).then_some(preview)
        });
        ShapeOverview {
            handle: format!("s{slide_index}:sh{shape_index}"),
            kind: self.kind,
            object_id: self.object_id,
            name: self.name,
            placeholder: self.placeholder,
            geometry: self.geometry,
            text_frame: self.text_frame,
            graphic_frame_kind: self.graphic_frame_kind,
            group_child_count: (self.kind == ShapeKind::Group).then_some(self.group_child_count),
            text_preview,
        }
    }
}

#[derive(Clone, Debug)]
struct ActiveParagraph {
    depth: usize,
    text: String,
    runs: Vec<ActiveRun>,
}

impl ActiveParagraph {
    fn new(depth: usize) -> Self {
        Self {
            depth,
            text: String::new(),
            runs: Vec::new(),
        }
    }

    fn into_overview(
        self,
        slide_index: usize,
        shape_index: usize,
        paragraph_index: usize,
    ) -> TextParagraphOverview {
        let handle = format!("s{slide_index}:sh{shape_index}:p{paragraph_index}");
        TextParagraphOverview {
            handle: handle.clone(),
            text: self.text,
            runs: self
                .runs
                .into_iter()
                .enumerate()
                .map(|(index, run)| TextRunOverview {
                    handle: format!("{handle}:r{index}"),
                    text: run.text,
                    formatting: run.formatting,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
struct ActiveRun {
    depth: usize,
    text: String,
    formatting: DirectRunFormatting,
}

impl ActiveRun {
    fn new(depth: usize) -> Self {
        Self {
            depth,
            text: String::new(),
            formatting: DirectRunFormatting::default(),
        }
    }
}

fn element_text_context(
    text_body_depth: Option<usize>,
    paragraph: &Option<ActiveParagraph>,
) -> bool {
    text_body_depth.is_some() && paragraph.is_some()
}

fn bool_value(value: &str) -> Option<bool> {
    match value {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

fn graphic_frame_kind(uri: Option<&str>) -> GraphicFrameKind {
    match uri {
        Some(uri) if uri.contains("/table") => GraphicFrameKind::Table,
        Some(uri) if uri.contains("/chart") => GraphicFrameKind::Chart,
        Some(uri) if uri.contains("/diagram") => GraphicFrameKind::Diagram,
        _ => GraphicFrameKind::Other,
    }
}

fn require_element(
    presentation: bool,
    element: &BytesStart<'_>,
    local: &str,
    code: &'static str,
) -> Result<(), PptxInspection> {
    presentation_element(presentation, element, local)
        .then_some(())
        .ok_or_else(|| PptxInspection::failed(code, format!("expected p:{local} root element")))
}

fn presentation_element(presentation: bool, element: &BytesStart<'_>, local: &str) -> bool {
    presentation && element.local_name().as_ref() == local.as_bytes()
}

fn shape_kind(presentation: bool, element: &BytesStart<'_>) -> Option<ShapeKind> {
    presentation_element(presentation, element, "sp")
        .then_some(ShapeKind::Shape)
        .or_else(|| {
            presentation_element(presentation, element, "pic").then_some(ShapeKind::Picture)
        })
        .or_else(|| {
            presentation_element(presentation, element, "graphicFrame")
                .then_some(ShapeKind::GraphicFrame)
        })
        .or_else(|| {
            presentation_element(presentation, element, "grpSp").then_some(ShapeKind::Group)
        })
        .or_else(|| {
            presentation_element(presentation, element, "cxnSp").then_some(ShapeKind::Connector)
        })
}

fn top_level_shape_kind(presentation: bool, element: &BytesStart<'_>) -> Option<ShapeKind> {
    shape_kind(presentation, element).or_else(|| {
        (!presentation_element(presentation, element, "nvGrpSpPr")
            && !presentation_element(presentation, element, "grpSpPr")
            && !presentation_element(presentation, element, "extLst"))
        .then_some(ShapeKind::Other)
    })
}

fn is_presentation_namespace(namespace: &ResolveResult<'_>) -> bool {
    match namespace {
        ResolveResult::Bound(uri) => {
            PRESENTATION_NAMESPACES.contains(&std::str::from_utf8(uri.as_ref()).unwrap_or_default())
        }
        _ => false,
    }
}

fn is_drawing_namespace(namespace: &ResolveResult<'_>) -> bool {
    match namespace {
        ResolveResult::Bound(uri) => {
            DRAWING_NAMESPACES.contains(&std::str::from_utf8(uri.as_ref()).unwrap_or_default())
        }
        _ => false,
    }
}

fn required_relationship_id(element: &BytesStart<'_>) -> Result<String, PptxInspection> {
    element
        .attributes()
        .with_checks(false)
        .flatten()
        .find(|attribute| attribute.key.as_ref().ends_with(b":id"))
        .map(|attribute| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
        .ok_or_else(|| {
            PptxInspection::failed("INVALID_PRESENTATION", "slide id has no relationship id")
        })
}

fn optional_attribute(element: &BytesStart<'_>, local: &[u8]) -> Option<String> {
    element
        .attributes()
        .with_checks(false)
        .flatten()
        .find_map(|attribute| {
            let key = attribute.key.as_ref();
            (key.rsplit(|byte| *byte == b':').next() == Some(local))
                .then(|| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
        })
}

fn append_preview(output: &mut String, text: &str, limit: usize) {
    if output.len() >= limit {
        return;
    }
    if !output.is_empty() {
        output.push(' ');
    }
    output.extend(text.chars().take(limit.saturating_sub(output.len())));
}

fn append_text(output: &mut String, text: &str) {
    output.extend(
        text.chars()
            .take(TEXT_VALUE_LIMIT.saturating_sub(output.chars().count())),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read, Write};
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

    const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
    const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
    const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    const OFFICE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
    const SLIDE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

    fn package(
        order: &[(&str, &str)],
        slide_relationships: &str,
        slides: &[(&str, &str)],
    ) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        let entries = [
            (
                "[Content_Types].xml",
                format!(
                    "<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/></Types>"
                ),
            ),
            (
                "_rels/.rels",
                format!(
                    "<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"ppt/presentation.xml\"/></Relationships>"
                ),
            ),
            (
                "ppt/presentation.xml",
                format!(
                    "<p:presentation xmlns:p=\"{P}\" xmlns:r=\"{R}\"><p:sldIdLst>{}</p:sldIdLst></p:presentation>",
                    order
                        .iter()
                        .map(|(id, _)| format!("<p:sldId r:id=\"{id}\"/>"))
                        .collect::<String>()
                ),
            ),
            (
                "ppt/_rels/presentation.xml.rels",
                format!("<Relationships>{slide_relationships}</Relationships>"),
            ),
            ("ppt/theme/theme1.xml", "<theme/>".to_owned()),
        ];
        for (name, value) in entries {
            zip.start_file(name, options).unwrap();
            zip.write_all(value.as_bytes()).unwrap();
        }
        for (name, xml) in slides {
            zip.start_file(name, options).unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
    fn slide(body: &str) -> String {
        format!(
            "<p:sld xmlns:p=\"{P}\" xmlns:a=\"{A}\"><p:cSld><p:spTree><p:nvGrpSpPr/><p:grpSpPr/>{body}</p:spTree></p:cSld></p:sld>"
        )
    }
    fn rel(id: &str, target: &str) -> String {
        format!("<Relationship Id=\"{id}\" Type=\"{SLIDE}\" Target=\"{target}\"/>")
    }

    fn entry_bytes(bytes: &[u8], name: &str) -> Vec<u8> {
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut entry = archive.by_name(name).unwrap();
        let mut value = Vec::new();
        entry.read_to_end(&mut value).unwrap();
        value
    }

    #[test]
    fn inspects_one_slide_text_shape() {
        let input = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[(
                "ppt/slides/slide1.xml",
                &slide(
                    "<p:sp><p:nvSpPr><p:cNvPr name=\"Title\"/></p:nvSpPr><p:txBody><a:p><a:r><a:t>Hello world</a:t></a:r></a:p></p:txBody></p:sp>",
                ),
            )],
        );
        let result = inspect_pptx(input);
        let overview = result.overview.unwrap();
        assert!(result.diagnostics.is_empty());
        assert_eq!(overview.slides[0].handle, "s0");
        assert_eq!(overview.slides[0].text_preview, "Hello world");
        assert_eq!(overview.slides[0].shapes[0].name.as_deref(), Some("Title"));
    }

    #[test]
    fn inspects_direct_text_shape_semantics() {
        let body = "<p:sp><p:nvSpPr><p:cNvPr id=\"7\" name=\"Body\"/><p:nvPr><p:ph type=\"body\" idx=\"2\"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm rot=\"60000\" flipH=\"1\"><a:off x=\"10\" y=\"20\"/><a:ext cx=\"30\" cy=\"40\"/></a:xfrm></p:spPr><p:txBody><a:p><a:r><a:rPr b=\"1\" i=\"0\" sz=\"1800\"><a:latin typeface=\"Aptos\"/><a:solidFill><a:srgbClr val=\"112233\"/></a:solidFill></a:rPr><a:t>Hello</a:t></a:r><a:r><a:t> world</a:t></a:r></a:p><a:p><a:r><a:t>Second</a:t></a:r></a:p></p:txBody></p:sp>";
        let result = inspect_pptx(package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[("ppt/slides/slide1.xml", &slide(body))],
        ));
        let shape = &result.overview.unwrap().slides[0].shapes[0];
        let frame = shape.text_frame.as_ref().unwrap();
        assert_eq!(shape.object_id.as_deref(), Some("7"));
        assert_eq!(shape.placeholder.as_ref().unwrap().index, Some(2));
        assert_eq!(shape.geometry.as_ref().unwrap().width, Some(30));
        assert_eq!(frame.paragraphs[0].handle, "s0:sh0:p0");
        assert_eq!(frame.paragraphs[0].runs[0].handle, "s0:sh0:p0:r0");
        assert_eq!(
            frame.paragraphs[0].runs[0].formatting.typeface.as_deref(),
            Some("Aptos")
        );
        assert_eq!(
            frame.paragraphs[0].runs[0].formatting.color.as_deref(),
            Some("112233")
        );
        assert_eq!(frame.paragraphs[1].text, "Second");
    }

    #[test]
    fn reports_direct_title_and_body_placeholders() {
        let body = "<p:sp><p:nvSpPr><p:nvPr><p:ph type=\"title\"/></p:nvPr></p:nvSpPr></p:sp><p:sp><p:nvSpPr><p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr></p:nvSpPr></p:sp>";
        let result = inspect_pptx(package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[("ppt/slides/slide1.xml", &slide(body))],
        ));
        let shapes = &result.overview.unwrap().slides[0].shapes;
        assert_eq!(
            shapes[0]
                .placeholder
                .as_ref()
                .unwrap()
                .placeholder_type
                .as_deref(),
            Some("title")
        );
        assert_eq!(
            shapes[1]
                .placeholder
                .as_ref()
                .unwrap()
                .placeholder_type
                .as_deref(),
            Some("body")
        );
        assert_eq!(shapes[1].placeholder.as_ref().unwrap().index, Some(1));
    }

    #[test]
    fn keeps_groups_opaque_and_classifies_graphic_frames() {
        let body = "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"4\" name=\"Table\"/></p:nvGraphicFramePr><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/table\"/></a:graphic></p:graphicFrame><p:graphicFrame><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"/></a:graphic></p:graphicFrame><p:grpSp><p:nvGrpSpPr><p:cNvPr id=\"9\" name=\"Group\"/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"1\" y=\"2\"/><a:ext cx=\"3\" cy=\"4\"/></a:xfrm></p:grpSpPr><p:sp><p:txBody><a:p><a:r><a:t>inside</a:t></a:r></a:p></p:txBody></p:sp></p:grpSp><p:sp><p:nvSpPr><p:cNvPr id=\"10\" name=\"No geometry\"/></p:nvSpPr></p:sp>";
        let result = inspect_pptx(package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[("ppt/slides/slide1.xml", &slide(body))],
        ));
        let shapes = &result.overview.unwrap().slides[0].shapes;
        assert_eq!(shapes.len(), 4);
        assert_eq!(shapes[0].graphic_frame_kind, Some(GraphicFrameKind::Table));
        assert_eq!(shapes[1].graphic_frame_kind, Some(GraphicFrameKind::Chart));
        assert_eq!(shapes[2].group_child_count, Some(1));
        assert_eq!(shapes[2].geometry.as_ref().unwrap().x, Some(1));
        assert!(shapes[2].text_frame.is_none());
        assert!(shapes[3].geometry.is_none());
    }

    #[test]
    fn preserves_unusual_text_and_keeps_handles_deterministic() {
        let body = "<p:sp><p:txBody><a:p><a:fld id=\"field\"><a:t>Field text</a:t></a:fld><a:br/><a:r><a:t> run</a:t></a:r></a:p></p:txBody></p:sp>";
        let input = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[("ppt/slides/slide1.xml", &slide(body))],
        );
        let first = inspect_pptx(input.clone());
        let second = inspect_pptx(input);
        let first_shape = &first.overview.as_ref().unwrap().slides[0].shapes[0];
        assert_eq!(first_shape.text_preview.as_deref(), Some("Field text run"));
        assert_eq!(
            first_shape.text_frame.as_ref().unwrap().paragraphs[0]
                .runs
                .len(),
            1
        );
        assert_eq!(first.overview, second.overview);
    }

    #[test]
    fn uses_presentation_slide_order() {
        let input = package(
            &[("rId2", ""), ("rId1", "")],
            &(rel("rId1", "slides/one.xml") + &rel("rId2", "slides/two.xml")),
            &[
                (
                    "ppt/slides/one.xml",
                    &slide(
                        "<p:sp><p:txBody><a:p><a:r><a:t>one</a:t></a:r></a:p></p:txBody></p:sp>",
                    ),
                ),
                (
                    "ppt/slides/two.xml",
                    &slide(
                        "<p:sp><p:txBody><a:p><a:r><a:t>two</a:t></a:r></a:p></p:txBody></p:sp>",
                    ),
                ),
            ],
        );
        let result = inspect_pptx(input);
        let slides = &result.overview.unwrap().slides;
        assert_eq!(slides[0].part_name, "/ppt/slides/two.xml");
        assert_eq!(slides[1].part_name, "/ppt/slides/one.xml");
    }

    #[test]
    fn classifies_multiple_and_unknown_shapes_without_failure() {
        let objects = "<p:sp><p:txBody><a:p><a:r><a:t>Text</a:t></a:r></a:p></p:txBody></p:sp><p:pic><p:nvPicPr><p:cNvPr name=\"Photo\"/></p:nvPicPr></p:pic><p:graphicFrame/><p:grpSp/><p:cxnSp/><p:customThing/>";
        let result = inspect_pptx(package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[("ppt/slides/slide1.xml", &slide(objects))],
        ));
        let slide = &result.overview.unwrap().slides[0];
        assert!(result.diagnostics.is_empty());
        assert_eq!(slide.shape_count, 6);
        assert_eq!(slide.text_shape_count, 1);
        assert_eq!(slide.shapes[1].kind, ShapeKind::Picture);
        assert_eq!(slide.shapes[5].kind, ShapeKind::Other);
    }

    #[test]
    fn reports_missing_slide_relationship() {
        let result = inspect_pptx(package(&[("rId9", "")], "", &[]));
        assert_eq!(result.diagnostics[0].code, "MISSING_SLIDE_RELATIONSHIP");
    }

    #[test]
    fn reports_a_non_slide_relationship_without_panicking() {
        let relationship =
            "<Relationship Id=\"rId1\" Type=\"urn:not-a-slide\" Target=\"slides/slide1.xml\"/>";
        let result = inspect_pptx(package(
            &[("rId1", "")],
            relationship,
            &[("ppt/slides/slide1.xml", &slide(""))],
        ));
        assert_eq!(result.diagnostics[0].code, "INVALID_SLIDE_RELATIONSHIP");
    }

    #[test]
    fn reports_a_slide_with_the_wrong_root() {
        let result = inspect_pptx(package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[("ppt/slides/slide1.xml", "<not-a-slide/>")],
        ));
        assert_eq!(result.diagnostics[0].code, "INVALID_SLIDE");
    }

    #[test]
    fn replaces_one_run_and_preserves_unrelated_parts() {
        let body = "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Text\"/></p:nvSpPr><p:txBody><a:p><a:r><a:rPr b=\"1\"><a:latin typeface=\"Aptos\"/></a:rPr><a:t>old</a:t></a:r><a:r><a:t>other</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:txBody><a:p><a:r><a:t>unchanged</a:t></a:r></a:p></p:txBody></p:sp>";
        let input = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[("ppt/slides/slide1.xml", &slide(body))],
        );
        let result = execute_pptx_replace_text_run(
            input.clone(),
            &ReplaceTextRun {
                run_handle: "s0:sh0:p0:r0".to_owned(),
                expected_current_text: "old".to_owned(),
                replacement_text: "new".to_owned(),
                base_revision: None,
            },
        );
        let output = result.output_artifact.unwrap();
        let overview = inspect_pptx(output.clone()).overview.unwrap();
        assert_eq!(
            overview.slides[0].shapes[0]
                .text_frame
                .as_ref()
                .unwrap()
                .paragraphs[0]
                .runs[0]
                .text,
            "new"
        );
        assert_eq!(
            overview.slides[0].shapes[0]
                .text_frame
                .as_ref()
                .unwrap()
                .paragraphs[0]
                .runs[0]
                .formatting
                .bold,
            Some(true)
        );
        assert_eq!(
            overview.slides[0].shapes[1].text_preview.as_deref(),
            Some("unchanged")
        );
        assert_eq!(
            entry_bytes(&input, "ppt/theme/theme1.xml"),
            entry_bytes(&output, "ppt/theme/theme1.xml")
        );
    }

    #[test]
    fn replaces_placeholder_whitespace_and_xml_characters() {
        let body = "<p:sp><p:nvSpPr><p:nvPr><p:ph type=\"body\"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>old</a:t></a:r></a:p></p:txBody></p:sp>";
        let input = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[("ppt/slides/slide1.xml", &slide(body))],
        );
        let replacement = "  A & B < C > D  ";
        let result = execute_pptx_replace_text_run(
            input,
            &ReplaceTextRun {
                run_handle: "s0:sh0:p0:r0".to_owned(),
                expected_current_text: "old".to_owned(),
                replacement_text: replacement.to_owned(),
                base_revision: None,
            },
        );
        let output = result.output_artifact.unwrap();
        let slide = entry_bytes(&output, "ppt/slides/slide1.xml");
        assert!(
            String::from_utf8(slide)
                .unwrap()
                .contains("  A &amp; B &lt; C &gt; D  ")
        );
        assert_eq!(
            inspect_pptx(output).overview.unwrap().slides[0].shapes[0]
                .text_preview
                .as_deref(),
            Some(replacement)
        );
    }

    #[test]
    fn refuses_stale_unknown_field_and_malformed_run_targets() {
        let normal = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[(
                "ppt/slides/slide1.xml",
                &slide("<p:sp><p:txBody><a:p><a:r><a:t>old</a:t></a:r></a:p></p:txBody></p:sp>"),
            )],
        );
        let stale = execute_pptx_replace_text_run(
            normal.clone(),
            &ReplaceTextRun {
                run_handle: "s0:sh0:p0:r0".to_owned(),
                expected_current_text: "wrong".to_owned(),
                replacement_text: "new".to_owned(),
                base_revision: None,
            },
        );
        assert_eq!(stale.operation.diagnostics[0].code, "PRECONDITION_FAILED");
        assert!(stale.output_artifact.is_none());
        let unknown = execute_pptx_replace_text_run(
            normal,
            &ReplaceTextRun {
                run_handle: "s0:sh9:p0:r0".to_owned(),
                expected_current_text: "old".to_owned(),
                replacement_text: "new".to_owned(),
                base_revision: None,
            },
        );
        assert_eq!(unknown.operation.diagnostics[0].code, "TARGET_NOT_FOUND");
        let field = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[(
                "ppt/slides/slide1.xml",
                &slide(
                    "<p:sp><p:txBody><a:p><a:fld id=\"field\"><a:t>field</a:t></a:fld></a:p></p:txBody></p:sp>",
                ),
            )],
        );
        let field_result = execute_pptx_replace_text_run(
            field,
            &ReplaceTextRun {
                run_handle: "s0:sh0:p0:r0".to_owned(),
                expected_current_text: "field".to_owned(),
                replacement_text: "new".to_owned(),
                base_revision: None,
            },
        );
        assert!(field_result.output_artifact.is_none());
        let malformed = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[(
                "ppt/slides/slide1.xml",
                &slide("<p:sp><p:txBody><a:p><a:r><a:rPr/></a:r></a:p></p:txBody></p:sp>"),
            )],
        );
        let malformed_result = execute_pptx_replace_text_run(
            malformed,
            &ReplaceTextRun {
                run_handle: "s0:sh0:p0:r0".to_owned(),
                expected_current_text: "".to_owned(),
                replacement_text: "new".to_owned(),
                base_revision: None,
            },
        );
        assert_eq!(
            malformed_result.operation.diagnostics[0].code,
            "UNSUPPORTED_OPERATION"
        );
    }

    #[test]
    fn searches_across_runs_with_context_limits_and_shape_follow_up() {
        let body = "<p:sp><p:nvSpPr><p:cNvPr name=\"Revenue\"/></p:nvSpPr><p:txBody><a:p><a:r><a:t>Quarter</a:t></a:r><a:r><a:t>ly Revenue</a:t></a:r></a:p><a:p><a:r><a:t>Quarterly Revenue again</a:t></a:r></a:p></p:txBody></p:sp>";
        let input = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[("ppt/slides/slide1.xml", &slide(body))],
        );
        let result = find_pptx_text(
            input.clone(),
            &FindPptxText {
                text: "quarterly revenue".to_owned(),
                case_sensitive: false,
                limit: Some(1),
            },
        );
        assert_eq!(result.matches.len(), 1);
        assert_eq!(
            result.matches[0].run_handles,
            vec!["s0:sh0:p0:r0", "s0:sh0:p0:r1"]
        );
        assert_eq!(result.matches[0].shape_name.as_deref(), Some("Revenue"));
        assert!(result.matches[0].context.len() <= 160);
        assert!(
            find_pptx_text(
                input.clone(),
                &FindPptxText {
                    text: "quarterly revenue".to_owned(),
                    case_sensitive: true,
                    limit: None
                }
            )
            .matches
            .is_empty()
        );
        assert!(
            inspect_pptx_shape(input, "s0:sh0")
                .shape
                .unwrap()
                .text_frame
                .is_some()
        );
    }

    #[test]
    fn searches_in_presentation_order_and_excludes_unsupported_objects() {
        let first =
            slide("<p:sp><p:txBody><a:p><a:r><a:t>duplicate</a:t></a:r></a:p></p:txBody></p:sp>");
        let second = slide(
            "<p:graphicFrame><a:graphic><a:graphicData><a:t>hidden</a:t></a:graphicData></a:graphic></p:graphicFrame><p:sp><p:txBody><a:p><a:r><a:t>duplicate</a:t></a:r></a:p></p:txBody></p:sp>",
        );
        let input = package(
            &[("rId2", ""), ("rId1", "")],
            &(rel("rId1", "slides/one.xml") + &rel("rId2", "slides/two.xml")),
            &[
                ("ppt/slides/one.xml", &first),
                ("ppt/slides/two.xml", &second),
            ],
        );
        let request = FindPptxText {
            text: "duplicate".to_owned(),
            case_sensitive: false,
            limit: Some(1000),
        };
        let first_result = find_pptx_text(input.clone(), &request);
        let second_result = find_pptx_text(input.clone(), &request);
        assert_eq!(
            first_result
                .matches
                .iter()
                .map(|item| item.slide_handle.as_str())
                .collect::<Vec<_>>(),
            vec!["s0", "s1"]
        );
        assert_eq!(first_result, second_result);
        assert!(
            find_pptx_text(
                input,
                &FindPptxText {
                    text: "hidden".to_owned(),
                    case_sensitive: false,
                    limit: None
                }
            )
            .matches
            .is_empty()
        );
    }

    #[test]
    fn search_reports_empty_and_malformed_requests_cleanly() {
        let empty = find_pptx_text(
            Vec::new(),
            &FindPptxText {
                text: "needle".to_owned(),
                case_sensitive: false,
                limit: None,
            },
        );
        assert!(!empty.diagnostics.is_empty());
        let invalid = find_pptx_text(
            Vec::new(),
            &FindPptxText {
                text: String::new(),
                case_sensitive: false,
                limit: None,
            },
        );
        assert_eq!(invalid.diagnostics[0].code, "INVALID_TEXT_QUERY");
        let no_match = find_pptx_text(
            package(
                &[("rId1", "")],
                &rel("rId1", "slides/slide1.xml"),
                &[(
                    "ppt/slides/slide1.xml",
                    &slide(
                        "<p:sp><p:txBody><a:p><a:r><a:t>text</a:t></a:r></a:p></p:txBody></p:sp>",
                    ),
                )],
            ),
            &FindPptxText {
                text: "missing".to_owned(),
                case_sensitive: false,
                limit: None,
            },
        );
        assert!(no_match.matches.is_empty());
        assert!(no_match.diagnostics.is_empty());
    }

    #[test]
    fn preserves_a_realistic_presentation_when_replacing_a_cross_run_range() {
        let input = include_bytes!("../tests/fixtures/realistic-presentation.pptx").to_vec();
        let inspection = inspect_pptx(input.clone());
        assert!(
            inspection.overview.is_some(),
            "{:?}",
            inspection.diagnostics
        );
        let overview = inspection.overview.as_ref().unwrap();
        assert_eq!(overview.slides.len(), 3);
        let search = find_pptx_text(
            input.clone(),
            &FindPptxText {
                text: "Baseline & source".to_owned(),
                case_sensitive: true,
                limit: None,
            },
        );
        assert_eq!(search.matches.len(), 1);
        let target = &search.matches[0];
        assert_eq!(
            inspect_pptx_shape(input.clone(), &target.shape_handle)
                .shape
                .unwrap()
                .handle,
            target.shape_handle
        );
        let before_paragraph = &overview.slides[0].shapes[1]
            .text_frame
            .as_ref()
            .unwrap()
            .paragraphs[0];
        assert_eq!(before_paragraph.runs[0].formatting.bold, Some(true));
        assert_eq!(before_paragraph.runs[1].formatting.italic, Some(true));
        let output = execute_pptx_replace_paragraph_text_range(
            input.clone(),
            &ReplaceParagraphTextRange {
                paragraph_handle: target.paragraph_handle.clone(),
                start_offset: target.start_offset,
                end_offset: target.end_offset,
                expected_current_text: target.matched_text.clone(),
                expected_paragraph_text: before_paragraph.text.clone(),
                replacement_text: "Reviewed content".to_owned(),
                base_revision: None,
            },
        )
        .output_artifact
        .unwrap();
        let changed = inspect_pptx(output.clone()).overview.unwrap();
        assert_eq!(changed.slides.len(), 3);
        assert_eq!(changed.slides[0].handle, "s0");
        let paragraph = &changed.slides[0].shapes[1]
            .text_frame
            .as_ref()
            .unwrap()
            .paragraphs[0];
        assert_eq!(paragraph.handle, target.paragraph_handle);
        assert_eq!(paragraph.text, "Reviewed content remains");
        assert_eq!(paragraph.runs[0].formatting.bold, Some(true));
        assert_eq!(paragraph.runs[1].text, " remains");
        assert_eq!(paragraph.runs[1].formatting.italic, Some(true));
        assert!(
            changed.slides[1]
                .text_preview
                .contains("Unrelated text stays")
        );
        for part in [
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/theme/theme1.xml",
            "ppt/slides/slide2.xml",
            "ppt/slides/charts/chart1.xml",
        ] {
            assert_eq!(
                entry_bytes(&input, part),
                entry_bytes(&output, part),
                "{part}"
            );
        }
    }

    #[test]
    fn replaces_a_paragraph_range_across_runs_and_preserves_suffixes() {
        let input = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[(
                "ppt/slides/slide1.xml",
                &slide(
                    "<p:sp><p:txBody><a:p><a:r><a:rPr b=\"1\"/><a:t>Hello WOR</a:t></a:r><a:r><a:rPr i=\"1\"/><a:t>LD today</a:t></a:r></a:p></p:txBody></p:sp>",
                ),
            )],
        );
        let result = execute_pptx_replace_paragraph_text_range(
            input,
            &ReplaceParagraphTextRange {
                paragraph_handle: "s0:sh0:p0".to_owned(),
                start_offset: 6,
                end_offset: 11,
                expected_current_text: "WORLD".to_owned(),
                expected_paragraph_text: "Hello WORLD today".to_owned(),
                replacement_text: "team".to_owned(),
                base_revision: None,
            },
        );
        let output = result.output_artifact.unwrap();
        let overview = inspect_pptx(output).overview.unwrap();
        let paragraph = &overview.slides[0].shapes[0]
            .text_frame
            .as_ref()
            .unwrap()
            .paragraphs[0];
        assert_eq!(paragraph.text, "Hello team today");
        assert_eq!(paragraph.runs[0].text, "Hello team");
        assert_eq!(paragraph.runs[1].text, " today");
    }

    #[test]
    fn replaces_duplicate_search_range_without_relocating_it() {
        let input = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[(
                "ppt/slides/slide1.xml",
                &slide(
                    "<p:sp><p:txBody><a:p><a:r><a:t>same same</a:t></a:r></a:p></p:txBody></p:sp>",
                ),
            )],
        );
        let found = find_pptx_text(
            input.clone(),
            &FindPptxText {
                text: "same".to_owned(),
                case_sensitive: true,
                limit: None,
            },
        );
        let target = &found.matches[1];
        let result = execute_pptx_replace_paragraph_text_range(
            input,
            &ReplaceParagraphTextRange {
                paragraph_handle: target.paragraph_handle.clone(),
                start_offset: target.start_offset,
                end_offset: target.end_offset,
                expected_current_text: target.matched_text.clone(),
                expected_paragraph_text: "same same".to_owned(),
                replacement_text: "other".to_owned(),
                base_revision: None,
            },
        );
        assert_eq!(
            inspect_pptx(result.output_artifact.unwrap())
                .overview
                .unwrap()
                .slides[0]
                .text_preview,
            "same other"
        );
    }

    #[test]
    fn refuses_invalid_or_line_break_paragraph_ranges() {
        let input = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[(
                "ppt/slides/slide1.xml",
                &slide("<p:sp><p:txBody><a:p><a:r><a:t>text</a:t></a:r></a:p></p:txBody></p:sp>"),
            )],
        );
        let invalid = ReplaceParagraphTextRange {
            paragraph_handle: "s0:sh0:p0".to_owned(),
            start_offset: 0,
            end_offset: 5,
            expected_current_text: "text".to_owned(),
            expected_paragraph_text: "text".to_owned(),
            replacement_text: "next".to_owned(),
            base_revision: None,
        };
        assert!(
            execute_pptx_replace_paragraph_text_range(input.clone(), &invalid)
                .output_artifact
                .is_none()
        );
        let newline = ReplaceParagraphTextRange {
            end_offset: 4,
            replacement_text: "next\nline".to_owned(),
            ..invalid
        };
        assert!(
            execute_pptx_replace_paragraph_text_range(input, &newline)
                .output_artifact
                .is_none()
        );
    }

    #[test]
    fn moves_and_resizes_a_direct_text_shape() {
        let input = package(
            &[("rId1", "")],
            &rel("rId1", "slides/slide1.xml"),
            &[(
                "ppt/slides/slide1.xml",
                &slide(
                    "<p:sp><p:spPr><a:xfrm rot=\"9\" flipH=\"1\"><a:off x=\"1\" y=\"2\"/><a:ext cx=\"3\" cy=\"4\"/></a:xfrm></p:spPr><p:txBody><a:p><a:r><a:t>keep</a:t></a:r></a:p></p:txBody></p:sp>",
                ),
            )],
        );
        let current = inspect_pptx(input.clone()).overview.unwrap().slides[0].shapes[0]
            .geometry
            .clone()
            .unwrap();
        let output = execute_pptx_set_shape_geometry(
            input,
            &SetShapeGeometry {
                shape_handle: "s0:sh0".to_owned(),
                expected_current_geometry: current,
                x: 10,
                y: 20,
                width: 30,
                height: 40,
                base_revision: None,
            },
        )
        .output_artifact
        .unwrap();
        let shape = &inspect_pptx(output).overview.unwrap().slides[0].shapes[0];
        assert_eq!(shape.geometry.as_ref().unwrap().x, Some(10));
        assert_eq!(shape.geometry.as_ref().unwrap().rotation, Some(9));
        assert_eq!(shape.text_preview.as_deref(), Some("keep"));
    }

    #[test]
    fn preserves_realistic_fixture_parts_when_moving_a_text_shape() {
        let input = include_bytes!("../tests/fixtures/realistic-presentation.pptx").to_vec();
        let before = inspect_pptx(input.clone()).overview.unwrap();
        let shape = &before.slides[0].shapes[1];
        let output = execute_pptx_set_shape_geometry(
            input.clone(),
            &SetShapeGeometry {
                shape_handle: shape.handle.clone(),
                expected_current_geometry: shape.geometry.clone().unwrap(),
                x: 1066800,
                y: 1676400,
                width: 4953000,
                height: 1371600,
                base_revision: None,
            },
        )
        .output_artifact
        .unwrap();
        let changed = inspect_pptx(output.clone()).overview.unwrap();
        assert_eq!(changed.slides.len(), 3);
        assert_eq!(changed.slides[0].shapes[1].text_preview, shape.text_preview);
        assert_eq!(
            changed.slides[0].shapes[1].geometry.as_ref().unwrap().x,
            Some(1066800)
        );
        for part in [
            "ppt/slides/slide2.xml",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/theme/theme1.xml",
            "ppt/slides/charts/chart1.xml",
        ] {
            assert_eq!(
                entry_bytes(&input, part),
                entry_bytes(&output, part),
                "{part}"
            );
        }
    }
}
