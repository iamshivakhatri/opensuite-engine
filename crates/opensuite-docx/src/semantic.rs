use std::fmt;

use quick_xml::escape::unescape;

use crate::{NodeId, SourceDocument, SourceNodeKind};

const WORDPROCESSINGML_NAMESPACES: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];

/// Read-only DOCX semantic entry point backed by a source document.
pub struct DocxDocument<'a> {
    source: &'a SourceDocument,
    source_id: NodeId,
    body_id: NodeId,
}

impl<'a> DocxDocument<'a> {
    pub fn new(source: &'a SourceDocument) -> Result<Self, SemanticError> {
        let source_id = source.root();
        if !is_word_element(source, source_id, "document") {
            return Err(SemanticError::InvalidDocumentRoot);
        }
        let body_id = source
            .children(source_id)
            .find(|id| is_word_element(source, *id, "body"))
            .ok_or(SemanticError::MissingDocumentBody)?;
        Ok(Self {
            source,
            source_id,
            body_id,
        })
    }

    pub fn source_id(&self) -> NodeId {
        self.source_id
    }

    pub fn body_id(&self) -> NodeId {
        self.body_id
    }

    pub fn blocks(&self) -> impl Iterator<Item = BodyBlock<'a>> + '_ {
        self.source.children(self.body_id).filter_map(|source_id| {
            if is_word_element(self.source, source_id, "p") {
                Some(BodyBlock::Paragraph(Paragraph {
                    source: self.source,
                    source_id,
                }))
            } else if is_word_element(self.source, source_id, "tbl") {
                Some(BodyBlock::Table(Table {
                    source: self.source,
                    source_id,
                }))
            } else {
                None
            }
        })
    }

    pub fn paragraphs(&self) -> impl Iterator<Item = Paragraph<'a>> + '_ {
        self.blocks().filter_map(|block| match block {
            BodyBlock::Paragraph(paragraph) => Some(paragraph),
            BodyBlock::Table(_) => None,
        })
    }
}

/// A direct body child, kept in source order.
pub enum BodyBlock<'a> {
    Paragraph(Paragraph<'a>),
    Table(Table<'a>),
}

/// A read-only paragraph view over a source node.
pub struct Paragraph<'a> {
    source: &'a SourceDocument,
    source_id: NodeId,
}

impl<'a> Paragraph<'a> {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }

    pub fn runs(&self) -> impl Iterator<Item = Run<'a>> + '_ {
        self.source
            .children(self.source_id)
            .filter(|id| is_word_element(self.source, *id, "r"))
            .map(|source_id| Run {
                source: self.source,
                source_id,
            })
    }

    pub fn text(&self) -> Result<String, SemanticError> {
        collect_text(self.runs().map(|run| run.text()))
    }
}

/// A read-only run view over a source node.
pub struct Run<'a> {
    source: &'a SourceDocument,
    source_id: NodeId,
}

impl<'a> Run<'a> {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }

    pub fn texts(&self) -> impl Iterator<Item = Text<'a>> + '_ {
        self.source
            .children(self.source_id)
            .filter(|id| is_word_element(self.source, *id, "t"))
            .map(|source_id| Text {
                source: self.source,
                source_id,
            })
    }

    pub fn text(&self) -> Result<String, SemanticError> {
        collect_text(self.texts().map(|text| text.value()))
    }
}

/// A read-only table view over a source node.
pub struct Table<'a> {
    source: &'a SourceDocument,
    source_id: NodeId,
}

impl<'a> Table<'a> {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }

    pub fn rows(&self) -> impl Iterator<Item = Row<'a>> + '_ {
        self.source
            .children(self.source_id)
            .filter(|id| is_word_element(self.source, *id, "tr"))
            .map(|source_id| Row {
                source: self.source,
                source_id,
            })
    }
}

/// A read-only table row view over a source node.
pub struct Row<'a> {
    source: &'a SourceDocument,
    source_id: NodeId,
}

impl<'a> Row<'a> {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }

    pub fn cells(&self) -> impl Iterator<Item = Cell<'a>> + '_ {
        self.source
            .children(self.source_id)
            .filter(|id| is_word_element(self.source, *id, "tc"))
            .map(|source_id| Cell {
                source: self.source,
                source_id,
            })
    }
}

