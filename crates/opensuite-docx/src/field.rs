use std::{fmt, sync::Arc};

use crate::{NodeId, SemanticError, SourceDocument, SourceNodeKind};

const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldKind {
    Simple,
    Complex,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldState {
    Complete,
    Unterminated,
}

pub struct Field<'a> {
    source: &'a SourceDocument,
    source_id: NodeId,
    kind: FieldKind,
    state: FieldState,
    instruction_end_id: Option<NodeId>,
    separate_id: Option<NodeId>,
    end_id: Option<NodeId>,
    text_nodes: Arc<[NodeId]>,
    instruction_nodes: Arc<[NodeId]>,
}

impl Field<'_> {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }
    pub fn kind(&self) -> FieldKind {
        self.kind
    }
    pub fn state(&self) -> FieldState {
        self.state
    }
    pub fn separate_id(&self) -> Option<NodeId> {
        self.separate_id
    }
    pub fn end_id(&self) -> Option<NodeId> {
        self.end_id
    }
    pub fn has_separator(&self) -> bool {
        self.separate_id.is_some()
    }
    pub fn instruction(&self) -> Result<Option<String>, FieldError> {
        self.bounded_text(true, usize::MAX).map(|v| v.0)
    }
    pub fn result_text(&self) -> Result<Option<String>, FieldError> {
        self.bounded_text(false, usize::MAX).map(|v| v.0)
    }
}

pub struct FieldSet<'a> {
    fields: Vec<Field<'a>>,
    errors: Vec<FieldError>,
}
impl<'a> FieldSet<'a> {
    pub fn iter(&self) -> impl Iterator<Item = &Field<'a>> {
        self.fields.iter()
    }
    pub fn errors(&self) -> impl Iterator<Item = &FieldError> {
        self.errors.iter()
    }
    pub fn len(&self) -> usize {
        self.fields.len()
    }
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

pub(crate) fn fields(source: &SourceDocument) -> FieldSet<'_> {
    let text_nodes: Arc<[NodeId]> = source
        .node_ids()
        .filter(|id| word(source, *id, "t"))
        .collect::<Vec<_>>()
        .into();
    let instruction_nodes: Arc<[NodeId]> = source
        .node_ids()
        .filter(|id| word(source, *id, "instrText"))
        .collect::<Vec<_>>()
        .into();
    let mut fields = source
        .node_ids()
        .filter(|id| word(source, *id, "fldSimple"))
        .map(|source_id| Field {
            source,
            source_id,
            kind: FieldKind::Simple,
            state: FieldState::Complete,
            instruction_end_id: None,
            separate_id: None,
            end_id: None,
            text_nodes: text_nodes.clone(),
            instruction_nodes: instruction_nodes.clone(),
        })
        .collect::<Vec<_>>();
    let mut stack = Vec::new();
    let mut errors = Vec::new();
    for id in source.node_ids() {
        if word(source, id, "instrText") {
            if let Some((_, None, instruction_end)) = stack.last_mut() {
                *instruction_end = Some(id);
            }
            continue;
        }
        if !word(source, id, "fldChar") {
            continue;
        }
        let kind = source
            .node(id)
            .and_then(|node| node.attribute("fldCharType"));
        match kind {
            Some("begin") => stack.push((id, None, None)),
            Some("separate") => match stack.last_mut() {
                Some((_, separate, _)) if separate.is_none() => *separate = Some(id),
                _ => errors.push(FieldError::UnexpectedSeparate(id)),
            },
            Some("end") => match stack.pop() {
                Some((begin, separate, instruction_end_id)) => fields.push(Field {
                    source,
                    source_id: begin,
                    kind: FieldKind::Complex,
                    state: FieldState::Complete,
                    instruction_end_id,
                    separate_id: separate,
                    end_id: Some(id),
                    text_nodes: text_nodes.clone(),
                    instruction_nodes: instruction_nodes.clone(),
                }),
                None => errors.push(FieldError::UnmatchedEnd(id)),
            },
            _ => errors.push(FieldError::InvalidMarker(id)),
        }
    }
    for (begin, separate, instruction_end_id) in stack {
        errors.push(FieldError::UnterminatedBegin(begin));
        fields.push(Field {
            source,
            source_id: begin,
            kind: FieldKind::Complex,
            state: FieldState::Unterminated,
            instruction_end_id,
            separate_id: separate,
            end_id: None,
            text_nodes: text_nodes.clone(),
            instruction_nodes: instruction_nodes.clone(),
        });
    }
    fields.sort_by_key(|field| source.node(field.source_id).map(|node| node.span().start));
    FieldSet { fields, errors }
}

