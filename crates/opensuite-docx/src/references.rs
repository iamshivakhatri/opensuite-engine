use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use opensuite_opc::{Package, PackageError, Part, RelationshipTarget};

use crate::{NodeId, Run, SourceDocument, SourceNodeKind};

const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
const HYPERLINK_REL: [&str; 2] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink",
    "http://purl.oclc.org/ooxml/officeDocument/relationships/hyperlink",
];

pub struct Hyperlink<'a> {
    pub(crate) source: &'a SourceDocument,
    pub(crate) source_id: NodeId,
}
impl<'a> Hyperlink<'a> {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }
    pub fn relationship_id(&self) -> Option<&str> {
        self.source
            .node(self.source_id)
            .and_then(|node| node.attribute("id"))
    }
    pub fn anchor(&self) -> Option<&str> {
        self.source
            .node(self.source_id)
            .and_then(|node| node.attribute("anchor"))
    }
    pub fn runs(&self) -> impl Iterator<Item = Run<'a>> + '_ {
        self.source
            .children(self.source_id)
            .filter(|id| word(self.source, *id, "r"))
            .map(|source_id| Run {
                source: self.source,
                source_id,
            })
    }
    pub fn text(&self) -> Result<String, crate::SemanticError> {
        self.runs()
            .map(|run| run.text())
            .try_fold(String::new(), |mut text, value| {
                text.push_str(&value?);
                Ok(text)
            })
    }
    pub fn target(
        &self,
        package: &Package,
        main: &Part,
    ) -> Result<Option<HyperlinkTarget>, ReferenceError> {
        if let Some(anchor) = self.anchor() {
            return Ok(Some(HyperlinkTarget::InternalAnchor(anchor.to_owned())));
        }
        let id = self
            .relationship_id()
            .ok_or(ReferenceError::MissingHyperlinkRelationshipId)?;
        let relationship = package
            .part_relationships(main)
            .map_err(ReferenceError::Package)?
            .into_iter()
            .find(|relationship| relationship.id.as_str() == id)
            .ok_or_else(|| ReferenceError::MissingHyperlinkRelationship(id.to_owned()))?;
        if !HYPERLINK_REL.contains(&relationship.relationship_type.as_str()) {
            return Err(ReferenceError::WrongHyperlinkRelationshipType);
        }
        Ok(Some(match relationship.target {
            RelationshipTarget::External { original } => HyperlinkTarget::External(original),
            RelationshipTarget::Internal { part_name, .. } => HyperlinkTarget::InternalPart(
                package.part(&part_name).map_err(ReferenceError::Package)?,
            ),
        }))
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HyperlinkTarget {
    External(String),
    InternalAnchor(String),
    InternalPart(Part),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BookmarkId(pub u32);
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bookmark {
    pub id: BookmarkId,
    pub name: String,
    pub start_node_id: NodeId,
    pub end_node_id: NodeId,
}

pub(crate) fn hyperlinks(source: &SourceDocument) -> impl Iterator<Item = Hyperlink<'_>> {
    source
        .node_ids()
        .filter(move |id| word(source, *id, "hyperlink"))
        .map(|source_id| Hyperlink { source, source_id })
}
pub(crate) fn bookmarks(source: &SourceDocument) -> Result<Vec<Bookmark>, ReferenceError> {
    let mut starts = HashMap::new();
    let mut complete = HashSet::new();
    let mut result = Vec::new();
    for id in source.node_ids() {
        if word(source, id, "bookmarkStart") {
            let node = source.node(id).ok_or(ReferenceError::MalformedBookmark)?;
            let bookmark_id = bookmark_id(node.attribute("id"))?;
            let name = node
                .attribute("name")
                .ok_or(ReferenceError::MissingBookmarkName)?
                .to_owned();
            if starts.insert(bookmark_id, (id, name)).is_some() {
                return Err(ReferenceError::DuplicateBookmarkStart(bookmark_id));
            }
        } else if word(source, id, "bookmarkEnd") {
            let node = source.node(id).ok_or(ReferenceError::MalformedBookmark)?;
            let bookmark_id = bookmark_id(node.attribute("id"))?;
            let Some((start_node_id, name)) = starts.remove(&bookmark_id) else {
                return Err(if complete.contains(&bookmark_id) {
                    ReferenceError::DuplicateBookmarkEnd(bookmark_id)
                } else {
                    ReferenceError::UnmatchedBookmarkEnd(bookmark_id)
                });
            };
            complete.insert(bookmark_id);
            result.push(Bookmark {
                id: bookmark_id,
                name,
                start_node_id,
                end_node_id: id,
            });
        }
    }
    if let Some((id, _)) = starts.into_iter().next() {
        return Err(ReferenceError::UnmatchedBookmarkStart(id));
    }
    Ok(result)
}

fn bookmark_id(value: Option<&str>) -> Result<BookmarkId, ReferenceError> {
    value
        .ok_or(ReferenceError::MalformedBookmark)?
        .parse()
        .map(BookmarkId)
        .map_err(|_| ReferenceError::MalformedBookmark)
}
fn word(source: &SourceDocument, id: NodeId, name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name: xml_name, .. }) if xml_name.local_name() == name && xml_name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}

