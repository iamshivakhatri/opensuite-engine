//! Bounded, read-only access to original embedded image payloads.
use super::*;
use opensuite_protocol::OperationResult;
use serde::Serialize;

pub const MAX_EMBEDDED_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_IMAGE_PIXELS: u64 = 64_000_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInspection {
    pub ok: bool,
    pub total: usize,
    pub offset: usize,
    pub has_more: bool,
    pub images: Vec<EmbeddedImageSnapshot>,
    pub diagnostics: Vec<crate::StyleInspectionDiagnostic>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedImageSnapshot {
    pub index: usize,
    pub part_name: String,
    pub handle: Option<String>,
    pub format: Option<&'static str>,
    pub intrinsic_width_px: Option<u32>,
    pub intrinsic_height_px: Option<u32>,
    pub display_width_emu: Option<i64>,
    pub display_height_emu: Option<i64>,
    pub alt_text: Option<String>,
    pub kind: &'static str,
    pub anchor: Option<ImageAnchorFacts>,
    pub relationship_id: Option<String>,
    pub media_part_name: Option<String>,
    pub size_bytes: Option<u64>,
    pub diagnostics: Vec<String>,
}
fn failure(code: &str, message: impl Into<String>) -> OperationResult {
    OperationResult::failed(code, message)
}
fn failed(error: impl fmt::Display) -> OperationResult {
    failure("IMAGE_READ_FAILED", error.to_string())
}
fn header_footer(part: &Part) -> bool {
    matches!(
        part.content_type.as_str(),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"
    )
}
fn relationships_bytes(package: &Package, owner: &Part) -> Result<Vec<u8>, OperationResult> {
    let (directory, name) = owner
        .name
        .as_str()
        .rsplit_once('/')
        .ok_or_else(|| failed("invalid owning part"))?;
    let name =
        opensuite_opc::PartName::parse(format!("{directory}/_rels/{name}.rels")).map_err(failed)?;
    package.read_part_by_name(&name).map_err(failed)
}
// Same fixed FNV-1a convention as section handles, scoped to source + relations + payload.
fn handle(
    owner: &Part,
    source: &SourceDocument,
    index: usize,
    relationships: &[u8],
    media: &Part,
    bytes: &[u8],
) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for value in [
        owner.name.as_str().as_bytes(),
        source.original_bytes(),
        relationships,
        media.name.as_str().as_bytes(),
        bytes,
    ] {
        for byte in value {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
    }
    format!("image:{}:{index}:{hash:016x}", owner.name.as_str())
}
fn payload(
    package: &Package,
    owner: &Part,
    picture: &Picture<'_>,
) -> Result<(Part, Vec<u8>, crate::ImageInfo), OperationResult> {
    if !picture.is_simple() {
        return Err(failure(
            "UNSUPPORTED_IMAGE_STRUCTURE",
            "image retrieval requires one direct picture and blip",
        ));
    }
    let blip = picture
        .blip()
        .map_err(failed)?
        .and_then(|id| picture.source.node(id))
        .ok_or_else(|| failed("missing image blip"))?;
    if blip.attribute("link").is_some() {
        return Err(failure("LINKED_IMAGE", "linked images cannot be retrieved"));
    }
    let relationships = package.part_relationships(owner).map_err(failed)?;
    let relationship_id = picture.relationship_id().map_err(failed)?;
    if relationships
        .iter()
        .filter(|rel| Some(rel.id.as_str()) == relationship_id)
        .count()
        > 1
    {
        return Err(failure(
            "AMBIGUOUS_IMAGE_RELATIONSHIP",
            "image relationship id is duplicated",
        ));
    }
    let ImageReference::Embedded(image) = picture
        .image_reference_with_relationships(package, &relationships)
        .map_err(|e| failure(e.code(), e.to_string()))?
    else {
        return Err(failure("LINKED_IMAGE", "image must be embedded"));
    };
    if !matches!(image.part.content_type.as_str(), "image/png" | "image/jpeg") {
        return Err(failure(
            "UNSUPPORTED_IMAGE_FORMAT",
            "only embedded PNG and JPEG are supported",
        ));
    }
    if image.size_bytes > MAX_EMBEDDED_IMAGE_BYTES {
        return Err(failure(
            "IMAGE_SIZE_LIMIT",
            "embedded image exceeds the 20 MiB limit",
        ));
    }
    let bytes = package.read_part(&image.part).map_err(failed)?;
    let info = crate::read_image_info(&bytes)
        .map_err(|e| failure("UNSUPPORTED_IMAGE_FORMAT", e.to_string()))?;
    let expected = match info.format {
        crate::ImageFormat::Png => "image/png",
        crate::ImageFormat::Jpeg => "image/jpeg",
    };
    if expected != image.part.content_type.as_str() {
        return Err(failure(
            "IMAGE_FORMAT_MISMATCH",
            "image bytes disagree with package content type",
        ));
    }
    if u64::from(info.dimensions.width_px) * u64::from(info.dimensions.height_px) > MAX_IMAGE_PIXELS
    {
        return Err(failure(
            "IMAGE_SIZE_LIMIT",
            "embedded image exceeds the 64 million pixel limit",
        ));
    }
    Ok((image.part, bytes, info))
}

/// Main document then header/footer parts in name order. Reads media only for the requested page (1–100).
pub fn inspect_docx_images(input: Vec<u8>, offset: usize, limit: usize) -> ImageInspection {
    let mut result = ImageInspection {
        ok: true,
        total: 0,
        offset,
        has_more: false,
        images: Vec::new(),
        diagnostics: Vec::new(),
    };
    let read = (|| -> Result<(), OperationResult> {
        let package = Package::from_bytes(input).map_err(failed)?;
        let main = package.main_office_document().map_err(failed)?;
        let mut parts = package
            .part_names_with_prefix("/")
            .filter_map(|name| package.part(name).ok())
            .filter(header_footer)
            .collect::<Vec<_>>();
        parts.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
        parts.insert(0, main);
        for owner in parts {
            let source = SourceDocument::parse(package.read_part(&owner).map_err(failed)?)
                .map_err(failed)?;
            for (index, picture) in pictures(&source).enumerate() {
                let global_index = result.total;
                result.total += 1;
                if global_index < offset || result.images.len() >= limit.clamp(1, 100) {
                    continue;
                }
                let extent = picture.extent().ok().flatten();
                let mut snapshot = EmbeddedImageSnapshot {
                    index: global_index,
                    part_name: owner.name.as_str().into(),
                    handle: None,
                    format: None,
                    intrinsic_width_px: None,
                    intrinsic_height_px: None,
                    display_width_emu: extent.map(|e| e.width_emu),
                    display_height_emu: extent.map(|e| e.height_emu),
                    alt_text: picture
                        .metadata()
                        .and_then(|m| m.description)
                        .map(|v| v.chars().take(2000).collect()),
                    kind: if picture.kind() == PictureKind::Inline {
                        "inline"
                    } else {
                        "floating"
                    },
                    anchor: picture.anchor_facts(),
                    relationship_id: picture.relationship_id().ok().flatten().map(str::to_owned),
                    media_part_name: None,
                    size_bytes: None,
                    diagnostics: Vec::new(),
                };
                match payload(&package, &owner, &picture).and_then(|(part, bytes, info)| {
                    Ok((part, bytes, info, relationships_bytes(&package, &owner)?))
                }) {
                    Ok((part, bytes, info, rels)) => {
                        snapshot.handle =
                            Some(handle(&owner, &source, index, &rels, &part, &bytes));
                        snapshot.format = Some(match info.format {
                            crate::ImageFormat::Png => "png",
                            crate::ImageFormat::Jpeg => "jpeg",
                        });
                        snapshot.intrinsic_width_px = Some(info.dimensions.width_px);
                        snapshot.intrinsic_height_px = Some(info.dimensions.height_px);
                        snapshot.media_part_name = Some(part.name.as_str().into());
                        snapshot.size_bytes = Some(bytes.len() as u64);
                    }
                    Err(error) => snapshot
                        .diagnostics
                        .extend(error.diagnostics.into_iter().map(|d| d.code)),
                }
                result.images.push(snapshot);
            }
        }
        Ok(())
    })();
    if let Err(error) = read {
        result.ok = false;
        result
            .diagnostics
            .extend(
                error
                    .diagnostics
                    .into_iter()
                    .map(|d| crate::StyleInspectionDiagnostic {
                        code: d.code,
                        message: d.message,
                    }),
            );
    }
    result.has_more = offset.saturating_add(result.images.len()) < result.total;
    result
}

/// Backend binary read. Source, owning relationships and selected media must match inspection.
pub fn read_docx_embedded_image(
    input: Vec<u8>,
    image_handle: &str,
) -> Result<Vec<u8>, OperationResult> {
    let stale = || {
        failure(
            "PRECONDITION_FAILED",
            "image handle is invalid or stale; inspect images again",
        )
    };
    let value = image_handle.strip_prefix("image:").ok_or_else(stale)?;
    let (value, _) = value.rsplit_once(':').ok_or_else(stale)?;
    let (name, index) = value.rsplit_once(':').ok_or_else(stale)?;
    let index = index.parse::<usize>().map_err(|_| stale())?;
    let package = Package::from_bytes(input).map_err(failed)?;
    let name = opensuite_opc::PartName::parse(name).map_err(|_| stale())?;
    let owner = package.part(&name).map_err(|_| stale())?;
    if owner.name != package.main_office_document().map_err(failed)?.name && !header_footer(&owner)
    {
        return Err(stale());
    }
    let source =
        SourceDocument::parse(package.read_part(&owner).map_err(failed)?).map_err(failed)?;
    let picture = pictures(&source).nth(index).ok_or_else(stale)?;
    let (part, bytes, _) = payload(&package, &owner, &picture)?;
    let rels = relationships_bytes(&package, &owner)?;
    if handle(&owner, &source, index, &rels, &part, &bytes) != image_handle {
        return Err(stale());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
