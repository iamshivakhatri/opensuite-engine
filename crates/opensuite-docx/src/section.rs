use std::fmt;

use crate::{HeaderFooterKind, HeaderFooterReference, NodeId, SourceDocument, SourceNodeKind};

const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];

pub struct Section<'a> {
    source: &'a SourceDocument,
    properties_id: NodeId,
    end_body_child_index: usize,
}

impl<'a> Section<'a> {
    pub fn source_id(&self) -> NodeId {
        self.properties_id
    }
    pub fn end_body_child_index(&self) -> usize {
        self.end_body_child_index
    }
    pub fn properties(&self) -> SectionProperties<'a> {
        SectionProperties {
            source: self.source,
            source_id: self.properties_id,
        }
    }
    pub fn header_references(&self) -> impl Iterator<Item = HeaderFooterReference> + '_ {
        crate::header_footer::references(self.source, self.properties_id, HeaderFooterKind::Header)
    }
    pub fn footer_references(&self) -> impl Iterator<Item = HeaderFooterReference> + '_ {
        crate::header_footer::references(self.source, self.properties_id, HeaderFooterKind::Footer)
    }
}

pub struct SectionProperties<'a> {
    pub(crate) source: &'a SourceDocument,
    pub(crate) source_id: NodeId,
}

impl SectionProperties<'_> {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }
    pub fn section_type(&self) -> Option<SectionType> {
        child(self.source, self.source_id, "type")
            .and_then(|id| self.source.node(id))
            .and_then(|node| node.attribute("val"))
            .map(section_type)
    }
    pub fn page_size(&self) -> Result<Option<PageSize>, SectionError> {
        let Some(id) = child(self.source, self.source_id, "pgSz") else {
            return Ok(None);
        };
        let node = self
            .source
            .node(id)
            .ok_or(SectionError::MalformedSectionProperties)?;
        Ok(Some(PageSize {
            width_twips: optional_u32(node.attribute("w"))?,
            height_twips: optional_u32(node.attribute("h"))?,
            orientation: node.attribute("orient").map(orientation),
        }))
    }
    pub fn page_margins(&self) -> Result<Option<PageMargins>, SectionError> {
        let Some(id) = child(self.source, self.source_id, "pgMar") else {
            return Ok(None);
        };
        let node = self
            .source
            .node(id)
            .ok_or(SectionError::MalformedSectionProperties)?;
        Ok(Some(PageMargins {
            top_twips: optional_i32(node.attribute("top"))?,
            bottom_twips: optional_i32(node.attribute("bottom"))?,
            left_twips: optional_i32(node.attribute("left"))?,
            right_twips: optional_i32(node.attribute("right"))?,
            header_twips: optional_i32(node.attribute("header"))?,
            footer_twips: optional_i32(node.attribute("footer"))?,
            gutter_twips: optional_i32(node.attribute("gutter"))?,
        }))
    }
    pub fn columns(&self) -> Result<Option<Columns>, SectionError> {
        let Some(id) = child(self.source, self.source_id, "cols") else {
            return Ok(None);
        };
        let node = self
            .source
            .node(id)
            .ok_or(SectionError::MalformedSectionProperties)?;
        Ok(Some(Columns {
            count: optional_u16(node.attribute("num"))?,
            spacing_twips: optional_i32(node.attribute("space"))?,
            equal_width: node.attribute("equalWidth").map(bool_value).transpose()?,
        }))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageSize {
    pub width_twips: Option<u32>,
    pub height_twips: Option<u32>,
    pub orientation: Option<PageOrientation>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PageOrientation {
    Portrait,
    Landscape,
    Unknown(String),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageMargins {
    pub top_twips: Option<i32>,
    pub bottom_twips: Option<i32>,
    pub left_twips: Option<i32>,
    pub right_twips: Option<i32>,
    pub header_twips: Option<i32>,
    pub footer_twips: Option<i32>,
    pub gutter_twips: Option<i32>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Columns {
    pub count: Option<u16>,
    pub spacing_twips: Option<i32>,
    pub equal_width: Option<bool>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SectionType {
    NextPage,
    Continuous,
    EvenPage,
    OddPage,
    Unknown(String),
}

#[derive(Debug)]
pub enum SectionError {
    MalformedSectionProperties,
    InvalidSectionProperty,
}
impl SectionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MalformedSectionProperties => "MALFORMED_SECTION_PROPERTIES",
            Self::InvalidSectionProperty => "INVALID_SECTION_PROPERTY",
        }
    }
}
impl fmt::Display for SectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "section inspection failed: {}", self.code())
    }
}
impl std::error::Error for SectionError {}

pub(crate) fn sections(
    source: &SourceDocument,
    body_id: NodeId,
) -> impl Iterator<Item = Section<'_>> {
    source
        .children(body_id)
        .enumerate()
        .filter_map(move |(index, id)| {
            let properties_id = if word(source, id, "p") {
                child(source, id, "pPr").and_then(|ppr| child(source, ppr, "sectPr"))
            } else if word(source, id, "sectPr") {
                Some(id)
            } else {
                None
            }?;
            Some(Section {
                source,
                properties_id,
                end_body_child_index: index + usize::from(!word(source, id, "sectPr")),
            })
        })
}

