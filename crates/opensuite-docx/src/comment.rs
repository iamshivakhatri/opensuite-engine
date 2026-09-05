use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use opensuite_opc::{Package, PackageError, Part, RelationshipTarget};

use crate::{NodeId, SemanticError, SourceDocument, SourceError, SourceNodeKind};

const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
const REL: [&str; 2] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments",
    "http://purl.oclc.org/ooxml/officeDocument/relationships/comments",
];

/// Read-only comments loaded from the main document's comments relationship.
pub struct CommentSet {
    source: Option<SourceDocument>,
    comments: Vec<CommentData>,
    errors: Vec<CommentIssue>,
}

struct CommentData {
    source_id: NodeId,
    id: Option<String>,
    author: Option<String>,
    initials: Option<String>,
    date: Option<String>,
    range_start: Option<NodeId>,
    range_end: Option<NodeId>,
    complete_range: bool,
    reference: Option<NodeId>,
}

/// A source-backed standard Word comment.
pub struct Comment<'a> {
    source: &'a SourceDocument,
    data: &'a CommentData,
}

impl CommentSet {
    pub fn comments(&self) -> impl Iterator<Item = Comment<'_>> {
        self.source.iter().flat_map(|source| {
            self.comments
                .iter()
                .map(move |data| Comment { source, data })
        })
    }

    pub fn errors(&self) -> impl Iterator<Item = &CommentIssue> {
        self.errors.iter()
    }
}

impl Comment<'_> {
    pub fn id(&self) -> Option<&str> {
        self.data.id.as_deref()
    }

    pub fn metadata(&self) -> CommentMetadata {
        CommentMetadata {
            author: self.data.author.clone(),
            initials: self.data.initials.clone(),
            date: self.data.date.clone(),
        }
    }

    pub fn text(&self) -> Result<String, SemanticError> {
        let mut paragraphs = Vec::new();
        paragraph_texts(self.source, self.data.source_id, &mut paragraphs)?;
        Ok(paragraphs.join("\n"))
    }

    /// True only when both document range markers were found.
    pub fn has_range(&self) -> bool {
        self.data.complete_range
    }

    pub fn has_reference(&self) -> bool {
        self.data.reference.is_some()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommentMetadata {
    pub author: Option<String>,
    pub initials: Option<String>,
    pub date: Option<String>,
}

/// A recoverable malformed comment marker situation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommentIssue {
    MissingCommentId,
    DuplicateCommentId(String),
    MissingCommentEntry(String),
    DuplicateRangeStart(String),
    UnmatchedRangeStart(String),
    DuplicateRangeEnd(String),
    UnmatchedRangeEnd(String),
    DuplicateReference(String),
}

impl CommentIssue {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingCommentId => "MISSING_COMMENT_ID",
            Self::DuplicateCommentId(_) => "DUPLICATE_COMMENT_ID",
            Self::MissingCommentEntry(_) => "MISSING_COMMENT_ENTRY",
            Self::DuplicateRangeStart(_) => "DUPLICATE_COMMENT_RANGE_START",
            Self::UnmatchedRangeStart(_) => "UNMATCHED_COMMENT_RANGE_START",
            Self::DuplicateRangeEnd(_) => "DUPLICATE_COMMENT_RANGE_END",
            Self::UnmatchedRangeEnd(_) => "UNMATCHED_COMMENT_RANGE_END",
            Self::DuplicateReference(_) => "DUPLICATE_COMMENT_REFERENCE",
        }
    }
}