fn read_range(
    source: &SourceDocument,
    nodes: &[NodeId],
    start: usize,
    end: usize,
    limit: usize,
) -> Result<(String, bool), FieldError> {
    let first = nodes.partition_point(|id| source.node(*id).unwrap().span().start < start);
    let last = nodes.partition_point(|id| source.node(*id).unwrap().span().start < end);
    let mut text = String::new();
    let mut count = 0;
    for id in &nodes[first..last.max(first)] {
        let value = crate::semantic::text_value(source, *id).map_err(FieldError::Semantic)?;
        for c in value.chars() {
            if count == limit {
                return Ok((text, true));
            }
            text.push(c);
            count += 1;
        }
    }
    Ok((text, false))
}

impl Field<'_> {
    pub(crate) fn bounded_text(
        &self,
        instruction: bool,
        limit: usize,
    ) -> Result<(Option<String>, bool), FieldError> {
        let span = self.source.node(self.source_id).unwrap().span();
        if self.kind == FieldKind::Simple {
            if instruction {
                return Ok(
                    match self.source.node(self.source_id).unwrap().attribute("instr") {
                        Some(value) => (
                            Some(value.chars().take(limit).collect()),
                            value.chars().count() > limit,
                        ),
                        None => (None, false),
                    },
                );
            }
            return read_range(
                self.source,
                &self.text_nodes,
                span.start + 1,
                span.end,
                limit,
            )
            .map(|(s, t)| (Some(s), t));
        }
        let range = if instruction {
            self.separate_id
                .or(self.end_id)
                .or(self.instruction_end_id)
                .map(|end| (span.end, self.source.node(end).unwrap().span().end))
        } else {
            self.separate_id.zip(self.end_id).map(|(start, end)| {
                (
                    self.source.node(start).unwrap().span().end,
                    self.source.node(end).unwrap().span().start,
                )
            })
        };
        match range {
            Some((start, end)) => read_range(
                self.source,
                if instruction {
                    &self.instruction_nodes
                } else {
                    &self.text_nodes
                },
                start,
                end,
                limit,
            )
            .map(|(s, t)| (Some(s), t)),
            None => Ok((None, false)),
        }
    }
    pub(crate) fn span(&self) -> crate::SourceSpan {
        let mut span = self.source.node(self.source_id).unwrap().span();
        if let Some(end) = self.end_id {
            span.end = self.source.node(end).unwrap().span().end;
        } else if self.kind == FieldKind::Complex {
            span.end = self.source.original_bytes().len();
        }
        span
    }
}

fn word(source: &SourceDocument, id: NodeId, name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name: xml_name, .. }) if xml_name.local_name() == name && xml_name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}

#[derive(Debug)]
pub enum FieldError {
    Semantic(SemanticError),
    MalformedField,
    UnexpectedSeparate(NodeId),
    InvalidMarker(NodeId),
    UnmatchedEnd(NodeId),
    UnterminatedBegin(NodeId),
}
impl FieldError {
    pub(crate) fn source_id(&self) -> Option<NodeId> {
        match self {
            Self::InvalidMarker(id)
            | Self::UnexpectedSeparate(id)
            | Self::UnmatchedEnd(id)
            | Self::UnterminatedBegin(id) => Some(*id),
            _ => None,
        }
    }
    pub fn code(&self) -> &'static str {
        match self {
            Self::Semantic(error) => error.code(),
            Self::MalformedField => "MALFORMED_FIELD",
            Self::InvalidMarker(_) => "INVALID_FIELD_MARKER",
            Self::UnexpectedSeparate(_) => "UNEXPECTED_FIELD_SEPARATOR",
            Self::UnmatchedEnd(_) => "UNMATCHED_FIELD_END",
            Self::UnterminatedBegin(_) => "UNTERMINATED_FIELD_BEGIN",
        }
    }
}
impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "field inspection failed: {}", self.code())
    }
}
impl std::error::Error for FieldError {}

