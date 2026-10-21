mod cell;
mod header_footer;
mod inline;
mod layout;
use super::*;
use inline::{automatic_extent, picture_fragment};
pub use layout::set_picture_layout_to_vec;
use layout::{anchored_fragment, sized_extent};

pub(super) const EMU_PER_PIXEL_AT_96_DPI: i64 = 9_525;
// Preserve the legacy intrinsic-size cap; explicit size bypasses it, section inspection reports overflow.
pub(super) const MAX_INLINE_PICTURE_WIDTH_EMU: i64 = 5_943_600;
pub(super) const IMAGE_RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub(super) const PNG_CONTENT_TYPE: &str = "image/png";
pub(super) const JPEG_CONTENT_TYPE: &str = "image/jpeg";

pub(super) struct InsertedPictureVerification<'a> {
    owner: &'a PartName,
    layout: Option<&'a opensuite_protocol::PictureLayoutPatch>,
    alignment: Option<opensuite_protocol::ParagraphAlignment>,
    paragraph_start: usize,
    container_start: usize,
    document_xml: &'a [u8],
    picture_id: u32,
    width_emu: i64,
    height_emu: i64,
    image_bytes: &'a [u8],
}

pub(super) struct PictureResizeVerification {
    handle: String,
    width_emu: i64,
    height_emu: i64,
    relationship: Option<String>,
    metadata: Option<crate::PictureMetadata>,
    image_name: PartName,
    image_bytes: Vec<u8>,
    body: Vec<String>,
}

/// Inserts one inline PNG or JPEG picture and returns verified DOCX bytes.
pub fn insert_picture_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertPicture,
) -> Result<Vec<u8>, OperationResult> {
    let image = crate::read_image_info(&operation.image_bytes).map_err(image_info_error)?;
    let original = (
        i64::from(image.dimensions.width_px) * EMU_PER_PIXEL_AT_96_DPI,
        i64::from(image.dimensions.height_px) * EMU_PER_PIXEL_AT_96_DPI,
    );
    if (operation.auto_fit && operation.size.is_some())
        || (operation.layout.is_some() && (operation.auto_fit || operation.alignment.is_some()))
    {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "automatic fit requires inline layout and no explicit size; paragraph alignment requires inline layout",
        ));
    }
    if matches!(
        operation.placement,
        opensuite_protocol::PicturePlacement::HeaderFooter { .. }
    ) {
        return header_footer::insert(package, main, source, operation, original);
    }
    let (container, insertion, cell) = match &operation.placement {
        opensuite_protocol::PicturePlacement::Body(placement) => {
            let placement = resolve_paragraph_placement(source, placement)?;
            let (body, blocks) = direct_body_blocks(source)?;
            let index = placement_index(&blocks, placement)?;
            (body, body_insertion(source, body, &blocks, index)?, None)
        }
        opensuite_protocol::PicturePlacement::TableCell { handle, position } => {
            if operation.layout.is_some() {
                return Err(unsupported("table-cell pictures require inline layout"));
            }
            let cell = cell::resolve_insertion(package, main, source, handle, *position)?;
            (cell.cell, cell.insertion, Some(cell))
        }
        opensuite_protocol::PicturePlacement::HeaderFooter { .. } => unreachable!(),
    };
    let (width_emu, height_emu) = if operation.auto_fit {
        automatic_extent(package, main, source, insertion, original, cell.as_ref())?
    } else {
        let size = operation.size.or_else(|| {
            (original.0 > MAX_INLINE_PICTURE_WIDTH_EMU).then_some(
                opensuite_protocol::PictureSizeChange::WidthEmu(MAX_INLINE_PICTURE_WIDTH_EMU),
            )
        });
        sized_extent(original, size)?
    };
    insert_at(
        package,
        main,
        source,
        operation,
        container,
        insertion,
        (width_emu, height_emu),
    )
}