pub(crate) fn load(
    package: &Package,
    main: &Part,
    document_source: &SourceDocument,
) -> Result<CommentSet, CommentError> {
    let relationships = match package.part_relationships(main) {
        Ok(relationships) => relationships,
        Err(PackageError::MissingPartRelationships(_)) => return Ok(empty()),
        Err(error) => return Err(CommentError::Package(error)),
    };
    let Some(relationship) = relationships
        .into_iter()
        .find(|relationship| REL.contains(&relationship.relationship_type.as_str()))
    else {
        return Ok(empty());
    };
    let RelationshipTarget::Internal { part_name, .. } = relationship.target else {
        return Err(CommentError::ExternalCommentsPart);
    };
    let part = package.part(&part_name).map_err(CommentError::Package)?;
    let source = SourceDocument::parse(package.read_part(&part).map_err(CommentError::Package)?)
        .map_err(CommentError::Source)?;
    if !word(&source, source.root(), "comments") {
        return Err(CommentError::InvalidCommentsRoot);
    }

    let mut errors = Vec::new();
    let mut comments = Vec::new();
    let mut by_id = HashMap::new();
    for source_id in source
        .children(source.root())
        .filter(|id| word(&source, *id, "comment"))
    {
        let node = source
            .node(source_id)
            .ok_or(CommentError::MalformedComments)?;
        let id = node.attribute("id").map(str::to_owned);
        let index = comments.len();
        if let Some(id) = id.as_ref() {
            if by_id.contains_key(id) {
                errors.push(CommentIssue::DuplicateCommentId(id.clone()));
            } else {
                by_id.insert(id.clone(), index);
            }
        } else {
            errors.push(CommentIssue::MissingCommentId);
        }
        comments.push(CommentData {
            source_id,
            id,
            author: node.attribute("author").map(str::to_owned),
            initials: node.attribute("initials").map(str::to_owned),
            date: node.attribute("date").map(str::to_owned),
            range_start: None,
            range_end: None,
            complete_range: false,
            reference: None,
        });
    }

    let mut starts = HashMap::new();
    let mut finished = HashSet::new();
    let mut missing_entries = HashSet::new();
    for source_id in document_source.node_ids() {
        let Some(kind) = marker_kind(document_source, source_id) else {
            continue;
        };
        let Some(id) = document_source
            .node(source_id)
            .and_then(|node| node.attribute("id"))
        else {
            errors.push(CommentIssue::MissingCommentId);
            continue;
        };
        let Some(&index) = by_id.get(id) else {
            if missing_entries.insert(id.to_owned()) {
                errors.push(CommentIssue::MissingCommentEntry(id.to_owned()));
            }
            continue;
        };
        match kind {
            MarkerKind::Start => {
                if starts.contains_key(id) {
                    errors.push(CommentIssue::DuplicateRangeStart(id.to_owned()));
                } else {
                    starts.insert(id.to_owned(), source_id);
                }
                comments[index].range_start.get_or_insert(source_id);
            }
            MarkerKind::End => {
                if comments[index].range_end.is_some() || finished.contains(id) {
                    errors.push(CommentIssue::DuplicateRangeEnd(id.to_owned()));
                } else if starts.remove(id).is_none() {
                    errors.push(CommentIssue::UnmatchedRangeEnd(id.to_owned()));
                    comments[index].range_end = Some(source_id);
                } else {
                    comments[index].range_end = Some(source_id);
                    comments[index].complete_range = true;
                    finished.insert(id.to_owned());
                }
            }
            MarkerKind::Reference => {
                if comments[index].reference.replace(source_id).is_some() {
                    errors.push(CommentIssue::DuplicateReference(id.to_owned()));
                }
            }
        }
    }
    for (id, _) in starts {
        errors.push(CommentIssue::UnmatchedRangeStart(id));
    }

    comments.sort_by_key(|comment| {
        comment
            .range_start
            .or(comment.range_end)
            .and_then(|id| document_source.node(id).map(|node| node.span().start))
            .unwrap_or(usize::MAX)
    });
    Ok(CommentSet {
        source: Some(source),
        comments,
        errors,
    })
}

fn empty() -> CommentSet {
    CommentSet {
        source: None,
        comments: Vec::new(),
        errors: Vec::new(),
    }
}

fn paragraph_texts(
    source: &SourceDocument,
    id: NodeId,
    paragraphs: &mut Vec<String>,
) -> Result<(), SemanticError> {
    if word(source, id, "p") {
        let mut text = String::new();
        collect_text(source, id, &mut text)?;
        paragraphs.push(text);
        return Ok(());
    }
    for child in source.children(id) {
        paragraph_texts(source, child, paragraphs)?;
    }
    Ok(())
}

