use std::fmt;

use opensuite_opc::{Package, PackageError, Part, RelationshipTarget};

use crate::{BodyBlock, NodeId, SourceDocument, SourceError, SourceNodeKind};

const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
const HEADER_REL: [&str; 2] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header",
    "http://purl.oclc.org/ooxml/officeDocument/relationships/header",
];
const FOOTER_REL: [&str; 2] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer",
    "http://purl.oclc.org/ooxml/officeDocument/relationships/footer",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeaderFooterType {
    Default,
    First,
    Even,
    Unknown(String),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeaderFooterKind {
    Header,
    Footer,
}

#[derive(Clone, Debug)]
pub struct HeaderFooterReference {
    source_id: NodeId,
    kind: HeaderFooterKind,
    reference_type: HeaderFooterType,
    relationship_id: Option<String>,
}

impl HeaderFooterReference {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }
    pub fn kind(&self) -> HeaderFooterKind {
        self.kind
    }
    pub fn reference_type(&self) -> &HeaderFooterType {
        &self.reference_type
    }
    pub fn relationship_id(&self) -> Option<&str> {
        self.relationship_id.as_deref()
    }
    pub fn resolve_part(&self, package: &Package, main: &Part) -> Result<Part, HeaderFooterError> {
        let id = self
            .relationship_id
            .as_deref()
            .ok_or(HeaderFooterError::MissingRelationshipId)?;
        let relationship = package
            .part_relationships(main)
            .map_err(HeaderFooterError::Package)?
            .into_iter()
            .find(|relationship| relationship.id.as_str() == id)
            .ok_or_else(|| HeaderFooterError::MissingRelationship(id.to_owned()))?;
        let expected = match self.kind {
            HeaderFooterKind::Header => &HEADER_REL,
            HeaderFooterKind::Footer => &FOOTER_REL,
        };
        if !expected.contains(&relationship.relationship_type.as_str()) {
            return Err(HeaderFooterError::WrongRelationshipType);
        }
        let RelationshipTarget::Internal { part_name, .. } = relationship.target else {
            return Err(HeaderFooterError::ExternalTarget);
        };
        package.part(&part_name).map_err(HeaderFooterError::Package)
    }
}

pub struct HeaderFooter {
    part: Part,
    source: SourceDocument,
    root_id: NodeId,
    kind: HeaderFooterKind,
}

impl HeaderFooter {
    pub fn part(&self) -> &Part {
        &self.part
    }
    pub fn source(&self) -> &SourceDocument {
        &self.source
    }
    pub fn root_id(&self) -> NodeId {
        self.root_id
    }
    pub fn kind(&self) -> HeaderFooterKind {
        self.kind
    }
    pub fn blocks(&self) -> impl Iterator<Item = BodyBlock<'_>> {
        crate::semantic::container_blocks(&self.source, self.root_id)
    }
}

pub fn load_header(
    package: &Package,
    main: &Part,
    reference: &HeaderFooterReference,
) -> Result<HeaderFooter, HeaderFooterError> {
    load(package, main, reference, HeaderFooterKind::Header)
}
pub fn load_footer(
    package: &Package,
    main: &Part,
    reference: &HeaderFooterReference,
) -> Result<HeaderFooter, HeaderFooterError> {
    load(package, main, reference, HeaderFooterKind::Footer)
}

fn load(
    package: &Package,
    main: &Part,
    reference: &HeaderFooterReference,
    expected: HeaderFooterKind,
) -> Result<HeaderFooter, HeaderFooterError> {
    if reference.kind != expected {
        return Err(HeaderFooterError::WrongKind);
    }
    let part = reference.resolve_part(package, main)?;
    let source = SourceDocument::parse(
        package
            .read_part(&part)
            .map_err(HeaderFooterError::Package)?,
    )
    .map_err(HeaderFooterError::Source)?;
    let root_id = source.root();
    if !word(
        &source,
        root_id,
        match expected {
            HeaderFooterKind::Header => "hdr",
            HeaderFooterKind::Footer => "ftr",
        },
    ) {
        return Err(HeaderFooterError::InvalidRoot(expected));
    }
    Ok(HeaderFooter {
        part,
        source,
        root_id,
        kind: expected,
    })
}

#[derive(Debug)]
pub enum HeaderFooterError {
    Package(PackageError),
    Source(SourceError),
    MissingRelationshipId,
    MissingRelationship(String),
    WrongRelationshipType,
    ExternalTarget,
    WrongKind,
    InvalidRoot(HeaderFooterKind),
}
impl HeaderFooterError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Package(error) => error.code(),
            Self::Source(error) => error.code(),
            Self::MissingRelationshipId => "MISSING_HEADER_FOOTER_RELATIONSHIP_ID",
            Self::MissingRelationship(_) => "MISSING_HEADER_FOOTER_RELATIONSHIP",
            Self::WrongRelationshipType => "WRONG_HEADER_FOOTER_RELATIONSHIP_TYPE",
            Self::ExternalTarget => "EXTERNAL_HEADER_FOOTER_TARGET",
            Self::WrongKind => "WRONG_HEADER_FOOTER_KIND",
            Self::InvalidRoot(HeaderFooterKind::Header) => "INVALID_HEADER_ROOT",
            Self::InvalidRoot(HeaderFooterKind::Footer) => "INVALID_FOOTER_ROOT",
        }
    }
}
impl fmt::Display for HeaderFooterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "header/footer inspection failed: {}", self.code())
    }
}
impl std::error::Error for HeaderFooterError {}

