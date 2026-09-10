use super::*;

/// Inserts one inline PNG or JPEG picture and returns verified DOCX bytes.
pub fn insert_picture_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &InsertPicture,
) -> Result<Vec<u8>, OperationResult> {
    let image = crate::read_image_info(&operation.image_bytes).map_err(image_info_error)?;
    let (extension, content_type) = match image.format {
        crate::ImageFormat::Png => ("png", PNG_CONTENT_TYPE),
        crate::ImageFormat::Jpeg => ("jpeg", JPEG_CONTENT_TYPE),
    };
    let (width_emu, height_emu) = picture_dimensions(image.dimensions);
    let placement = resolve_paragraph_placement(source, &operation.placement)?;
    let (body, blocks) = direct_body_blocks(source)?;
    let index = placement_index(&blocks, placement)?;
    let insertion = body_insertion(source, body, &blocks, index)?;
    let media_name = next_media_part_name(package, extension)?;
    let (relationships_name, relationships_exist, relationships, relationship_id) =
        picture_relationships(package, main)?;
    let picture_id = next_picture_id(source)?;
    let fragment = picture_fragment_for_body(
        source,
        body,
        width_emu,
        height_emu,
        picture_id,
        &relationship_id,
        operation.alt_text.as_deref(),
    )?;
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
            r#"<Relationship Id="{relationship_id}" Type="{IMAGE_RELATIONSHIP_TYPE}" Target="media/{}"/>"#,
            media_name
                .as_str()
                .rsplit('/')
                .next()
                .expect("media file name")
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
                index,
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
            index,
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
    verify_inserted_picture_bytes(
        &output,
        expected.index,
        expected.picture_id,
        expected.width_emu,
        expected.height_emu,
        expected.image_bytes,
    )?;
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

pub(super) fn picture_dimensions(dimensions: crate::ImageDimensions) -> (i64, i64) {
    let mut width = i64::from(dimensions.width_px) * EMU_PER_PIXEL_AT_96_DPI;
    let mut height = i64::from(dimensions.height_px) * EMU_PER_PIXEL_AT_96_DPI;
    if width > MAX_INLINE_PICTURE_WIDTH_EMU {
        height = height * MAX_INLINE_PICTURE_WIDTH_EMU / width;
        width = MAX_INLINE_PICTURE_WIDTH_EMU;
    }
    (width, height)
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
        Ok(values) => values
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
            .unwrap_or(0),
        Err(PackageError::MissingPartRelationships(_)) => 0,
        Err(error) => return Err(OperationResult::failed(error.code(), error.to_string())),
    }
    .checked_add(1)
    .ok_or_else(|| {
        OperationResult::failed("PACKAGE_CONFLICT", "cannot allocate image relationship")
    })?;
    Ok((
        relationships_part_name,
        relationships_exist,
        relationships,
        format!("rId{next}"),
    ))
}

