use std::fmt;

use opensuite_opc::{Package, PackageError, Part, RelationshipTarget};

use crate::{NodeId, SourceDocument, SourceNodeKind};

const WORD: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
const WP: [&str; 2] = [
    "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing",
    "http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing",
];
const DRAWING: [&str; 2] = [
    "http://schemas.openxmlformats.org/drawingml/2006/main",
    "http://purl.oclc.org/ooxml/drawingml/main",
];
const PICTURE: [&str; 2] = [
    "http://schemas.openxmlformats.org/drawingml/2006/picture",
    "http://purl.oclc.org/ooxml/drawingml/picture",
];
const IMAGE_REL: [&str; 2] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
    "http://purl.oclc.org/ooxml/officeDocument/relationships/image",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PictureKind {
    Inline,
    Anchored,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PictureExtent {
    pub width_emu: i64,
    pub height_emu: i64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PictureMetadata {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub title: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImagePart {
    pub part: Part,
    pub size_bytes: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageReference {
    Embedded(ImagePart),
    LinkedExternal(String),
    LinkedInternal(Part),
}

/// Structural facts preserve imported references and wrap names, including unsupported ones.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePositionFacts {
    pub reference: Option<String>,
    pub alignment: Option<String>,
    pub offset_emu: Option<i64>,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAnchorFacts {
    pub horizontal: Option<ImagePositionFacts>,
    pub vertical: Option<ImagePositionFacts>,
    pub wrap: Option<String>,
    pub behind_text: Option<bool>,
    pub distance_emu: std::collections::BTreeMap<String, i64>,
    pub editable: bool,
}

pub struct Picture<'a> {
    source: &'a SourceDocument,
    source_id: NodeId,
    container_id: NodeId,
    kind: PictureKind,
}
impl Picture<'_> {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }
    pub fn kind(&self) -> PictureKind {
        self.kind
    }
    pub fn extent(&self) -> Result<Option<PictureExtent>, PictureError> {
        let Some(id) = child(self.source, self.container_id, WP, "extent") else {
            return Ok(None);
        };
        let node = self.source.node(id).ok_or(PictureError::MalformedPicture)?;
        Ok(Some(PictureExtent {
            width_emu: number(node.attribute("cx"))?,
            height_emu: number(node.attribute("cy"))?,
        }))
    }
    pub(crate) fn container_id(&self) -> NodeId {
        self.container_id
    }
    pub(crate) fn layout_child(&self, local: &str) -> Option<NodeId> {
        child(self.source, self.container_id, WP, local)
    }
    /// Only one direct picture in one drawing container is safe to target.
    pub(crate) fn is_simple(&self) -> bool {
        let containers = self
            .source
            .children(self.source_id)
            .filter(|id| {
                element(self.source, *id, WP, "inline") || element(self.source, *id, WP, "anchor")
            })
            .count();
        let mut todo = vec![self.source_id];
        let (mut pictures, mut blips, mut graphics) = (0, 0, 0);
        while let Some(id) = todo.pop() {
            todo.extend(self.source.children(id));
            pictures += usize::from(element(self.source, id, PICTURE, "pic"));
            blips += usize::from(element(self.source, id, DRAWING, "blip"));
            graphics += usize::from(element(self.source, id, DRAWING, "graphic"));
        }
        let direct_picture = child(self.source, self.container_id, DRAWING, "graphic")
            .and_then(|id| child(self.source, id, DRAWING, "graphicData"))
            .is_some_and(|id| {
                self.source
                    .children(id)
                    .filter(|id| {
                        matches!(
                            self.source.node(*id).map(|n| n.kind()),
                            Some(SourceNodeKind::Element { .. })
                        )
                    })
                    .count()
                    == 1
            });
        containers == 1
            && pictures == 1
            && blips == 1
            && graphics == 1
            && direct_picture
            && self.blip().ok().flatten().is_some()
    }
    pub(crate) fn extent_nodes(&self) -> Option<[NodeId; 2]> {
        let only = |parent, namespaces, local| {
            let mut nodes = self
                .source
                .children(parent)
                .filter(|id| element(self.source, *id, namespaces, local));
            let id = nodes.next()?;
            nodes.next().is_none().then_some(id)
        };
        let extent = only(self.container_id, WP, "extent")?;
        let graphic = only(self.container_id, DRAWING, "graphic")?;
        let data = only(graphic, DRAWING, "graphicData")?;
        let picture = only(data, PICTURE, "pic")?;
        let properties = only(picture, PICTURE, "spPr")?;
        let transform = only(properties, DRAWING, "xfrm")?;
        Some([extent, only(transform, DRAWING, "ext")?])
    }
    pub fn anchor_facts(&self) -> Option<ImageAnchorFacts> {
        if self.kind != PictureKind::Anchored {
            return None;
        }
        let node = self.source.node(self.container_id)?;
        let position = |local| {
            self.layout_child(local).map(|id| {
                let alignment = child(self.source, id, WP, "align")
                    .and_then(|v| element_text(self.source, v))
                    .map(str::to_owned);
                let offset = child(self.source, id, WP, "posOffset")
                    .and_then(|v| element_text(self.source, v))
                    .and_then(|v| v.parse().ok());
                ImagePositionFacts {
                    reference: self
                        .source
                        .node(id)
                        .and_then(|n| n.attribute("relativeFrom"))
                        .map(str::to_owned),
                    alignment,
                    offset_emu: offset,
                }
            })
        };
        let wrap = [
            "wrapNone",
            "wrapSquare",
            "wrapTopAndBottom",
            "wrapTight",
            "wrapThrough",
        ]
        .into_iter()
        .find(|v| self.layout_child(v).is_some());
        let behind_text = match node.attribute("behindDoc") {
            Some("1" | "true") => Some(true),
            Some("0" | "false") => Some(false),
            _ => None,
        };
        let horizontal = position("positionH");
        let vertical = position("positionV");
        let supported_axis = |facts: &Option<ImagePositionFacts>, horizontal: bool| {
            facts.as_ref().is_some_and(|f| {
                let reference = f.reference.as_deref();
                (matches!(reference, Some("page" | "margin"))
                    || reference == Some(if horizontal { "column" } else { "paragraph" }))
                    && match (&f.alignment, f.offset_emu) {
                        (None, Some(v)) => i32::try_from(v).is_ok(),
                        (Some(v), None) => {
                            reference != Some("paragraph")
                                && if horizontal {
                                    matches!(v.as_str(), "left" | "center" | "right")
                                } else {
                                    matches!(v.as_str(), "top" | "center" | "bottom")
                                }
                        }
                        _ => false,
                    }
            })
        };
        let position_count = |local| {
            self.source
                .children(self.container_id)
                .filter(|id| element(self.source, *id, WP, local))
                .count()
        };
        let wrap_count = [
            "wrapNone",
            "wrapSquare",
            "wrapTopAndBottom",
            "wrapTight",
            "wrapThrough",
        ]
        .into_iter()
        .map(position_count)
        .sum::<usize>();
        let editable = self.is_simple()
            && position_count("positionH") == 1
            && position_count("positionV") == 1
            && wrap_count == 1
            && node
                .attribute("simplePos")
                .is_some_and(|v| matches!(v, "0" | "false"))
            && matches!(wrap, Some("wrapNone" | "wrapSquare" | "wrapTopAndBottom"))
            && behind_text.is_some()
            && supported_axis(&horizontal, true)
            && supported_axis(&vertical, false);
        let wrap_node = wrap
            .and_then(|local| self.layout_child(local))
            .and_then(|id| self.source.node(id));
        Some(ImageAnchorFacts {
            horizontal,
            vertical,
            wrap: wrap.map(|v| {
                match v {
                    "wrapNone" if behind_text == Some(true) => "behindText",
                    "wrapNone" if behind_text == Some(false) => "inFrontOfText",
                    "wrapSquare" => "square",
                    "wrapTopAndBottom" => "topAndBottom",
                    "wrapTight" => "tight",
                    "wrapThrough" => "through",
                    _ => v,
                }
                .to_owned()
            }),
            behind_text,
            distance_emu: [
                ("top", "distT"),
                ("bottom", "distB"),
                ("left", "distL"),
                ("right", "distR"),
            ]
            .into_iter()
            .filter_map(|(key, attr)| {
                wrap_node
                    .and_then(|n| n.attribute(attr))
                    .or_else(|| node.attribute(attr))?
                    .parse()
                    .ok()
                    .map(|v| (key.to_owned(), v))
            })
            .collect(),
            editable,
        })
    }
    pub fn metadata(&self) -> Option<PictureMetadata> {
        let node = child(self.source, self.container_id, WP, "docPr")
            .and_then(|id| self.source.node(id))?;
        Some(PictureMetadata {
            id: node.attribute("id").map(str::to_owned),
            name: node.attribute("name").map(str::to_owned),
            description: node.attribute("descr").map(str::to_owned),
            title: node.attribute("title").map(str::to_owned),
        })
    }
    pub fn relationship_id(&self) -> Result<Option<&str>, PictureError> {
        Ok(self
            .blip()?
            .and_then(|id| self.source.node(id))
            .and_then(|node| node.attribute("embed").or_else(|| node.attribute("link"))))
    }
    pub fn image_reference(
        &self,
        package: &Package,
        owner: &Part,
    ) -> Result<ImageReference, PictureError> {
        self.image_reference_with_relationships(
            package,
            &package
                .part_relationships(owner)
                .map_err(PictureError::Package)?,
        )
    }

    pub(crate) fn image_reference_with_relationships(
        &self,
        package: &Package,
        relationships: &[opensuite_opc::Relationship],
    ) -> Result<ImageReference, PictureError> {
        let node = self
            .source
            .node(self.blip()?.ok_or(PictureError::MalformedPicture)?)
            .ok_or(PictureError::MalformedPicture)?;
        let (id, embedded) = match (node.attribute("embed"), node.attribute("link")) {
            (Some(id), _) => (id, true),
            (None, Some(id)) => (id, false),
            (None, None) => return Err(PictureError::MissingImageRelationshipId),
        };
        let relationship = relationships
            .iter()
            .find(|relationship| relationship.id.as_str() == id)
            .ok_or_else(|| PictureError::MissingImageRelationship(id.to_owned()))?;
        if !IMAGE_REL.contains(&relationship.relationship_type.as_str()) {
            return Err(PictureError::WrongImageRelationshipType);
        }
        match &relationship.target {
            RelationshipTarget::External { original: _ } if embedded => {
                Err(PictureError::ExternalEmbeddedImage)
            }
            RelationshipTarget::External { original } => {
                Ok(ImageReference::LinkedExternal(original.clone()))
            }
            RelationshipTarget::Internal { part_name, .. } => {
                let part = package.part(part_name).map_err(PictureError::Package)?;
                if embedded {
                    Ok(ImageReference::Embedded(ImagePart {
                        size_bytes: package.part_size(&part).map_err(PictureError::Package)?,
                        part,
                    }))
                } else {
                    Ok(ImageReference::LinkedInternal(part))
                }
            }
        }
    }
    fn blip(&self) -> Result<Option<NodeId>, PictureError> {
        let Some(graphic) = child(self.source, self.container_id, DRAWING, "graphic") else {
            return Ok(None);
        };
        let Some(data) = child(self.source, graphic, DRAWING, "graphicData") else {
            return Ok(None);
        };
        let Some(picture) = child(self.source, data, PICTURE, "pic") else {
            return Ok(None);
        };
        let Some(fill) = child(self.source, picture, PICTURE, "blipFill") else {
            return Ok(None);
        };
        Ok(child(self.source, fill, DRAWING, "blip"))
    }
}

pub(crate) fn pictures(source: &SourceDocument) -> impl Iterator<Item = Picture<'_>> {
    source.node_ids().filter_map(move |source_id| {
        if !element(source, source_id, WORD, "drawing") {
            return None;
        }
        let (container_id, kind) = if let Some(id) = child(source, source_id, WP, "inline") {
            (id, PictureKind::Inline)
        } else {
            (
                child(source, source_id, WP, "anchor")?,
                PictureKind::Anchored,
            )
        };
        Some(Picture {
            source,
            source_id,
            container_id,
            kind,
        })
    })
}
fn element_text(source: &SourceDocument, id: NodeId) -> Option<&str> {
    let mut children = source.children(id);
    let child = children.next()?;
    if children.next().is_some() {
        return None;
    }
    std::str::from_utf8(source.text_bytes(child)?).ok()
}
fn number(value: Option<&str>) -> Result<i64, PictureError> {
    value
        .ok_or(PictureError::InvalidDimension)?
        .parse()
        .map_err(|_| PictureError::InvalidDimension)
}
fn child(
    source: &SourceDocument,
    parent: NodeId,
    namespaces: [&str; 2],
    name: &str,
) -> Option<NodeId> {
    source
        .children(parent)
        .find(|id| element(source, *id, namespaces, name))
}
fn element(source: &SourceDocument, id: NodeId, namespaces: [&str; 2], name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name: xml_name, .. }) if xml_name.local_name() == name && xml_name.namespace_uri().is_some_and(|uri| namespaces.contains(&uri)))
}
#[derive(Debug)]
pub enum PictureError {
    Package(PackageError),
    MalformedPicture,
    MissingImageRelationshipId,
    MissingImageRelationship(String),
    WrongImageRelationshipType,
    ExternalEmbeddedImage,
    InvalidDimension,
}
impl PictureError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Package(error) => error.code(),
            Self::MalformedPicture => "MALFORMED_PICTURE",
            Self::MissingImageRelationshipId => "MISSING_IMAGE_RELATIONSHIP_ID",
            Self::MissingImageRelationship(_) => "MISSING_IMAGE_RELATIONSHIP",
            Self::WrongImageRelationshipType => "WRONG_IMAGE_RELATIONSHIP_TYPE",
            Self::ExternalEmbeddedImage => "EXTERNAL_EMBEDDED_IMAGE",
            Self::InvalidDimension => "INVALID_PICTURE_DIMENSION",
        }
    }
}
impl fmt::Display for PictureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "picture inspection failed: {}", self.code())
    }
}
impl std::error::Error for PictureError {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{DocxDocument, HeaderFooterType, load_header};