fn insert_at(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertPicture,
    container: NodeId,
    insertion: usize,
    extent: (i64, i64),
) -> Result<Vec<u8>, OperationResult> {
    let (width_emu, height_emu) = extent;
    let image = crate::read_image_info(&operation.image_bytes).map_err(image_info_error)?;
    let (extension, content_type) = match image.format {
        crate::ImageFormat::Png => ("png", PNG_CONTENT_TYPE),
        crate::ImageFormat::Jpeg => ("jpeg", JPEG_CONTENT_TYPE),
    };
    let media_name = next_media_part_name(package, extension)?;
    let (relationships_name, relationships_exist, relationships, relationship_id) =
        picture_relationships(package, main)?;
    let picture_id = next_picture_id(source)?;
    let fragment = picture_fragment(
        source,
        container,
        (width_emu, height_emu),
        picture_id,
        &relationship_id,
        operation.alt_text.as_deref(),
        operation.alignment,
    )?;
    let fragment = match &operation.layout {
        Some(layout) => anchored_fragment(fragment, layout)?,
        None => fragment,
    };
    let document = apply_patches(
        source,
        vec![Patch {
            span: SourceSpan {
                start: insertion,
                end: insertion,
            },
            replacement: fragment,
        }],
    )?;
    let mut replaced = vec![(main.name.clone(), document.as_slice())];
    let mut added = vec![(media_name.clone(), operation.image_bytes.as_slice())];
    let relationship_xml = append_xml_element(
        &relationships,
        &format!(
            r#"<Relationship Id="{relationship_id}" Type="{IMAGE_RELATIONSHIP_TYPE}" Target="{}"/>"#,
            if main.name.as_str().rsplit_once('/').map(|v| v.0) == Some("/word") {
                media_name
                    .as_str()
                    .strip_prefix("/word/")
                    .expect("media path")
            } else {
                media_name.as_str()
            }
        ),
    )?;
    if relationships_exist {
        replaced.push((relationships_name, relationship_xml.as_slice()));
    } else {
        added.push((relationships_name, relationship_xml.as_slice()));
    }
    let content_types_part = PartName::parse("/[Content_Types].xml")
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    if let Some(existing) = package.content_type_default(extension) {
        if existing.as_str() != content_type {
            return Err(OperationResult::failed(
                "PACKAGE_CONFLICT",
                "image file extension has a conflicting content type",
            ));
        }
    }
    if package.content_type_default(extension).is_none()
        || package.content_type_default("rels").is_none()
    {
        let bytes = package
            .read_part_by_name(&content_types_part)
            .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
        let defaults = [
            package
                .content_type_default(extension)
                .is_none()
                .then(|| format!(r#"<Default Extension="{extension}" ContentType="{content_type}"/>"#)),
            (!bytes
                .windows(b"Extension=\"rels\"".len())
                .any(|value| value == b"Extension=\"rels\""))
            .then(|| {
                r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#.to_owned()
            }),
        ]
        .into_iter()
        .flatten()
        .collect::<String>();
        let content_types = append_xml_element(&bytes, &defaults)?;
        replaced.push((content_types_part, content_types.as_slice()));
        // Keep the bytes alive until the package writer consumes the references.
        return write_inserted_picture(
            package,
            replaced,
            added,
            &content_types,
            InsertedPictureVerification {
                owner: &main.name,
                layout: operation.layout.as_ref(),
                alignment: operation.alignment,
                paragraph_start: insertion,
                container_start: source.node(container).expect("container").span().start,
                document_xml: &document,
                picture_id,
                width_emu,
                height_emu,
                image_bytes: &operation.image_bytes,
            },
        );
    }
    write_inserted_picture(
        package,
        replaced,
        added,
        &[],
        InsertedPictureVerification {
            owner: &main.name,
            layout: operation.layout.as_ref(),
            alignment: operation.alignment,
            paragraph_start: insertion,
            container_start: source.node(container).expect("container").span().start,
            document_xml: &document,
            picture_id,
            width_emu,
            height_emu,
            image_bytes: &operation.image_bytes,
        },
    )
}

/// Inserts one inline PNG or JPEG picture into a new DOCX artifact.
pub fn insert_picture(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertPicture,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let bytes = match insert_picture_to_vec(package, main, source, operation) {
        Ok(bytes) => bytes,
        Err(result) => return result,
    };
    let temporary = output.with_file_name(format!(
        ".opensuite-{}-{}.docx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if let Err(error) = std::fs::write(&temporary, bytes) {
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    if let Err(error) = std::fs::rename(&temporary, output) {
        let _ = std::fs::remove_file(&temporary);
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::picture_inserted()
}

pub(super) fn write_inserted_picture(
    package: &Package,
    replaced: Vec<(PartName, &[u8])>,
    added: Vec<(PartName, &[u8])>,
    _content_types: &[u8],
    expected: InsertedPictureVerification<'_>,
) -> Result<Vec<u8>, OperationResult> {
    let output = package
        .write_package_with_named_changes_to_vec(&replaced, &added)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    verify_inserted_picture_bytes(&output, &expected)?;
    Ok(output)
}

pub(super) fn image_info_error(error: crate::ImageDimensionError) -> OperationResult {
    let code = match error {
        crate::ImageDimensionError::UnsupportedFormat => "UNSUPPORTED_IMAGE_FORMAT",
        crate::ImageDimensionError::InvalidPng | crate::ImageDimensionError::InvalidJpeg => {
            "MALFORMED_IMAGE"
        }
    };
    OperationResult::failed(code, error.to_string())
}

pub(super) fn next_media_part_name(
    package: &Package,
    extension: &str,
) -> Result<PartName, OperationResult> {
    let next = package
        .part_names_with_prefix("/word/media/")
        .filter_map(|name| {
            name.as_str()
                .strip_prefix("/word/media/image")?
                .rsplit_once('.')
                .and_then(|(number, _)| number.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| {
            OperationResult::failed("PACKAGE_CONFLICT", "cannot allocate media part name")
        })?;
    PartName::parse(format!("/word/media/image{next}.{extension}"))
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))
}

pub(super) fn relationship_part_name(main: &Part) -> Result<PartName, OperationResult> {
    let name = main.name.as_str();
    let (directory, file) = name.rsplit_once('/').ok_or_else(|| {
        OperationResult::failed("PACKAGE_CONFLICT", "main document part has no file name")
    })?;
    PartName::parse(format!("{directory}/_rels/{file}.rels"))
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))
}

pub(super) fn picture_relationships(
    package: &Package,
    main: &Part,
) -> Result<(PartName, bool, Vec<u8>, String), OperationResult> {
    let relationships_part_name = relationship_part_name(main)?;
    let (relationships_exist, relationships) =
        match package.read_part_by_name(&relationships_part_name) {
            Ok(bytes) => (true, bytes),
            Err(PackageError::MissingTargetPart(_)) => (false, Vec::new()),
            Err(error) => return Err(OperationResult::failed(error.code(), error.to_string())),
        };
    let relationships = if relationships_exist {
        relationships
    } else {
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_vec()
    };
    let next = match package.part_relationships(main) {
        Ok(values) => {
            let mut used_ids = HashSet::new();
            if values.iter().any(|rel| !used_ids.insert(rel.id.as_str())) {
                return Err(unsupported("picture owner has duplicate relationship ids"));
            }
            values
                .iter()
                .filter_map(|relationship| {
                    relationship
                        .id
                        .as_str()
                        .strip_prefix("rId")?
                        .parse::<u32>()
                        .ok()
                })
                .max()
                .unwrap_or(0)
        }
        Err(PackageError::MissingPartRelationships(_)) => 0,
        Err(error) => return Err(OperationResult::failed(error.code(), error.to_string())),
    }
    .checked_add(1)
    .ok_or_else(|| OperationResult::failed("PACKAGE_CONFLICT", "cannot allocate relationship"))?;
    Ok((
        relationships_part_name,
        relationships_exist,
        relationships,
        format!("rId{next}"),
    ))
}

pub(super) fn next_picture_id(source: &SourceDocument) -> Result<u32, OperationResult> {
    crate::picture::pictures(source)
        .filter_map(|picture| picture.metadata()?.id?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| {
            OperationResult::failed("PACKAGE_CONFLICT", "cannot allocate picture identifier")
        })
}

pub(super) fn append_xml_element(bytes: &[u8], element: &str) -> Result<Vec<u8>, OperationResult> {
    let close = bytes
        .iter()
        .rposition(|byte| *byte == b'<')
        .ok_or_else(|| {
            OperationResult::failed("DOCUMENT_INVALID", "XML part has no closing element")
        })?;
    if !bytes[close..].starts_with(b"</") {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "XML part has no closing element",
        ));
    }
    let mut output = Vec::with_capacity(bytes.len() + element.len());
    output.extend_from_slice(&bytes[..close]);
    output.extend_from_slice(element.as_bytes());
    output.extend_from_slice(&bytes[close..]);
    Ok(output)
}

/// Deletes a dedicated picture paragraph, or only its drawing when paragraph
/// properties must remain (including section boundaries). Media and
/// relationships are deliberately preserved, even when they become unused.
pub fn delete_picture_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &DeletePicture,
) -> Result<Vec<u8>, OperationResult> {
    let handle = operation.target.handle.as_deref().ok_or_else(|| {
        OperationResult::failed(
            "TARGET_NOT_FOUND",
            "delete_picture requires a picture handle",
        )
    })?;
    let drawing = crate::inspection::picture_source_for_handle(package, main, source, handle)
        .ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "picture handle was not found")
        })?;
    let run = source
        .node(drawing)
        .and_then(|node| node.parent())
        .ok_or_else(|| OperationResult::failed("DOCUMENT_INVALID", "picture drawing has no run"))?;
    let paragraph = source
        .node(run)
        .and_then(|node| node.parent())
        .ok_or_else(|| {
            OperationResult::failed("DOCUMENT_INVALID", "picture run has no paragraph")
        })?;
    if !word(source, run, "r") || !word(source, paragraph, "p") {
        return Err(unsupported(
            "delete_picture supports only a direct picture run",
        ));
    }
    let run_children = source.children(run).collect::<Vec<_>>();
    let paragraph_children = source
        .children(paragraph)
        .filter(|id| !word(source, *id, "pPr"))
        .collect::<Vec<_>>();
    if run_children != [drawing] || paragraph_children != [run] {
        return Err(unsupported(
            "delete_picture supports only a dedicated picture paragraph",
        ));
    }
    if !crate::DocxDocument::new(source)
        .map_err(document_invalid)?
        .pictures()
        .find(|picture| picture.source_id() == drawing)
        .is_some_and(|picture| picture.is_simple())
    {
        return Err(unsupported(
            "delete_picture requires one simple picture drawing",
        ));
    }
    let keep_paragraph = source.children(paragraph).any(|id| word(source, id, "pPr"))
        || source
            .node(paragraph)
            .and_then(|node| node.parent())
            .is_some_and(|parent| word(source, parent, "tc"));
    let before = body_texts(source)?;
    let mut expected = before
        .iter()
        .map(|(_, text)| text.clone())
        .collect::<Vec<_>>();
    if !keep_paragraph {
        let index = before
            .iter()
            .position(|(id, _)| *id == paragraph)
            .ok_or_else(|| {
                OperationResult::failed("TARGET_NOT_FOUND", "picture paragraph was not found")
            })?;
        expected.remove(index);
    }
    let removed = if keep_paragraph { drawing } else { paragraph };
    let removed_span = source.node(removed).expect("picture source").span();
    protected_range::ensure_safe_picture_delete(source, removed_span, drawing)?;
    let output = write_patches_to_vec(
        package,
        main,
        source,
        vec![Patch {
            span: removed_span,
            replacement: Vec::new(),
        }],
    )?;
    verify_deleted_picture_bytes(&output, &expected, source, removed_span)?;
    Ok(output)
}