pub(super) fn next_picture_id(source: &SourceDocument) -> Result<u32, OperationResult> {
    crate::DocxDocument::new(source)
        .map_err(document_invalid)?
        .pictures()
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

/// Deletes one safe, direct main-body paragraph selected by Current-view text.
/// Deletes only the source paragraph holding one supported picture. Media and
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
    let before = body_texts(source)?;
    let index = before
        .iter()
        .position(|(id, _)| *id == paragraph)
        .ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "picture paragraph was not found")
        })?;
    let mut expected = before.into_iter().map(|(_, text)| text).collect::<Vec<_>>();
    expected.remove(index);
    let output = write_patches_to_vec(
        package,
        main,
        source,
        vec![Patch {
            span: source
                .node(paragraph)
                .expect("picture paragraph exists")
                .span(),
            replacement: Vec::new(),
        }],
    )?;
    verify_deleted_picture_bytes(&output, &expected)?;
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
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (_, source) = crate::open_main_source(&package).map_err(document_invalid)?;
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
    let (cx, cy) = match operation.size {
        opensuite_protocol::PictureSizeChange::WidthEmu(cx) => (
            cx,
            old.height_emu
                .checked_mul(cx)
                .and_then(|v| v.checked_div(old.width_emu))
                .unwrap_or(0),
        ),
        opensuite_protocol::PictureSizeChange::HeightEmu(cy) => (
            old.width_emu
                .checked_mul(cy)
                .and_then(|v| v.checked_div(old.height_emu))
                .unwrap_or(0),
            cy,
        ),
    };
    if cx <= 0 || cy <= 0 {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "picture dimensions must be positive and not overflow",
        ));
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
            let node = source.node(id).unwrap();
            let old = node
                .attribute(name)
                .ok_or_else(|| unsupported("picture extent is missing a dimension"))?;
            let SourceNodeKind::Element { start_tag, .. } = node.kind() else {
                unreachable!()
            };
            let tag = &source.original_bytes()[start_tag.start..start_tag.end];
            let needle = format!("{name}=\"{old}\"");
            let offset = tag
                .windows(needle.len())
                .position(|x| x == needle.as_bytes())
                .ok_or_else(|| unsupported("picture extent is not patchable"))?;
            let start = start_tag.start + offset + name.len() + 2;
            patches.push(Patch {
                span: SourceSpan {
                    start,
                    end: start + old.len(),
                },
                replacement: value.to_string().into_bytes(),
            });
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
    let mut inline = Vec::new();
    let mut transform = Vec::new();
    let mut todo = vec![drawing];
    while let Some(id) = todo.pop() {
        todo.extend(source.children(id));
        let Some(node) = source.node(id) else {
            continue;
        };
        let SourceNodeKind::Element { name, .. } = node.kind() else {
            continue;
        };
        if name.local_name() != "extent" && name.local_name() != "ext" {
            continue;
        }
        let parent = node.parent().and_then(|parent| source.node(parent));
        if name.local_name() == "extent" && parent.is_some_and(|parent| matches!(parent.kind(), SourceNodeKind::Element { name, .. } if name.local_name() == "inline")) {
            inline.push(id);
        }
        if name.local_name() == "ext" && parent.is_some_and(|parent| matches!(parent.kind(), SourceNodeKind::Element { name, .. } if name.local_name() == "xfrm")) {
            transform.push(id);
        }
    }
    match (inline.as_slice(), transform.as_slice()) {
        ([inline], [transform]) => Ok([*inline, *transform]),
        _ => Err(unsupported(
            "set_picture_size requires one inline extent and one transform extent",
        )),
    }
}