fn collect_text(
    source: &SourceDocument,
    id: NodeId,
    text: &mut String,
) -> Result<(), SemanticError> {
    if word(source, id, "t") || word(source, id, "delText") {
        text.push_str(&crate::semantic::text_value(source, id)?);
        return Ok(());
    }
    for child in source.children(id) {
        collect_text(source, child, text)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum MarkerKind {
    Start,
    End,
    Reference,
}

fn marker_kind(source: &SourceDocument, id: NodeId) -> Option<MarkerKind> {
    if word(source, id, "commentRangeStart") {
        Some(MarkerKind::Start)
    } else if word(source, id, "commentRangeEnd") {
        Some(MarkerKind::End)
    } else if word(source, id, "commentReference") {
        Some(MarkerKind::Reference)
    } else {
        None
    }
}

fn word(source: &SourceDocument, id: NodeId, name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name: xml_name, .. }) if xml_name.local_name() == name && xml_name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}

#[derive(Debug)]
pub enum CommentError {
    Package(PackageError),
    Source(SourceError),
    InvalidCommentsRoot,
    MalformedComments,
    ExternalCommentsPart,
}

impl CommentError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Package(error) => error.code(),
            Self::Source(error) => error.code(),
            Self::InvalidCommentsRoot => "INVALID_COMMENTS_ROOT",
            Self::MalformedComments => "MALFORMED_COMMENTS",
            Self::ExternalCommentsPart => "EXTERNAL_COMMENTS_PART",
        }
    }
}

impl fmt::Display for CommentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "comment inspection failed: {}", self.code())
    }
}

impl std::error::Error for CommentError {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{DocxDocument, SourceDocument};

    use super::*;

    #[test]
    fn loads_comments_in_anchor_order_with_metadata_and_body_text() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/comments.docx");
        let package = Package::open(fixture).unwrap();
        let main = package.main_office_document().unwrap();
        let source = SourceDocument::parse(package.read_part(&main).unwrap()).unwrap();
        let comments = DocxDocument::new(&source)
            .unwrap()
            .comments(&package, &main)
            .unwrap();
        let values = comments.comments().collect::<Vec<_>>();

        assert_eq!(values.len(), 5);
        assert_eq!(values[0].id(), Some("7"));
        assert_eq!(values[0].metadata().author.as_deref(), Some("Alice"));
        assert_eq!(values[0].metadata().initials.as_deref(), Some("AL"));
        assert_eq!(
            values[0].text().unwrap(),
            "Please verify & keep\nSecond line "
        );
        assert!(values[0].has_range());
        assert!(values[0].has_reference());
        assert_eq!(values[1].id(), Some("8"));
        assert!(values[1].has_range());
        assert!(values[1].has_reference());
        assert_eq!(values[2].id(), Some("10"));
        assert!(!values[2].has_range());
        assert_eq!(values[3].id(), Some("11"));
        assert!(!values[3].has_range());
        assert_eq!(values[4].id(), Some("9"));
        assert!(!values[4].has_range());
        assert_eq!(values[4].metadata(), CommentMetadata::default());
        let codes = comments
            .errors()
            .map(CommentIssue::code)
            .collect::<Vec<_>>();
        assert_eq!(
            codes,
            [
                "UNMATCHED_COMMENT_RANGE_END",
                "MISSING_COMMENT_ENTRY",
                "UNMATCHED_COMMENT_RANGE_START",
            ]
        );
    }

    #[test]
    fn accepts_a_document_without_a_comments_relationship() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/semantic.docx");
        let package = Package::open(fixture).unwrap();
        let main = package.main_office_document().unwrap();
        let source = SourceDocument::parse(package.read_part(&main).unwrap()).unwrap();
        let comments = DocxDocument::new(&source)
            .unwrap()
            .comments(&package, &main)
            .unwrap();

        assert_eq!(comments.comments().count(), 0);
        assert_eq!(comments.errors().count(), 0);
    }
}