#[derive(Debug)]
pub enum ReferenceError {
    Package(PackageError),
    MissingHyperlinkRelationshipId,
    MissingHyperlinkRelationship(String),
    WrongHyperlinkRelationshipType,
    MalformedBookmark,
    MissingBookmarkName,
    DuplicateBookmarkStart(BookmarkId),
    DuplicateBookmarkEnd(BookmarkId),
    UnmatchedBookmarkStart(BookmarkId),
    UnmatchedBookmarkEnd(BookmarkId),
}
impl ReferenceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Package(error) => error.code(),
            Self::MissingHyperlinkRelationshipId => "MISSING_HYPERLINK_RELATIONSHIP_ID",
            Self::MissingHyperlinkRelationship(_) => "MISSING_HYPERLINK_RELATIONSHIP",
            Self::WrongHyperlinkRelationshipType => "WRONG_HYPERLINK_RELATIONSHIP_TYPE",
            Self::MalformedBookmark => "MALFORMED_BOOKMARK",
            Self::MissingBookmarkName => "MISSING_BOOKMARK_NAME",
            Self::DuplicateBookmarkStart(_) => "DUPLICATE_BOOKMARK_START",
            Self::DuplicateBookmarkEnd(_) => "DUPLICATE_BOOKMARK_END",
            Self::UnmatchedBookmarkStart(_) => "UNMATCHED_BOOKMARK_START",
            Self::UnmatchedBookmarkEnd(_) => "UNMATCHED_BOOKMARK_END",
        }
    }
}
impl fmt::Display for ReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reference inspection failed: {}", self.code())
    }
}
impl std::error::Error for ReferenceError {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::DocxDocument;

    use super::*;

    #[test]
    fn resolves_hyperlinks_and_pairs_bookmarks_in_source_order() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/references.docx");
        let package = Package::open(fixture).unwrap();
        let main = package.main_office_document().unwrap();
        let source = SourceDocument::parse(package.read_part(&main).unwrap()).unwrap();
        let document = DocxDocument::new(&source).unwrap();
        let paragraph = document.paragraphs().next().unwrap();
        let hyperlinks: Vec<_> = paragraph.hyperlinks().collect();

        assert_eq!(paragraph.text().unwrap(), "Visit OpenSuite and Revenue");
        assert_eq!(paragraph.runs().count(), 4);
        assert_eq!(hyperlinks.len(), 2);
        assert!(source.node(hyperlinks[0].source_id()).is_some());
        assert_eq!(hyperlinks[0].text().unwrap(), "OpenSuite");
        assert_eq!(
            hyperlinks[0].target(&package, &main).unwrap(),
            Some(HyperlinkTarget::External(
                "https://example.com/opensuite".to_owned()
            ))
        );
        assert_eq!(
            hyperlinks[1].target(&package, &main).unwrap(),
            Some(HyperlinkTarget::InternalAnchor("RevenueSection".to_owned()))
        );
        assert_eq!(document.hyperlinks().count(), 2);

        let bookmarks = document.bookmarks().unwrap();
        assert_eq!(bookmarks.len(), 2);
        let revenue = document
            .bookmark_by_name("RevenueSection")
            .unwrap()
            .unwrap();
        assert_eq!(revenue.id, BookmarkId(7));
        assert!(
            source.node(revenue.start_node_id).unwrap().span().start
                < source.node(revenue.end_node_id).unwrap().span().start
        );
        assert_eq!(
            document.bookmark_by_name("TableCell").unwrap().unwrap().id,
            BookmarkId(8)
        );
    }

    #[test]
    fn reports_missing_hyperlinks_and_malformed_bookmark_pairing() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/references.docx");
        let package = Package::open(fixture).unwrap();
        let main = package.main_office_document().unwrap();
        let source = SourceDocument::parse(b"<x:document xmlns:x=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><x:body><x:p><x:hyperlink r:id=\"missing\"/></x:p><x:p><x:bookmarkStart x:id=\"1\" x:name=\"One\"/><x:bookmarkStart x:id=\"1\" x:name=\"Again\"/></x:p></x:body></x:document>".to_vec()).unwrap();
        let document = DocxDocument::new(&source).unwrap();
        let hyperlink = document.hyperlinks().next().unwrap();

        assert!(matches!(
            hyperlink.target(&package, &main),
            Err(ReferenceError::MissingHyperlinkRelationship(_))
        ));
        assert!(matches!(
            document.bookmarks(),
            Err(ReferenceError::DuplicateBookmarkStart(BookmarkId(1)))
        ));
    }
}