pub(super) fn verify_resized_picture_bytes(
    output: &[u8],
    expected: &PictureResizeVerification,
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let drawing =
        crate::inspection::picture_source_for_handle(&package, &main, &source, &expected.handle)
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

pub fn replace_picture(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &ReplacePicture,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if operation.target.handle.is_none()
        && operation.target.name.is_none()
        && operation.target.description.is_none()
    {
        return OperationResult::failed(
            "TARGET_NOT_FOUND",
            "picture target requires name or description",
        );
    }
    let document = match crate::DocxDocument::new(source) {
        Ok(value) => value,
        Err(error) => return document_invalid(error),
    };
    let mut pictures = document
        .pictures()
        .filter(|picture| {
            if let Some(handle) = &operation.target.handle {
                return crate::inspection::picture_source_for_handle(package, main, source, handle)
                    == Some(picture.source_id());
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
                return OperationResult::failed(
                    "TARGET_NOT_FOUND",
                    "picture target occurrence was not found",
                );
            }
        }
    } else if pictures.len() != 1 {
        return OperationResult::failed(
            if pictures.is_empty() {
                "TARGET_NOT_FOUND"
            } else {
                "TARGET_AMBIGUOUS"
            },
            "picture target did not resolve deterministically",
        );
    } else {
        pictures.pop().expect("one picture")
    };
    let metadata = picture.metadata().expect("matched metadata");
    let name = metadata
        .name
        .clone()
        .or(metadata.description.clone())
        .unwrap_or_else(|| "picture".to_owned());
    let crate::ImageReference::Embedded(image) = (match picture.image_reference(package, main) {
        Ok(value) => value,
        Err(error) => return unsupported(error.to_string()),
    }) else {
        return unsupported("replace_picture supports only internal embedded images");
    };
    let content_type = image.part.content_type.as_str();
    if !matches!(content_type, "image/png" | "image/jpeg")
        || operation.replacement.content_type != content_type
        || !valid_image(&operation.replacement.bytes, content_type)
    {
        return unsupported(
            "replacement image must be a valid PNG or JPEG with the existing content type",
        );
    }
    let references = match crate::DocxDocument::new(source) { Ok(document) => document.pictures().filter_map(|item| item.image_reference(package, main).ok()).filter(|reference| matches!(reference, crate::ImageReference::Embedded(other) if other.part.name == image.part.name)).count(), Err(error) => return document_invalid(error) };
    if references != 1 {
        return unsupported("replace_picture does not replace shared image parts");
    }
    if let Err(error) =
        package.write_replaced_part(&image.part, &operation.replacement.bytes, output)
    {
        return OperationResult::failed(error.code(), error.to_string());
    }
    let reopened = match Package::open(output) {
        Ok(value) => value,
        Err(error) => return document_invalid(error),
    };
    if let Err(error) = reopened.verify() {
        return document_invalid(error);
    }
    let (reopened_main, reopened_source) = match crate::open_main_source(&reopened) {
        Ok(value) => value,
        Err(error) => return document_invalid(error),
    };
    let output_picture = match crate::DocxDocument::new(&reopened_source) {
        Ok(document) => document
            .pictures()
            .filter(|picture| {
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
        Err(error) => return document_invalid(error),
    };
    let Some(output_picture) = output_picture else {
        return OperationResult::failed("DOCUMENT_INVALID", "output picture target was not found");
    };
    if output_picture.kind() != picture.kind()
        || output_picture.extent().ok() != picture.extent().ok()
    {
        return OperationResult::failed(
            "DOCUMENT_INVALID",
            "output picture metadata or dimensions changed",
        );
    }
    let crate::ImageReference::Embedded(output_image) =
        (match output_picture.image_reference(&reopened, &reopened_main) {
            Ok(value) => value,
            Err(error) => return document_invalid(error),
        })
    else {
        return OperationResult::failed(
            "DOCUMENT_INVALID",
            "output picture image relationship changed",
        );
    };
    if output_image.part.content_type != image.part.content_type
        || reopened
            .read_part(&output_image.part)
            .map_err(document_invalid)
            .ok()
            .as_deref()
            != Some(operation.replacement.bytes.as_slice())
    {
        return OperationResult::failed(
            "DOCUMENT_INVALID",
            "output picture payload does not match replacement",
        );
    }
    OperationResult::picture_replaced(
        name,
        image.size_bytes as usize,
        operation.replacement.bytes.len(),
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
    expected_index: usize,
    picture_id: u32,
    width_emu: i64,
    height_emu: i64,
    image_bytes: &[u8],
) -> Result<(), OperationResult> {
    let package = Package::from_bytes(output.to_vec()).map_err(document_invalid)?;
    package.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&package).map_err(document_invalid)?;
    let (_, blocks) = direct_body_blocks(&source)?;
    let paragraph = blocks.get(expected_index).ok_or_else(|| {
        OperationResult::failed(
            "DOCUMENT_INVALID",
            "inserted picture paragraph was not found",
        )
    })?;
    if !word(&source, *paragraph, "p") {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "picture is not at the requested body position",
        ));
    }
    let picture = crate::DocxDocument::new(&source)
        .map_err(document_invalid)?
        .pictures()
        .find(|picture| {
            picture.metadata().and_then(|metadata| metadata.id) == Some(picture_id.to_string())
        })
        .ok_or_else(|| {
            OperationResult::failed("DOCUMENT_INVALID", "inserted picture was not found")
        })?;
    if picture.kind() != crate::PictureKind::Inline
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