pub fn delete_picture(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &DeletePicture,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let bytes = match delete_picture_to_vec(package, main, source, operation) {
        Ok(bytes) => bytes,
        Err(result) => return result,
    };
    if let Err(error) = std::fs::write(output, bytes) {
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::picture_deleted()
}

pub(super) fn verify_deleted_picture_bytes(
    output: &[u8],
    expected: &[String],
    original: &SourceDocument,
    removed_span: SourceSpan,
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let mut expected_xml = original.original_bytes().to_vec();
    expected_xml.drain(removed_span.start..removed_span.end);
    if source.original_bytes() != expected_xml {
        return Err(document_invalid(
            "picture deletion changed unrelated source",
        ));
    }
    let actual = body_texts(&source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    (actual == expected)
        .then_some(())
        .ok_or_else(|| OperationResult::failed("DOCUMENT_INVALID", "output body ordering changed"))
}

pub fn set_picture_size_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetPictureSize,
) -> Result<Vec<u8>, OperationResult> {
    let handle = operation.target.handle.as_deref().ok_or_else(|| {
        OperationResult::failed(
            "TARGET_NOT_FOUND",
            "set_picture_size requires a picture handle",
        )
    })?;
    let drawing = crate::inspection::picture_source_for_handle(package, main, source, handle)
        .ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "picture handle was not found")
        })?;
    let picture = crate::DocxDocument::new(source)
        .map_err(document_invalid)?
        .pictures()
        .find(|p| p.source_id() == drawing)
        .ok_or_else(|| OperationResult::failed("TARGET_NOT_FOUND", "picture was not found"))?;
    let old = picture
        .extent()
        .map_err(|e| unsupported(e.to_string()))?
        .ok_or_else(|| unsupported("picture extent is missing"))?;
    let relationship = picture
        .relationship_id()
        .map_err(|e| unsupported(e.to_string()))?
        .map(str::to_owned);
    let metadata = picture.metadata();
    let crate::ImageReference::Embedded(image) = picture
        .image_reference(package, main)
        .map_err(|e| unsupported(e.to_string()))?
    else {
        return Err(unsupported("set_picture_size requires an embedded image"));
    };
    let image_name = image.part.name.clone();
    let image_bytes = package.read_part(&image.part).map_err(document_invalid)?;
    let body = body_texts(source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    let (cx, cy) = sized_extent((old.width_emu, old.height_emu), Some(operation.size))?;
    if !picture.is_simple() {
        return Err(unsupported("drawing is not a single direct image"));
    }
    let extents = picture_extent_nodes(source, drawing)?;
    for id in extents {
        let node = source.node(id).expect("picture extent exists");
        if node.attribute("cx") != Some(old.width_emu.to_string().as_str())
            || node.attribute("cy") != Some(old.height_emu.to_string().as_str())
        {
            return Err(unsupported("picture extents disagree"));
        }
    }
    let mut patches = Vec::new();
    for id in extents {
        for (name, value) in [("cx", cx), ("cy", cy)] {
            patches.push(layout::attribute_patch(
                source,
                id,
                name,
                &value.to_string(),
            )?);
        }
    }
    let output = write_patches_to_vec(package, main, source, patches)?;
    verify_resized_picture_bytes(
        &output,
        &PictureResizeVerification {
            handle: handle.to_owned(),
            width_emu: cx,
            height_emu: cy,
            relationship,
            metadata,
            image_name,
            image_bytes,
            body,
        },
    )?;
    Ok(output)
}

