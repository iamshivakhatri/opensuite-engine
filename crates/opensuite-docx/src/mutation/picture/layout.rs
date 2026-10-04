use super::*;
use opensuite_protocol::{
    ImageAlignment, ImageAxisPosition, ImagePosition, ImagePositionReference, ImageWrap,
    PictureLayoutPatch, PictureSizeChange, SetPictureLayout,
};
use std::fmt::Write as _;

/// One integer sizing rule for insertion and resize; single dimensions preserve ratio.
pub(super) fn sized_extent(
    old: (i64, i64),
    size: Option<PictureSizeChange>,
) -> Result<(i64, i64), OperationResult> {
    let ratio = |a: i64, b: i64, c: i64| {
        i128::from(a)
            .checked_mul(i128::from(b))
            .and_then(|v| v.checked_div(i128::from(c)))
            .and_then(|v| i64::try_from(v).ok())
            .unwrap_or(0)
    };
    let (width, height) = match size {
        None => old,
        Some(PictureSizeChange::WidthEmu(width)) => (width, ratio(old.1, width, old.0)),
        Some(PictureSizeChange::HeightEmu(height)) => (ratio(old.0, height, old.1), height),
        Some(PictureSizeChange::ExactEmu { width, height }) => (width, height),
    };
    if width <= 0 || height <= 0 || width > u32::MAX as i64 || height > u32::MAX as i64 {
        return Err(OperationResult::failed(
            "INVALID_IMAGE_DIMENSIONS",
            "image dimensions must be positive and within Word's EMU range",
        ));
    }
    Ok((width, height))
}

fn position_values(
    axis: ImageAxisPosition,
    horizontal: bool,
) -> Result<(&'static str, &'static str, String), OperationResult> {
    let reference = match (horizontal, axis.reference) {
        (_, ImagePositionReference::Page) => "page",
        (_, ImagePositionReference::Margin) => "margin",
        (true, ImagePositionReference::Column) => "column",
        (false, ImagePositionReference::Paragraph) => "paragraph",
        _ => {
            return Err(OperationResult::failed(
                "INVALID_IMAGE_POSITION",
                "horizontal references: page/margin/column; vertical references: page/margin/paragraph",
            ));
        }
    };
    let position = match axis.position {
        ImagePosition::Align(alignment) => {
            if !horizontal && axis.reference == ImagePositionReference::Paragraph {
                return Err(OperationResult::failed(
                    "INVALID_IMAGE_POSITION",
                    "paragraph-relative vertical positioning requires an offset",
                ));
            }
            let value = match (horizontal, alignment) {
                (true, ImageAlignment::Start) => "left",
                (true, ImageAlignment::End) => "right",
                (false, ImageAlignment::Start) => "top",
                (false, ImageAlignment::End) => "bottom",
                (_, ImageAlignment::Center) => "center",
            };
            ("align", value.to_owned())
        }
        ImagePosition::OffsetEmu(value) => {
            if i32::try_from(value).is_err() {
                return Err(OperationResult::failed(
                    "INVALID_IMAGE_POSITION",
                    "offset must fit a signed 32-bit EMU value",
                ));
            }
            ("posOffset", value.to_string())
        }
    };
    Ok((reference, position.0, position.1))
}
fn position_xml(axis: ImageAxisPosition, horizontal: bool) -> Result<String, OperationResult> {
    let (reference, property, value) = position_values(axis, horizontal)?;
    let tag = if horizontal { "positionH" } else { "positionV" };
    Ok(format!(
        r#"<wp:{tag} xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" relativeFrom="{reference}"><wp:{property}>{value}</wp:{property}></wp:{tag}>"#
    ))
}
fn wrap_xml(wrap: ImageWrap) -> &'static str {
    match wrap {
        ImageWrap::Square => {
            r#"<wp:wrapSquare xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" wrapText="bothSides"/>"#
        }
        ImageWrap::TopAndBottom => {
            r#"<wp:wrapTopAndBottom xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"/>"#
        }
        ImageWrap::BehindText | ImageWrap::InFrontOfText => {
            r#"<wp:wrapNone xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"/>"#
        }
    }
}
fn distances(layout: &PictureLayoutPatch) -> Result<Vec<(&'static str, i64)>, OperationResult> {
    let values = [
        ("distT", layout.distance.top_emu),
        ("distB", layout.distance.bottom_emu),
        ("distL", layout.distance.left_emu),
        ("distR", layout.distance.right_emu),
    ]
    .into_iter()
    .filter_map(|(key, v)| v.map(|v| (key, v)))
    .collect::<Vec<_>>();
    if values.iter().any(|(_, v)| *v < 0 || *v > u32::MAX as i64) {
        return Err(OperationResult::failed(
            "INVALID_IMAGE_POSITION",
            "text distances must be unsigned 32-bit EMU values",
        ));
    }
    Ok(values)
}