fn section_type(value: &str) -> SectionType {
    match value {
        "nextPage" => SectionType::NextPage,
        "continuous" => SectionType::Continuous,
        "evenPage" => SectionType::EvenPage,
        "oddPage" => SectionType::OddPage,
        value => SectionType::Unknown(value.to_owned()),
    }
}
fn orientation(value: &str) -> PageOrientation {
    match value {
        "portrait" => PageOrientation::Portrait,
        "landscape" => PageOrientation::Landscape,
        value => PageOrientation::Unknown(value.to_owned()),
    }
}
fn optional_u32(value: Option<&str>) -> Result<Option<u32>, SectionError> {
    value
        .map(|value| {
            value
                .parse()
                .map_err(|_| SectionError::InvalidSectionProperty)
        })
        .transpose()
}
fn optional_u16(value: Option<&str>) -> Result<Option<u16>, SectionError> {
    value
        .map(|value| {
            value
                .parse()
                .map_err(|_| SectionError::InvalidSectionProperty)
        })
        .transpose()
}
fn optional_i32(value: Option<&str>) -> Result<Option<i32>, SectionError> {
    value
        .map(|value| {
            value
                .parse()
                .map_err(|_| SectionError::InvalidSectionProperty)
        })
        .transpose()
}
fn bool_value(value: &str) -> Result<bool, SectionError> {
    match value {
        "true" | "1" | "on" => Ok(true),
        "false" | "0" | "off" => Ok(false),
        _ => Err(SectionError::InvalidSectionProperty),
    }
}
fn child(source: &SourceDocument, parent: NodeId, name: &str) -> Option<NodeId> {
    source.children(parent).find(|id| word(source, *id, name))
}
fn word(source: &SourceDocument, id: NodeId, name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name: xml_name, .. }) if xml_name.local_name() == name && xml_name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}

#[cfg(test)]
mod tests {
    use crate::DocxDocument;

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    #[test]
    fn discovers_intermediate_and_final_sections_in_body_order() {
        let source = SourceDocument::parse(format!(
            "<x:document xmlns:x=\"{WORD}\"><x:body><x:p><x:r><x:t>First</x:t></x:r><x:pPr><x:sectPr><x:type x:val=\"nextPage\"/><x:pgSz x:w=\"12240\" x:h=\"15840\" x:orient=\"portrait\"/><x:pgMar x:top=\"1440\" x:bottom=\"1440\" x:left=\"1440\" x:right=\"1440\" x:header=\"720\" x:footer=\"720\" x:gutter=\"0\"/><x:cols x:num=\"1\" x:space=\"720\" x:equalWidth=\"1\"/></x:sectPr></x:pPr></x:p><x:p><x:r><x:t>Second</x:t></x:r></x:p><x:sectPr><x:type x:val=\"continuous\"/><x:pgSz x:w=\"15840\" x:h=\"12240\" x:orient=\"landscape\"/><x:cols x:num=\"2\"/></x:sectPr></x:body></x:document>"
        ).into_bytes()).unwrap();
        let document = DocxDocument::new(&source).unwrap();
        let sections: Vec<_> = document.sections().collect();

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].end_body_child_index(), 1);
        assert_eq!(sections[1].end_body_child_index(), 2);
        assert!(source.node(sections[0].source_id()).is_some());
        assert!(source.node(sections[0].properties().source_id()).is_some());
        let first = sections[0].properties();
        assert_eq!(first.section_type(), Some(SectionType::NextPage));
        assert_eq!(
            first.page_size().unwrap(),
            Some(PageSize {
                width_twips: Some(12240),
                height_twips: Some(15840),
                orientation: Some(PageOrientation::Portrait)
            })
        );
        assert_eq!(
            first.page_margins().unwrap(),
            Some(PageMargins {
                top_twips: Some(1440),
                bottom_twips: Some(1440),
                left_twips: Some(1440),
                right_twips: Some(1440),
                header_twips: Some(720),
                footer_twips: Some(720),
                gutter_twips: Some(0)
            })
        );
        assert_eq!(
            first.columns().unwrap(),
            Some(Columns {
                count: Some(1),
                spacing_twips: Some(720),
                equal_width: Some(true)
            })
        );
        let last = sections[1].properties();
        assert_eq!(last.section_type(), Some(SectionType::Continuous));
        assert_eq!(
            last.page_size().unwrap().unwrap().orientation,
            Some(PageOrientation::Landscape)
        );
        assert_eq!(
            last.columns().unwrap(),
            Some(Columns {
                count: Some(2),
                spacing_twips: None,
                equal_width: None
            })
        );
    }

    #[test]
    fn keeps_unknown_values_and_absent_properties_nonfatal() {
        let source = SourceDocument::parse(format!(
            "<document xmlns=\"{WORD}\"><body><sectPr><type val=\"custom\"/><pgSz orient=\"sideways\"/><unknown/></sectPr></body></document>"
        ).into_bytes()).unwrap();
        let section = DocxDocument::new(&source)
            .unwrap()
            .sections()
            .next()
            .unwrap();
        let properties = section.properties();

        assert_eq!(
            properties.section_type(),
            Some(SectionType::Unknown("custom".to_owned()))
        );
        assert_eq!(
            properties.page_size().unwrap(),
            Some(PageSize {
                width_twips: None,
                height_twips: None,
                orientation: Some(PageOrientation::Unknown("sideways".to_owned()))
            })
        );
        assert_eq!(properties.page_margins().unwrap(), None);
        assert_eq!(properties.columns().unwrap(), None);
    }

    #[test]
    fn rejects_malformed_numeric_properties() {
        let source = SourceDocument::parse(format!(
            "<document xmlns=\"{WORD}\"><body><sectPr><pgSz w=\"wide\"/></sectPr></body></document>"
        ).into_bytes()).unwrap();
        let properties = DocxDocument::new(&source)
            .unwrap()
            .sections()
            .next()
            .unwrap()
            .properties();
        assert!(matches!(
            properties.page_size(),
            Err(SectionError::InvalidSectionProperty)
        ));
    }
}