pub fn set_picture_size(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetPictureSize,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    let bytes = match set_picture_size_to_vec(package, main, source, operation) {
        Ok(bytes) => bytes,
        Err(result) => return result,
    };
    if let Err(error) = std::fs::write(output, bytes) {
        return OperationResult::failed("SERIALIZATION_FAILED", error.to_string());
    }
    OperationResult::picture_resized()
}

pub(super) fn picture_extent_nodes(
    source: &SourceDocument,
    drawing: NodeId,
) -> Result<[NodeId; 2], OperationResult> {
    crate::picture::pictures(source)
        .find(|p| p.source_id() == drawing)
        .and_then(|p| p.extent_nodes())
        .ok_or_else(|| unsupported("set_picture_size requires one drawing extent and one direct picture transform extent"))
}

pub(super) fn verify_resized_picture_bytes(
    output: &[u8],
    expected: &PictureResizeVerification,
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let fresh_handle = if let Some(value) = expected.handle.strip_prefix('d') {
        let index = value.split_once(':').map(|v| v.0).unwrap_or(value);
        format!(
            "d{index}:{:016x}",
            super::sections::section_fingerprint(&package, &main, &source)?
        )
    } else {
        expected.handle.clone()
    };
    let drawing =
        crate::inspection::picture_source_for_handle(&package, &main, &source, &fresh_handle)
            .ok_or_else(|| {
                OperationResult::failed("DOCUMENT_INVALID", "resized picture was not found")
            })?;
    let picture = crate::DocxDocument::new(&source)
        .map_err(document_invalid)?
        .pictures()
        .find(|picture| picture.source_id() == drawing)
        .ok_or_else(|| OperationResult::failed("DOCUMENT_INVALID", "resized picture is invalid"))?;
    let extent = picture.extent().map_err(document_invalid)?.ok_or_else(|| {
        OperationResult::failed("DOCUMENT_INVALID", "resized picture extent is missing")
    })?;
    if (extent.width_emu, extent.height_emu) != (expected.width_emu, expected.height_emu)
        || picture_extent_nodes(&source, drawing)?
            .into_iter()
            .any(|id| {
                let node = source.node(id).expect("picture extent exists");
                node.attribute("cx") != Some(expected.width_emu.to_string().as_str())
                    || node.attribute("cy") != Some(expected.height_emu.to_string().as_str())
            })
        || picture
            .relationship_id()
            .map_err(document_invalid)?
            .map(str::to_owned)
            != expected.relationship
        || picture.metadata() != expected.metadata
        || package
            .read_part_by_name(&expected.image_name)
            .map_err(document_invalid)?
            != expected.image_bytes
        || body_texts(&source)?
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            != expected.body
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "picture resize postcondition failed",
        ));
    }
    Ok(())
}