    use super::*;

    #[test]
    fn discovers_pictures_and_resolves_their_owning_part_relationships() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/images.docx");
        let package = Package::open(fixture).unwrap();
        let main = package.main_office_document().unwrap();
        let source = SourceDocument::parse(package.read_part(&main).unwrap()).unwrap();
        let document = DocxDocument::new(&source).unwrap();
        let pictures: Vec<_> = document.pictures().collect();

        assert_eq!(pictures.len(), 3);
        assert_eq!(pictures[0].kind(), PictureKind::Inline);
        assert!(source.node(pictures[0].source_id()).is_some());
        assert_eq!(
            pictures[0].extent().unwrap(),
            Some(PictureExtent {
                width_emu: 1_828_800,
                height_emu: 914_400
            })
        );
        assert_eq!(
            pictures[0].metadata(),
            Some(PictureMetadata {
                id: Some("1".to_owned()),
                name: Some("Logo".to_owned()),
                description: Some("Company logo".to_owned()),
                title: Some("Brand".to_owned())
            })
        );
        let ImageReference::Embedded(image) = pictures[0].image_reference(&package, &main).unwrap()
        else {
            panic!("first picture must be embedded")
        };
        assert_eq!(image.part.name.as_str(), "/media/logo.png");
        assert_eq!(image.part.content_type.as_str(), "image/png");
        assert!(image.size_bytes > 0);