#[cfg(test)]
mod tests {
    use crate::DocxDocument;

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    #[test]
    fn discovers_simple_complex_nested_and_table_fields() {
        let source = SourceDocument::parse(format!(
            "<x:document xmlns:x=\"{WORD}\"><x:body><x:p><x:fldSimple x:instr=\"DATE\"><x:r><x:t>September 4</x:t></x:r></x:fldSimple></x:p><x:p><x:r><x:fldChar x:fldCharType=\"begin\"/></x:r><x:r><x:instrText xml:space=\"preserve\"> MERGEFIELD </x:instrText></x:r><x:r><x:instrText>CustomerName </x:instrText></x:r><x:r><x:fldChar x:fldCharType=\"separate\"/></x:r><x:r><x:t>Acme Corp</x:t></x:r><x:r><x:fldChar x:fldCharType=\"end\"/></x:r></x:p><x:p><x:r><x:fldChar x:fldCharType=\"begin\"/></x:r><x:r><x:instrText>OUTER</x:instrText></x:r><x:r><x:fldChar x:fldCharType=\"separate\"/></x:r><x:r><x:t>A</x:t></x:r><x:r><x:fldChar x:fldCharType=\"begin\"/></x:r><x:r><x:instrText>INNER</x:instrText></x:r><x:r><x:fldChar x:fldCharType=\"separate\"/></x:r><x:r><x:t>B</x:t></x:r><x:r><x:fldChar x:fldCharType=\"end\"/></x:r><x:r><x:t>C</x:t></x:r><x:r><x:fldChar x:fldCharType=\"end\"/></x:r></x:p><x:tbl><x:tr><x:tc><x:p><x:fldSimple x:instr=\"PAGE\"><x:r><x:t>1</x:t></x:r></x:fldSimple></x:p></x:tc></x:tr></x:tbl></x:body></x:document>"
        ).into_bytes()).unwrap();
        let fields = DocxDocument::new(&source).unwrap().fields();
        let fields: Vec<_> = fields.iter().collect();

        assert_eq!(fields.len(), 5);
        assert_eq!(fields[0].kind(), FieldKind::Simple);
        assert_eq!(fields[0].instruction().unwrap().as_deref(), Some("DATE"));
        assert_eq!(
            fields[0].result_text().unwrap().as_deref(),
            Some("September 4")
        );
        assert_eq!(
            fields[1].instruction().unwrap().as_deref(),
            Some(" MERGEFIELD CustomerName ")
        );
        assert_eq!(
            fields[1].result_text().unwrap().as_deref(),
            Some("Acme Corp")
        );
        assert!(fields[1].has_separator());
        assert_eq!(fields[2].instruction().unwrap().as_deref(), Some("OUTER"));
        assert_eq!(fields[2].result_text().unwrap().as_deref(), Some("ABC"));
        assert_eq!(fields[3].instruction().unwrap().as_deref(), Some("INNER"));
        assert_eq!(fields[3].result_text().unwrap().as_deref(), Some("B"));
        assert_eq!(fields[4].instruction().unwrap().as_deref(), Some("PAGE"));
        assert_eq!(fields[4].result_text().unwrap().as_deref(), Some("1"));
        assert!(
            fields
                .iter()
                .all(|field| source.node(field.source_id()).is_some())
        );
    }

    #[test]
    fn keeps_unterminated_fields_and_reports_unmatched_markers() {
        let source = SourceDocument::parse(format!(
            "<document xmlns=\"{WORD}\"><body><p><r><fldChar fldCharType=\"end\"/></r><r><fldChar fldCharType=\"begin\"/></r><r><instrText>DATE</instrText></r></p></body></document>"
        ).into_bytes()).unwrap();
        let fields = DocxDocument::new(&source).unwrap().fields();
        let field = fields.iter().next().unwrap();

        assert_eq!(field.state(), FieldState::Unterminated);
        assert_eq!(field.instruction().unwrap().as_deref(), Some("DATE"));
        assert_eq!(field.result_text().unwrap(), None);
        assert!(matches!(
            fields.errors().next(),
            Some(FieldError::UnmatchedEnd(_))
        ));
        assert!(
            fields
                .errors()
                .any(|error| matches!(error, FieldError::UnterminatedBegin(_)))
        );
    }

    #[test]
    fn leaves_complete_fields_without_a_separator_without_a_result() {
        let source = SourceDocument::parse(format!(
            "<document xmlns=\"{WORD}\"><body><p><r><fldChar fldCharType=\"begin\"/></r><r><instrText>DATE</instrText></r><r><fldChar fldCharType=\"end\"/></r></p></body></document>"
        ).into_bytes()).unwrap();
        let fields = DocxDocument::new(&source).unwrap().fields();
        let field = fields.iter().next().unwrap();

        assert_eq!(field.state(), FieldState::Complete);
        assert!(!field.has_separator());
        assert_eq!(field.instruction().unwrap().as_deref(), Some("DATE"));
        assert_eq!(field.result_text().unwrap(), None);
    }
}