/// Replaces one supported picture and returns verified DOCX bytes.
pub fn replace_picture_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &ReplacePicture,
) -> Result<Vec<u8>, OperationResult> {
    if operation.target.handle.is_none()
        && operation.target.name.is_none()
        && operation.target.description.is_none()
    {
        return Err(OperationResult::failed(
            "TARGET_NOT_FOUND",
            "picture target requires name or description",
        ));
    }
    let document = match crate::DocxDocument::new(source) {
        Ok(value) => value,
        Err(error) => return Err(document_invalid(error)),
    };
    let target_drawing = operation.target.handle.as_deref().and_then(|handle| {
        crate::inspection::picture_source_for_handle(package, main, source, handle)
    });
    let mut pictures = document
        .pictures()
        .filter(|picture| {
            if operation.target.handle.is_some() {
                return target_drawing == Some(picture.source_id());
            }
            picture.metadata().is_some_and(|meta| {
                operation
                    .target
                    .name
                    .as_ref()
                    .is_none_or(|name| meta.name.as_ref() == Some(name))
                    && operation
                        .target
                        .description
                        .as_ref()
                        .is_none_or(|description| meta.description.as_ref() == Some(description))
            })
        })
        .collect::<Vec<_>>();
    let picture = if let Some(occurrence) = operation.target.occurrence {
        match pictures.get(occurrence) {
            Some(_) => pictures.remove(occurrence),
            None => {
                return Err(OperationResult::failed(
                    "TARGET_NOT_FOUND",
                    "picture target occurrence was not found",
                ));
            }
        }
    } else if pictures.len() != 1 {
        return Err(OperationResult::failed(
            if pictures.is_empty() {
                "TARGET_NOT_FOUND"
            } else {
                "TARGET_AMBIGUOUS"
            },
            "picture target did not resolve deterministically",
        ));
    } else {
        pictures.pop().expect("one picture")
    };
    if !picture.is_simple() {
        return Err(unsupported("replacement requires a single direct image"));
    }
    let crate::ImageReference::Embedded(image) = (match picture.image_reference(package, main) {
        Ok(value) => value,
        Err(error) => return Err(unsupported(error.to_string())),
    }) else {
        return Err(unsupported(
            "replace_picture supports only internal embedded images",
        ));
    };
    let content_type = image.part.content_type.as_str();
    if !matches!(content_type, "image/png" | "image/jpeg")
        || operation.replacement.content_type != content_type
        || !valid_image(&operation.replacement.bytes, content_type)
    {
        return Err(unsupported(
            "replacement image must be a valid PNG or JPEG with the existing content type",
        ));
    }
    let relationships = package.part_relationships(main).map_err(document_invalid)?;
    let image_ids = relationships.iter().filter(|rel| matches!(&rel.target, RelationshipTarget::Internal { part_name, .. } if *part_name == image.part.name)).map(|rel| rel.id.as_str()).collect::<Vec<_>>();
    let references = source
        .node_ids()
        .filter(|id| {
            let node = source.node(*id).expect("source node");
            match node.kind() {
                SourceNodeKind::Element { name, .. } if name.local_name() == "blip" => node
                    .attribute("embed")
                    .or_else(|| node.attribute("link"))
                    .is_some_and(|id| image_ids.contains(&id)),
                SourceNodeKind::Element { name, .. } if name.local_name() == "imagedata" => node
                    .attribute("id")
                    .is_some_and(|id| image_ids.contains(&id)),
                _ => false,
            }
        })
        .count();
    if references != 1 {
        return Err(unsupported(
            "replace_picture does not replace shared image parts",
        ));
    }
    // Other document parts can share the same asset, including headers and VML.
    for name in package
        .part_names_with_prefix("/")
        .filter(|name| **name != main.name && name.as_str().ends_with(".xml"))
    {
        let part = package.part(name).map_err(document_invalid)?;
        let relations = match package.part_relationships(&part) {
            Ok(values) => values,
            Err(PackageError::MissingPartRelationships(_)) => continue,
            Err(error) => return Err(document_invalid(error)),
        };
        if relations.iter().any(|rel| matches!(&rel.target, RelationshipTarget::Internal { part_name, .. } if *part_name == image.part.name)) {
            return Err(unsupported("replace_picture does not replace image parts shared with other document parts"));
        }
    }
    let output = package
        .write_replaced_part_to_vec(&image.part, &operation.replacement.bytes)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    let reopened = match Package::from_bytes(output.clone()) {
        Ok(value) => value,
        Err(error) => return Err(document_invalid(error)),
    };
    if let Err(error) = reopened.verify() {
        return Err(document_invalid(error));
    }
    let (reopened_main, reopened_source) = match crate::open_main_source(&reopened) {
        Ok(value) => value,
        Err(error) => return Err(document_invalid(error)),
    };
    let output_drawing = operation.target.handle.as_deref().and_then(|handle| {
        crate::inspection::picture_source_for_handle(
            &reopened,
            &reopened_main,
            &reopened_source,
            handle,
        )
    });
    let output_picture = match crate::DocxDocument::new(&reopened_source) {
        Ok(document) => document
            .pictures()
            .filter(|picture| {
                if operation.target.handle.is_some() {
                    return output_drawing == Some(picture.source_id());
                }
                picture.metadata().is_some_and(|meta| {
                    operation
                        .target
                        .name
                        .as_ref()
                        .is_none_or(|name| meta.name.as_ref() == Some(name))
                        && operation
                            .target
                            .description
                            .as_ref()
                            .is_none_or(|description| {
                                meta.description.as_ref() == Some(description)
                            })
                })
            })
            .nth(operation.target.occurrence.unwrap_or(0)),
        Err(error) => return Err(document_invalid(error)),
    };
    let Some(output_picture) = output_picture else {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output picture target was not found",
        ));
    };
    if output_picture.kind() != picture.kind()
        || output_picture.extent().ok() != picture.extent().ok()
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output picture metadata or dimensions changed",
        ));
    }
    let crate::ImageReference::Embedded(output_image) =
        (match output_picture.image_reference(&reopened, &reopened_main) {
            Ok(value) => value,
            Err(error) => return Err(document_invalid(error)),
        })
    else {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output picture image relationship changed",
        ));
    };
    if output_image.part.content_type != image.part.content_type
        || reopened
            .read_part(&output_image.part)
            .map_err(document_invalid)
            .ok()
            .as_deref()
            != Some(operation.replacement.bytes.as_slice())
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "output picture payload does not match replacement",
        ));
    }
    Ok(output)
}