/// Only newly authored inline fragments are transformed; imported drawings are patched below.
pub(super) fn anchored_fragment(
    fragment: Vec<u8>,
    layout: &PictureLayoutPatch,
) -> Result<Vec<u8>, OperationResult> {
    let horizontal = layout.horizontal.ok_or_else(|| {
        OperationResult::failed(
            "INVALID_IMAGE_POSITION",
            "floating insertion requires both position axes",
        )
    })?;
    let vertical = layout.vertical.ok_or_else(|| {
        OperationResult::failed(
            "INVALID_IMAGE_POSITION",
            "floating insertion requires both position axes",
        )
    })?;
    let wrap = layout.wrap.unwrap_or(ImageWrap::Square);
    let mut xml = String::from_utf8(fragment).map_err(document_invalid)?;
    let mut attrs = [("distT", 0), ("distB", 0), ("distL", 0), ("distR", 0)]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    attrs.extend(distances(layout)?);
    let mut distance = String::new();
    for (key, value) in attrs {
        write!(distance, r#" {key}="{value}""#).expect("write to String");
    }
    let start = xml.find("<wp:inline").expect("authored inline");
    let end = start + xml[start..].find('>').expect("authored start tag") + 1;
    xml.replace_range(start..end, &format!(r#"<wp:anchor xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"{distance} simplePos="0" relativeHeight="0" behindDoc="{}" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/>{}{}"#, u8::from(wrap == ImageWrap::BehindText), position_xml(horizontal, true)?, position_xml(vertical, false)?));
    let extent_end = xml.find("/><wp:docPr").expect("authored extent") + 2;
    xml.insert_str(extent_end, wrap_xml(wrap));
    Ok(xml.replace("</wp:inline>", "</wp:anchor>").into_bytes())
}

/// Patch a single start-tag attribute, retaining unknown attributes and original quoting.
pub(super) fn attribute_patch(
    source: &SourceDocument,
    id: NodeId,
    key: &str,
    value: &str,
) -> Result<Patch, OperationResult> {
    let node = source.node(id).expect("layout node");
    let SourceNodeKind::Element { start_tag, .. } = node.kind() else {
        return Err(unsupported("layout node is not an element"));
    };
    let tag = &source.original_bytes()[start_tag.start..start_tag.end];
    if node.attribute(key).is_none() {
        let end = start_tag.end - if tag.ends_with(b"/>") { 2 } else { 1 };
        return Ok(Patch {
            span: SourceSpan { start: end, end },
            replacement: format!(r#" {key}="{value}""#).into_bytes(),
        });
    }
    // Tokenize this start tag only; keep original attribute quoting and whitespace.
    let mut reader = quick_xml::Reader::from_reader(tag);
    let event = match reader.read_event().map_err(document_invalid)? {
        quick_xml::events::Event::Start(event) | quick_xml::events::Event::Empty(event) => event,
        _ => return Err(unsupported("unpatchable layout tag")),
    };
    let attribute = event
        .attributes()
        .with_checks(true)
        .find_map(|a| match a {
            Ok(a) if a.key.as_ref() == key.as_bytes() => Some(Ok(a)),
            Err(e) => Some(Err(e)),
            _ => None,
        })
        .transpose()
        .map_err(document_invalid)?
        .ok_or_else(|| unsupported("unpatchable attribute"))?;
    let offset = attribute.value.as_ptr() as usize - tag.as_ptr() as usize;
    Ok(Patch {
        span: SourceSpan {
            start: start_tag.start + offset,
            end: start_tag.start + offset + attribute.value.len(),
        },
        replacement: value.as_bytes().to_vec(),
    })
}

pub fn set_picture_layout_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetPictureLayout,
) -> Result<Vec<u8>, OperationResult> {
    let handle = operation.target.handle.as_deref().ok_or_else(|| {
        OperationResult::failed(
            "TARGET_NOT_FOUND",
            "image layout requires a fresh image handle",
        )
    })?;
    let drawing = crate::inspection::picture_source_for_handle(package, main, source, handle)
        .ok_or_else(|| {
            OperationResult::failed("TARGET_NOT_FOUND", "image handle is invalid or stale")
        })?;
    let picture = crate::picture::pictures(source)
        .find(|p| p.source_id() == drawing)
        .ok_or_else(|| unsupported("drawing is not a picture"))?;
    let facts = picture.anchor_facts().ok_or_else(|| {
        unsupported(
            "inline to floating conversion is not supported; insert a floating image instead",
        )
    })?;
    if !facts.editable {
        return Err(OperationResult::failed(
            "UNSUPPORTED_ANCHORED_IMAGE_STRUCTURE",
            "anchor uses unsupported structure or wrap geometry",
        ));
    }
    let layout = &operation.layout;
    let mut patches = Vec::new();
    for (local, position, horizontal) in [
        ("positionH", layout.horizontal, true),
        ("positionV", layout.vertical, false),
    ] {
        if let Some(position) = position {
            let id = picture
                .layout_child(local)
                .ok_or_else(|| unsupported("anchor position is missing"))?;
            if source
                .children(picture.container_id())
                .filter(|id| picture_layout_name(source, *id) == Some(local))
                .count()
                != 1
            {
                return Err(unsupported("anchor position is ambiguous"));
            }
            // Preserve position attributes/extensions; replace just the one align/offset child.
            let children = source
                .children(id)
                .filter(|id| {
                    matches!(
                        picture_layout_name(source, *id),
                        Some("align" | "posOffset")
                    )
                })
                .collect::<Vec<_>>();
            if children.len() != 1 {
                return Err(unsupported("anchor position is ambiguous"));
            }
            let (reference, property, value) = position_values(position, horizontal)?;
            patches.push(attribute_patch(source, id, "relativeFrom", reference)?);
            let old_child = source.node(children[0]).expect("position");
            let contents = source.children(children[0]).collect::<Vec<_>>();
            if contents.len() != 1
                || !matches!(
                    source.node(contents[0]).map(|n| n.kind()),
                    Some(SourceNodeKind::Text)
                )
            {
                return Err(unsupported("position contains unsupported child content"));
            }
            let patch = if picture_layout_name(source, children[0]) == Some(property) {
                Patch {
                    span: source.node(contents[0]).expect("position text").span(),
                    replacement: value.into_bytes(),
                }
            } else {
                if matches!(old_child.kind(), SourceNodeKind::Element { attributes, .. } if !attributes.is_empty())
                {
                    return Err(unsupported(
                        "position property has imported attributes that cannot be replaced safely",
                    ));
                }
                let namespace = drawing_namespace(source, id)?;
                Patch {
                    span: old_child.span(),
                    replacement: format!(
                        r#"<wp:{property} xmlns:wp="{namespace}">{value}</wp:{property}>"#
                    )
                    .into_bytes(),
                }
            };
            patches.push(patch);
        }
    }
    if let Some(wrap) = layout.wrap {
        let nodes = source
            .children(picture.container_id())
            .filter(|id| {
                matches!(
                    picture_layout_name(source, *id),
                    Some(
                        "wrapNone"
                            | "wrapSquare"
                            | "wrapTopAndBottom"
                            | "wrapTight"
                            | "wrapThrough"
                    )
                )
            })
            .collect::<Vec<_>>();
        if nodes.len() != 1 {
            return Err(unsupported("anchor wrap is ambiguous"));
        }
        let new_name = match wrap {
            ImageWrap::Square => "wrapSquare",
            ImageWrap::TopAndBottom => "wrapTopAndBottom",
            _ => "wrapNone",
        };
        if picture_layout_name(source, nodes[0]) != Some(new_name) {
            let node = source.node(nodes[0]).expect("wrap");
            if source.children(nodes[0]).next().is_some()
                || matches!(node.kind(), SourceNodeKind::Element { attributes, .. } if attributes.iter().any(|a| !matches!(a.local_name(), "wrapText" | "wp")))
            {
                return Err(unsupported(
                    "wrap contains imported extensions that cannot be replaced safely",
                ));
            }
            patches.push(Patch {
                span: node.span(),
                replacement: wrap_xml(wrap)
                    .replace(
                        "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing",
                        drawing_namespace(source, nodes[0])?,
                    )
                    .into_bytes(),
            });
        }
        patches.push(attribute_patch(
            source,
            picture.container_id(),
            "behindDoc",
            if wrap == ImageWrap::BehindText {
                "1"
            } else {
                "0"
            },
        )?);
    }
    for (key, value) in distances(layout)? {
        for local in ["wrapNone", "wrapSquare", "wrapTopAndBottom"] {
            if let Some(id) = picture
                .layout_child(local)
                .filter(|id| source.node(*id).is_some_and(|n| n.attribute(key).is_some()))
            {
                patches.push(attribute_patch(source, id, key, &value.to_string())?);
            }
        }
        patches.push(attribute_patch(
            source,
            picture.container_id(),
            key,
            &value.to_string(),
        )?);
    }
    if patches.is_empty() {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "image layout patch is empty",
        ));
    }
    let output = write_patches_to_vec(package, main, source, patches)?;
    let reopened = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    let (_, new_source) = crate::open_main_source(&reopened).map_err(document_invalid)?;
    let index = crate::picture::pictures(source)
        .position(|p| p.source_id() == drawing)
        .expect("target");
    let updated = crate::picture::pictures(&new_source)
        .nth(index)
        .ok_or_else(|| document_invalid("updated picture missing"))?;
    let actual = updated
        .anchor_facts()
        .ok_or_else(|| document_invalid("updated anchor missing"))?;
    if !layout_matches(&actual, layout)
        || updated.extent().ok() != picture.extent().ok()
        || updated.metadata() != picture.metadata()
        || updated.relationship_id().ok() != picture.relationship_id().ok()
    {
        return Err(document_invalid(
            "image layout changed unrelated picture facts",
        ));
    }
    Ok(output)
}
pub(super) fn layout_matches(
    actual: &crate::ImageAnchorFacts,
    layout: &PictureLayoutPatch,
) -> bool {
    let axis_matches = |requested: Option<ImageAxisPosition>,
                        actual: &Option<crate::picture::ImagePositionFacts>,
                        horizontal: bool| {
        requested.is_none_or(|axis| {
            let Ok((reference, property, value)) = position_values(axis, horizontal) else {
                return false;
            };
            let Some(actual) = actual else {
                return false;
            };
            actual.reference.as_deref() == Some(reference)
                && match axis.position {
                    ImagePosition::OffsetEmu(v) => {
                        actual.offset_emu == Some(v) && actual.alignment.is_none()
                    }
                    ImagePosition::Align(_) => {
                        actual.offset_emu.is_none()
                            && actual
                                .alignment
                                .as_ref()
                                .is_some_and(|v| property == "align" && *v == value)
                    }
                }
        })
    };
    let wrap_matches = layout.wrap.is_none_or(|v| {
        actual.wrap.as_deref()
            == Some(match v {
                ImageWrap::Square => "square",
                ImageWrap::TopAndBottom => "topAndBottom",
                ImageWrap::BehindText => "behindText",
                ImageWrap::InFrontOfText => "inFrontOfText",
            })
    });
    let distance_matches = [
        ("top", layout.distance.top_emu),
        ("bottom", layout.distance.bottom_emu),
        ("left", layout.distance.left_emu),
        ("right", layout.distance.right_emu),
    ]
    .into_iter()
    .all(|(key, v)| v.is_none_or(|v| actual.distance_emu.get(key) == Some(&v)));
    axis_matches(layout.horizontal, &actual.horizontal, true)
        && axis_matches(layout.vertical, &actual.vertical, false)
        && wrap_matches
        && distance_matches
}
fn drawing_namespace(source: &SourceDocument, id: NodeId) -> Result<&str, OperationResult> {
    match source.node(id).map(|n| n.kind()) {
        Some(SourceNodeKind::Element { name, .. }) => name
            .namespace_uri()
            .ok_or_else(|| unsupported("drawing namespace missing")),
        _ => Err(unsupported("drawing element missing")),
    }
}
fn picture_layout_name(source: &SourceDocument, id: NodeId) -> Option<&str> {
    match source.node(id)?.kind() {
        SourceNodeKind::Element { name, .. }
            if name.namespace_uri().is_some_and(|uri| {
                uri == "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
                    || uri == "http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing"
            }) =>
        {
            Some(name.local_name())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