/// A read-only table cell view over a source node.
pub struct Cell<'a> {
    source: &'a SourceDocument,
    source_id: NodeId,
}

impl<'a> Cell<'a> {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }

    pub fn paragraphs(&self) -> impl Iterator<Item = Paragraph<'a>> + '_ {
        self.source
            .children(self.source_id)
            .filter(|id| is_word_element(self.source, *id, "p"))
            .map(|source_id| Paragraph {
                source: self.source,
                source_id,
            })
    }

    pub fn text(&self) -> Result<String, SemanticError> {
        collect_text(self.paragraphs().map(|paragraph| paragraph.text()))
    }
}

/// A read-only WordprocessingML text view over a source node.
pub struct Text<'a> {
    source: &'a SourceDocument,
    source_id: NodeId,
}

impl<'a> Text<'a> {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }

    pub fn value(&self) -> Result<String, SemanticError> {
        let mut value = String::new();
        for id in self.source.children(self.source_id) {
            let bytes = self
                .source
                .text_bytes(id)
                .ok_or(SemanticError::InvalidTextNode)?;
            let raw = std::str::from_utf8(bytes).map_err(|_| SemanticError::InvalidTextNode)?;
            value.push_str(&unescape(raw).map_err(|_| SemanticError::InvalidTextNode)?);
        }
        Ok(value)
    }
}

/// Typed failures while recognizing the minimal DOCX semantic layer.
#[derive(Debug)]
pub enum SemanticError {
    InvalidDocumentRoot,
    MissingDocumentBody,
    InvalidTextNode,
}

impl SemanticError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidDocumentRoot => "INVALID_DOCX_DOCUMENT_ROOT",
            Self::MissingDocumentBody => "MISSING_DOCX_BODY",
            Self::InvalidTextNode => "INVALID_DOCX_TEXT",
        }
    }
}

impl fmt::Display for SemanticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDocumentRoot => {
                write!(formatter, "root is not a WordprocessingML document")
            }
            Self::MissingDocumentBody => write!(formatter, "document has no WordprocessingML body"),
            Self::InvalidTextNode => write!(formatter, "WordprocessingML text node is invalid"),
        }
    }
}

impl std::error::Error for SemanticError {}

fn collect_text(
    mut values: impl Iterator<Item = Result<String, SemanticError>>,
) -> Result<String, SemanticError> {
    values.try_fold(String::new(), |mut combined, value| {
        combined.push_str(&value?);
        Ok(combined)
    })
}

fn is_word_element(source: &SourceDocument, id: NodeId, local_name: &str) -> bool {
    let Some(node) = source.node(id) else {
        return false;
    };
    let SourceNodeKind::Element { name, .. } = node.kind() else {
        return false;
    };
    name.local_name() == local_name
        && name
            .namespace_uri()
            .is_some_and(|namespace| WORDPROCESSINGML_NAMESPACES.contains(&namespace))
}

#[cfg(test)]
mod tests {
    use crate::SourceDocument;

    use super::*;

    const TRANSITIONAL: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    const STRICT: &str = "http://purl.oclc.org/ooxml/wordprocessingml/main";

    #[test]
    fn exposes_paragraphs_runs_and_decoded_text_in_source_order() {
        let source = SourceDocument::parse(format!(
            "<x:document xmlns:x=\"{TRANSITIONAL}\"><x:body><x:p><x:r><x:t>Hello &amp; </x:t></x:r><x:r><x:t xml:space=\"preserve\"> OpenSuite </x:t></x:r></x:p><x:p><x:r><x:t>Next</x:t></x:r></x:p></x:body></x:document>"
        ).into_bytes()).unwrap();
        let document = DocxDocument::new(&source).unwrap();
        let paragraphs: Vec<_> = document.paragraphs().collect();

        assert_eq!(paragraphs.len(), 2);
        assert_eq!(paragraphs[0].text().unwrap(), "Hello &  OpenSuite ");
        assert_eq!(paragraphs[1].text().unwrap(), "Next");
        let runs: Vec<_> = paragraphs[0].runs().collect();
        assert_eq!(runs.len(), 2);
        assert_ne!(paragraphs[0].source_id(), paragraphs[1].source_id());
        assert_ne!(runs[0].source_id(), runs[1].source_id());
        assert!(
            source.node(paragraphs[0].source_id()).unwrap().span().start
                < source.node(paragraphs[1].source_id()).unwrap().span().start
        );
        assert!(
            source.node(runs[0].source_id()).unwrap().span().start
                < source.node(runs[1].source_id()).unwrap().span().start
        );
        assert_eq!(
            runs[0].texts().next().unwrap().source_id(),
            source.children(runs[0].source_id()).next().unwrap()
        );
    }