        assert_eq!(pictures[1].kind(), PictureKind::Anchored);
        assert_eq!(
            pictures[1].extent().unwrap(),
            Some(PictureExtent {
                width_emu: 914_400,
                height_emu: 457_200
            })
        );
        assert_eq!(
            pictures[2].image_reference(&package, &main).unwrap(),
            ImageReference::LinkedExternal("https://example.invalid/image.png".to_owned())
        );

        let section = document.sections().next().unwrap();
        let header_reference = section.header_references().next().unwrap();
        assert_eq!(
            header_reference.reference_type(),
            &HeaderFooterType::Default
        );
        let header = load_header(&package, &main, &header_reference).unwrap();
        let header_picture = header.pictures().next().unwrap();
        let ImageReference::Embedded(image) = header_picture
            .image_reference(&package, header.part())
            .unwrap()
        else {
            panic!("header picture must be embedded")
        };
        assert_eq!(image.part.name.as_str(), "/media/header.gif");
    }

    #[test]
    fn reports_missing_and_missing_target_image_relationships() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/images.docx");
        let package = Package::open(fixture).unwrap();
        let main = package.main_office_document().unwrap();
        let parse_picture = |relationship| {
            SourceDocument::parse(format!("<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><w:body><w:p><w:r><w:drawing><wp:inline><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed=\"{relationship}\"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p></w:body></w:document>").into_bytes()).unwrap()
        };
        let source = parse_picture("missing");
        let picture = DocxDocument::new(&source)
            .unwrap()
            .pictures()
            .next()
            .unwrap();
        assert!(matches!(
            picture.image_reference(&package, &main),
            Err(PictureError::MissingImageRelationship(_))
        ));
        let source = parse_picture("missingTarget");
        let picture = DocxDocument::new(&source)
            .unwrap()
            .pictures()
            .next()
            .unwrap();
        assert!(matches!(
            picture.image_reference(&package, &main),
            Err(PictureError::Package(PackageError::MissingTargetPart(_)))
        ));
    }
}