pub(crate) fn references(
    source: &SourceDocument,
    section_id: NodeId,
    kind: HeaderFooterKind,
) -> impl Iterator<Item = HeaderFooterReference> + '_ {
    source.children(section_id).filter_map(move |id| {
        let name = match kind {
            HeaderFooterKind::Header => "headerReference",
            HeaderFooterKind::Footer => "footerReference",
        };
        if !word(source, id, name) {
            return None;
        }
        let node = source.node(id)?;
        Some(HeaderFooterReference {
            source_id: id,
            kind,
            reference_type: node
                .attribute("type")
                .map(reference_type)
                .unwrap_or_else(|| HeaderFooterType::Unknown(String::new())),
            relationship_id: node.attribute("id").map(str::to_owned),
        })
    })
}

fn reference_type(value: &str) -> HeaderFooterType {
    match value {
        "default" => HeaderFooterType::Default,
        "first" => HeaderFooterType::First,
        "even" => HeaderFooterType::Even,
        value => HeaderFooterType::Unknown(value.to_owned()),
    }
}
fn word(source: &SourceDocument, id: NodeId, name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name: xml_name, .. }) if xml_name.local_name() == name && xml_name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{BodyBlock, DocxDocument};

    use super::*;

    #[test]
    fn resolves_declared_references_and_reuses_container_blocks() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/headers-footers.docx");
        let package = Package::open(fixture).unwrap();
        let main = package.main_office_document().unwrap();
        let source = SourceDocument::parse(package.read_part(&main).unwrap()).unwrap();
        let section = DocxDocument::new(&source)
            .unwrap()
            .sections()
            .next()
            .unwrap();
        let headers: Vec<_> = section.header_references().collect();
        let footers: Vec<_> = section.footer_references().collect();

        assert_eq!(main.name.as_str(), "/custom/main.xml");
        assert_eq!(headers.len(), 3);
        assert_eq!(footers.len(), 3);
        assert_eq!(headers[0].reference_type(), &HeaderFooterType::Default);
        assert_eq!(headers[1].reference_type(), &HeaderFooterType::First);
        assert_eq!(headers[2].reference_type(), &HeaderFooterType::Even);
        assert_eq!(footers[0].reference_type(), &HeaderFooterType::Default);
        assert_eq!(footers[1].reference_type(), &HeaderFooterType::First);
        assert_eq!(footers[2].reference_type(), &HeaderFooterType::Even);
        assert!(source.node(headers[0].source_id()).is_some());

        let header = load_header(&package, &main, &headers[0]).unwrap();
        assert_eq!(header.part().name.as_str(), "/content/heads/default.xml");
        assert!(header.source().node(header.root_id()).is_some());
        let blocks: Vec<_> = header.blocks().collect();
        let BodyBlock::Paragraph(paragraph) = &blocks[0] else {
            panic!("first header block must be a paragraph")
        };
        assert_eq!(paragraph.text().unwrap(), "Company Annual Report");
        let BodyBlock::Table(table) = &blocks[1] else {
            panic!("second header block must be a table")
        };
        assert_eq!(
            table
                .rows()
                .next()
                .unwrap()
                .cells()
                .next()
                .unwrap()
                .text()
                .unwrap(),
            "Header cell"
        );

        let footer = load_footer(&package, &main, &footers[0]).unwrap();
        let BodyBlock::Paragraph(paragraph) = footer.blocks().next().unwrap() else {
            panic!("footer must contain a paragraph")
        };
        assert_eq!(paragraph.text().unwrap(), "Confidential");
    }

    #[test]
    fn reports_missing_relationships_without_loading_a_part() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/headers-footers.docx");
        let package = Package::open(fixture).unwrap();
        let main = package.main_office_document().unwrap();
        let source = SourceDocument::parse(package.read_part(&main).unwrap()).unwrap();
        let section = DocxDocument::new(&source)
            .unwrap()
            .sections()
            .next()
            .unwrap();
        let mut reference = section.header_references().next().unwrap();
        reference.relationship_id = Some("missing".to_owned());

        assert!(matches!(
            load_header(&package, &main, &reference),
            Err(HeaderFooterError::MissingRelationship(_))
        ));
    }

    #[test]
    fn rejects_external_and_invalid_header_parts() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/headers-footers-errors.docx");
        let package = Package::open(fixture).unwrap();
        let main = package.main_office_document().unwrap();
        let source = SourceDocument::parse(package.read_part(&main).unwrap()).unwrap();
        let reference = |relationship_id: &str| HeaderFooterReference {
            source_id: source.root(),
            kind: HeaderFooterKind::Header,
            reference_type: HeaderFooterType::Default,
            relationship_id: Some(relationship_id.to_owned()),
        };

        assert!(matches!(
            load_header(&package, &main, &reference("external")),
            Err(HeaderFooterError::ExternalTarget)
        ));
        assert!(matches!(
            load_header(&package, &main, &reference("invalid")),
            Err(HeaderFooterError::InvalidRoot(HeaderFooterKind::Header))
        ));
        assert!(matches!(
            load_header(&package, &main, &reference("malformed")),
            Err(HeaderFooterError::Source(_))
        ));
    }
}