pub fn replace_picture(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &ReplacePicture,
    output: impl AsRef<Path>,
) -> OperationResult {
    write_operation_output(
        package,
        output.as_ref(),
        replace_picture_to_vec(package, main, source, operation),
    )
}

pub(super) fn valid_image(bytes: &[u8], content_type: &str) -> bool {
    match content_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]) && bytes.ends_with(&[0xff, 0xd9]),
        _ => false,
    }
}

pub(super) fn verify_inserted_picture_bytes(
    output: &[u8],
    expected: &InsertedPictureVerification<'_>,
) -> Result<(), OperationResult> {
    let picture_id = expected.picture_id;
    let width_emu = expected.width_emu;
    let height_emu = expected.height_emu;
    let image_bytes = expected.image_bytes;
    let expected_layout = expected.layout;
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let main = package.part(expected.owner).map_err(document_invalid)?;
    let source = SourceDocument::parse(package.read_part(&main).map_err(document_invalid)?)
        .map_err(document_invalid)?;
    if source.original_bytes() != expected.document_xml {
        return Err(document_invalid(
            "picture insertion changed unrelated source",
        ));
    }
    let paragraph = source
        .node_ids()
        .find(|id| {
            word(&source, *id, "p")
                && source.node(*id).is_some_and(|node| {
                    node.span().start == expected.paragraph_start
                        && node
                            .parent()
                            .and_then(|id| source.node(id))
                            .is_some_and(|parent| parent.span().start == expected.container_start)
                })
        })
        .ok_or_else(|| document_invalid("picture is not at the requested body or cell position"))?;
    let properties = sections::section_child(&source, paragraph, "pPr")?;
    let alignment = properties
        .map(|id| sections::section_child(&source, id, "jc"))
        .transpose()?
        .flatten();
    if alignment.and_then(|id| source.node(id)?.attribute("val"))
        != expected.alignment.map(alignment_value)
    {
        return Err(document_invalid("inserted picture alignment changed"));
    }
    let picture = crate::picture::pictures(&source)
        .find(|picture| {
            picture.metadata().and_then(|metadata| metadata.id) == Some(picture_id.to_string())
        })
        .ok_or_else(|| {
            OperationResult::failed("DOCUMENT_INVALID", "inserted picture was not found")
        })?;
    if !picture.is_simple()
        || picture
            .extent()
            .map_err(|error| unsupported(error.to_string()))?
            != Some(crate::PictureExtent {
                width_emu,
                height_emu,
            })
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "inserted picture dimensions changed",
        ));
    }
    if let Some(expected) = expected_layout {
        let mut expected = expected.clone();
        expected.wrap = Some(
            expected
                .wrap
                .unwrap_or(opensuite_protocol::ImageWrap::Square),
        );
        if !picture
            .anchor_facts()
            .is_some_and(|facts| layout::layout_matches(&facts, &expected))
        {
            return Err(document_invalid(
                "inserted image layout does not match request",
            ));
        }
    } else if picture.kind() != crate::PictureKind::Inline {
        return Err(document_invalid("inserted image is not inline"));
    }
    let crate::ImageReference::Embedded(image) = picture
        .image_reference(&package, &main)
        .map_err(|error| unsupported(error.to_string()))?
    else {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "inserted picture has no embedded image",
        ));
    };
    if !matches!(
        image.part.content_type.as_str(),
        PNG_CONTENT_TYPE | JPEG_CONTENT_TYPE
    ) || package.read_part(&image.part).map_err(document_invalid)? != image_bytes
    {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "inserted image payload does not match request",
        ));
    }
    Ok(())
}