    #[test]
    fn supports_the_strict_wordprocessingml_namespace() {
        let source = SourceDocument::parse(
            format!(
                "<document xmlns=\"{STRICT}\"><body><p><r><t>Strict</t></r></p></body></document>"
            )
            .into_bytes(),
        )
        .unwrap();

        assert_eq!(
            DocxDocument::new(&source)
                .unwrap()
                .paragraphs()
                .next()
                .unwrap()
                .text()
                .unwrap(),
            "Strict"
        );
    }

    #[test]
    fn rejects_invalid_or_missing_document_body() {
        let wrong_root =
            SourceDocument::parse(format!("<root xmlns=\"{TRANSITIONAL}\"/>").into_bytes())
                .unwrap();
        let missing_body =
            SourceDocument::parse(format!("<document xmlns=\"{TRANSITIONAL}\"/>").into_bytes())
                .unwrap();

        assert!(matches!(
            DocxDocument::new(&wrong_root),
            Err(SemanticError::InvalidDocumentRoot)
        ));
        assert!(matches!(
            DocxDocument::new(&missing_body),
            Err(SemanticError::MissingDocumentBody)
        ));
    }

    #[test]
    fn preserves_body_block_and_table_source_order() {
        let source = SourceDocument::parse(format!(
            "<d:document xmlns:d=\"{TRANSITIONAL}\"><d:body><d:p><d:r><d:t>Before</d:t></d:r></d:p><d:tbl><d:tr><d:tc><d:p><d:r><d:t>A1</d:t></d:r></d:p><d:p><d:r><d:t>A2</d:t></d:r></d:p></d:tc><d:tc><d:p><d:r><d:t>B1</d:t></d:r></d:p></d:tc></d:tr><d:tr><d:tc><d:p><d:r><d:t>C1</d:t></d:r></d:p></d:tc></d:tr></d:tbl><d:p><d:r><d:t>After</d:t></d:r></d:p></d:body></d:document>"
        ).into_bytes()).unwrap();
        let document = DocxDocument::new(&source).unwrap();
        let blocks: Vec<_> = document.blocks().collect();

        assert_eq!(blocks.len(), 3);
        let BodyBlock::Paragraph(before) = &blocks[0] else {
            panic!("first block must be a paragraph")
        };
        let BodyBlock::Table(table) = &blocks[1] else {
            panic!("second block must be a table")
        };
        let BodyBlock::Paragraph(after) = &blocks[2] else {
            panic!("third block must be a paragraph")
        };
        assert_eq!(before.text().unwrap(), "Before");
        assert_eq!(after.text().unwrap(), "After");
        assert!(
            source.node(before.source_id()).unwrap().span().start
                < source.node(table.source_id()).unwrap().span().start
        );
        assert!(
            source.node(table.source_id()).unwrap().span().start
                < source.node(after.source_id()).unwrap().span().start
        );

        let rows: Vec<_> = table.rows().collect();
        assert_eq!(rows.len(), 2);
        assert!(
            source.node(rows[0].source_id()).unwrap().span().start
                < source.node(rows[1].source_id()).unwrap().span().start
        );
        let first_row_cells: Vec<_> = rows[0].cells().collect();
        let second_row_cells: Vec<_> = rows[1].cells().collect();
        assert_eq!(first_row_cells.len(), 2);
        assert_eq!(second_row_cells.len(), 1);
        assert!(
            source
                .node(first_row_cells[0].source_id())
                .unwrap()
                .span()
                .start
                < source
                    .node(first_row_cells[1].source_id())
                    .unwrap()
                    .span()
                    .start
        );
        assert_eq!(first_row_cells[0].text().unwrap(), "A1A2");
        assert_eq!(first_row_cells[1].text().unwrap(), "B1");
        assert_eq!(second_row_cells[0].text().unwrap(), "C1");
        assert_eq!(first_row_cells[0].paragraphs().count(), 2);
    }
}
