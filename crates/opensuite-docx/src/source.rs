use std::{fmt, sync::Arc};

use quick_xml::{events::Event, name::ResolveResult, reader::NsReader};

/// A compact, stable identifier for a node in one source document.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(u32);

/// A half-open byte range into `SourceDocument::original_bytes`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

/// Namespace-resolved XML element name. The original prefix remains in the source bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XmlName {
    namespace_uri: Option<String>,
    local_name: String,
}

/// An attribute recorded from an element start tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAttribute {
    local_name: String,
    value: String,
}

impl SourceAttribute {
    pub fn local_name(&self) -> &str {
        &self.local_name
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl XmlName {
    pub fn namespace_uri(&self) -> Option<&str> {
        self.namespace_uri.as_deref()
    }

    pub fn local_name(&self) -> &str {
        &self.local_name
    }
}

/// The structural kind of a source node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceNodeKind {
    Element {
        name: XmlName,
        attributes: Vec<SourceAttribute>,
        start_tag: SourceSpan,
        end_tag: Option<SourceSpan>,
    },
    Text,
}

/// A compact source node. Links identify other entries in the document arena.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceNode {
    kind: SourceNodeKind,
    span: SourceSpan,
    parent: Option<NodeId>,
    first_child: Option<NodeId>,
    last_child: Option<NodeId>,
    next_sibling: Option<NodeId>,
}

impl SourceNode {
    pub fn kind(&self) -> &SourceNodeKind {
        &self.kind
    }

    pub fn span(&self) -> SourceSpan {
        self.span
    }

    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    pub fn attribute(&self, local_name: &str) -> Option<&str> {
        let SourceNodeKind::Element { attributes, .. } = &self.kind else {
            return None;
        };
        attributes
            .iter()
            .find(|attribute| attribute.local_name == local_name)
            .map(SourceAttribute::value)
    }
}

/// Immutable original XML plus a compact structural index over its bytes.
pub struct SourceDocument {
    original_bytes: Arc<[u8]>,
    nodes: Vec<SourceNode>,
    root: NodeId,
    element_count: usize,
    text_count: usize,
}

impl SourceDocument {
    pub fn parse(bytes: impl Into<Arc<[u8]>>) -> Result<Self, SourceError> {
        let original_bytes = bytes.into();
        let mut reader = NsReader::from_reader(original_bytes.as_ref());
        let mut nodes = Vec::new();
        let mut stack = Vec::new();
        let mut root = None;
        let mut element_count = 0;
        let mut text_count = 0;
        let mut buffer = Vec::new();

        loop {
            let (namespace, event) = reader
                .read_resolved_event_into(&mut buffer)
                .map_err(|_| SourceError::MalformedXml)?;
            let namespace_uri = namespace_uri(namespace)?;
            let end = reader.buffer_position() as usize;

            match event {
                Event::Start(element) => {
                    let start = tag_start(&original_bytes, end)?;
                    let id = add_node(
                        &mut nodes,
                        &mut stack,
                        SourceNodeKind::Element {
                            name: xml_name(namespace_uri, element.local_name().as_ref())?,
                            attributes: xml_attributes(&reader, &element)?,
                            start_tag: SourceSpan { start, end },
                            end_tag: None,
                        },
                        SourceSpan { start, end },
                    )?;
                    set_root(&mut root, &stack, id)?;
                    stack.push(id);
                    element_count += 1;
                }
                Event::Empty(element) => {
                    let start = tag_start(&original_bytes, end)?;
                    let id = add_node(
                        &mut nodes,
                        &mut stack,
                        SourceNodeKind::Element {
                            name: xml_name(namespace_uri, element.local_name().as_ref())?,
                            attributes: xml_attributes(&reader, &element)?,
                            start_tag: SourceSpan { start, end },
                            end_tag: None,
                        },
                        SourceSpan { start, end },
                    )?;
                    set_root(&mut root, &stack, id)?;
                    element_count += 1;
                }
                Event::End(_) => {
                    let id = stack.pop().ok_or(SourceError::MalformedXml)?;
                    let start = tag_start(&original_bytes, end)?;
                    let node = node_mut(&mut nodes, id)?;
                    let SourceNodeKind::Element { end_tag, .. } = &mut node.kind else {
                        return Err(SourceError::SourceSpanInconsistency);
                    };
                    *end_tag = Some(SourceSpan { start, end });
                    node.span.end = end;
                }
                Event::Text(text) if !stack.is_empty() && !text.is_empty() => {
                    let start = end
                        .checked_sub(text.len())
                        .ok_or(SourceError::SourceSpanInconsistency)?;
                    add_node(
                        &mut nodes,
                        &mut stack,
                        SourceNodeKind::Text,
                        SourceSpan { start, end },
                    )?;
                    text_count += 1;
                }
                Event::CData(text) if !stack.is_empty() && !text.is_empty() => {
                    let span = cdata_content_span(&original_bytes, end)?;
                    add_node(&mut nodes, &mut stack, SourceNodeKind::Text, span)?;
                    text_count += 1;
                }
                Event::Eof => break,
                _ => {}
            }
            buffer.clear();
        }

        if !stack.is_empty() {
            return Err(SourceError::MalformedXml);
        }
        let root = root.ok_or(SourceError::MissingRoot)?;
        Ok(Self {
            original_bytes,
            nodes,
            root,
            element_count,
            text_count,
        })
    }

    pub fn original_bytes(&self) -> &[u8] {
        &self.original_bytes
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn node(&self, id: NodeId) -> Option<&SourceNode> {
        self.nodes.get(id.0 as usize)
    }

    /// Returns the original, undecoded bytes for a text node.
    pub fn text_bytes(&self, id: NodeId) -> Option<&[u8]> {
        let node = self.node(id)?;
        matches!(node.kind, SourceNodeKind::Text)
            .then(|| self.original_bytes.get(node.span.start..node.span.end))?
    }

    pub fn children(&self, parent: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let first_child = self.node(parent).and_then(|node| node.first_child);
        std::iter::successors(first_child, move |id| {
            self.node(*id).and_then(|node| node.next_sibling)
        })
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        (0..self.nodes.len()).map(|index| NodeId(index as u32))
    }

    pub fn element_count(&self) -> usize {
        self.element_count
    }

    pub fn text_count(&self) -> usize {
        self.text_count
    }
}

/// Narrow failures from loading or indexing DOCX source XML.
#[derive(Debug)]
pub enum SourceError {
    Package(opensuite_opc::PackageError),
    MalformedXml,
    MissingRoot,
    InvalidNamespace,
    SourceSpanInconsistency,
}

impl SourceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Package(error) => error.code(),
            Self::MalformedXml => "MALFORMED_XML",
            Self::MissingRoot => "MISSING_XML_ROOT",
            Self::InvalidNamespace => "INVALID_NAMESPACE",
            Self::SourceSpanInconsistency => "SOURCE_SPAN_INCONSISTENCY",
        }
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Package(error) => error.fmt(formatter),
            Self::MalformedXml => write!(formatter, "malformed XML"),
            Self::MissingRoot => write!(formatter, "XML has no root element"),
            Self::InvalidNamespace => {
                write!(formatter, "XML contains an unresolved namespace prefix")
            }
            Self::SourceSpanInconsistency => write!(formatter, "XML source spans are inconsistent"),
        }
    }
}

impl std::error::Error for SourceError {}

fn namespace_uri(namespace: ResolveResult<'_>) -> Result<Option<String>, SourceError> {
    Ok(match namespace {
        ResolveResult::Unbound => None,
        ResolveResult::Bound(uri) => Some(
            String::from_utf8(uri.into_inner().to_vec())
                .map_err(|_| SourceError::InvalidNamespace)?,
        ),
        ResolveResult::Unknown(_) => return Err(SourceError::InvalidNamespace),
    })
}

fn xml_name(namespace_uri: Option<String>, local_name: &[u8]) -> Result<XmlName, SourceError> {
    let local_name =
        String::from_utf8(local_name.to_vec()).map_err(|_| SourceError::MalformedXml)?;
    Ok(XmlName {
        namespace_uri,
        local_name,
    })
}

fn xml_attributes(
    reader: &NsReader<&[u8]>,
    element: &quick_xml::events::BytesStart<'_>,
) -> Result<Vec<SourceAttribute>, SourceError> {
    element
        .attributes()
        .with_checks(false)
        .map(|attribute| {
            let attribute = attribute.map_err(|_| SourceError::MalformedXml)?;
            let local_name = attribute
                .key
                .as_ref()
                .rsplit(|byte| *byte == b':')
                .next()
                .unwrap_or(attribute.key.as_ref());
            Ok(SourceAttribute {
                local_name: String::from_utf8(local_name.to_vec())
                    .map_err(|_| SourceError::MalformedXml)?,
                value: attribute
                    .decode_and_unescape_value(reader.decoder())
                    .map_err(|_| SourceError::MalformedXml)?
                    .into_owned(),
            })
        })
        .collect()
}

fn tag_start(bytes: &[u8], end: usize) -> Result<usize, SourceError> {
    let mut quote = None;
    for index in (0..end).rev() {
        match (bytes[index], quote) {
            (b'\'' | b'"', None) => quote = Some(bytes[index]),
            (quote_byte, Some(open_quote)) if quote_byte == open_quote => quote = None,
            (b'<', None) => return Ok(index),
            _ => {}
        }
    }
    Err(SourceError::SourceSpanInconsistency)
}

fn cdata_start(bytes: &[u8], end: usize) -> Result<usize, SourceError> {
    bytes[..end]
        .windows(b"<![CDATA[".len())
        .rposition(|window| window == b"<![CDATA[")
        .ok_or(SourceError::SourceSpanInconsistency)
}

fn cdata_content_span(bytes: &[u8], end: usize) -> Result<SourceSpan, SourceError> {
    let start = cdata_start(bytes, end)? + b"<![CDATA[".len();
    let end = end
        .checked_sub(b"]]>".len())
        .ok_or(SourceError::SourceSpanInconsistency)?;
    if start > end {
        return Err(SourceError::SourceSpanInconsistency);
    }
    Ok(SourceSpan { start, end })
}

fn add_node(
    nodes: &mut Vec<SourceNode>,
    stack: &mut [NodeId],
    kind: SourceNodeKind,
    span: SourceSpan,
) -> Result<NodeId, SourceError> {
    let id = NodeId(u32::try_from(nodes.len()).map_err(|_| SourceError::SourceSpanInconsistency)?);
    let parent = stack.last().copied();
    nodes.push(SourceNode {
        kind,
        span,
        parent,
        first_child: None,
        last_child: None,
        next_sibling: None,
    });
    if let Some(parent) = parent {
        let last_child = node_mut(nodes, parent)?.last_child;
        if let Some(last_child) = last_child {
            node_mut(nodes, last_child)?.next_sibling = Some(id);
        } else {
            node_mut(nodes, parent)?.first_child = Some(id);
        }
        node_mut(nodes, parent)?.last_child = Some(id);
    }
    Ok(id)
}

fn node_mut(nodes: &mut [SourceNode], id: NodeId) -> Result<&mut SourceNode, SourceError> {
    nodes
        .get_mut(id.0 as usize)
        .ok_or(SourceError::SourceSpanInconsistency)
}

fn set_root(root: &mut Option<NodeId>, stack: &[NodeId], id: NodeId) -> Result<(), SourceError> {
    if stack.is_empty() && root.replace(id).is_some() {
        return Err(SourceError::MalformedXml);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn retains_original_bytes_and_indexes_namespaces_and_spans() {
        let xml =
            b"<w:document xmlns:w=\"urn:word\"><w:body><w:t>Hello</w:t></w:body></w:document>";
        let source = SourceDocument::parse(xml.to_vec()).unwrap();

        assert_eq!(source.original_bytes(), xml);
        assert_eq!(source.node_count(), 4);
        assert_eq!(source.element_count(), 3);
        assert_eq!(source.text_count(), 1);
        let root = source.node(source.root()).unwrap();
        let SourceNodeKind::Element { name, .. } = root.kind() else {
            panic!("root must be an element")
        };
        assert_eq!(name.local_name(), "document");
        assert_eq!(name.namespace_uri(), Some("urn:word"));

        let body = source.children(source.root()).next().unwrap();
        assert_eq!(source.node(body).unwrap().parent(), Some(source.root()));
        let text = source
            .children(body)
            .flat_map(|id| source.children(id))
            .next()
            .unwrap();
        let span = source.node(text).unwrap().span();
        assert_eq!(&source.original_bytes()[span.start..span.end], b"Hello");
    }

    #[test]
    fn reports_malformed_xml() {
        assert!(matches!(
            SourceDocument::parse(b"<document>".to_vec()),
            Err(SourceError::MalformedXml)
        ));
    }

    #[test]
    fn reads_the_main_part_lazily_from_the_opc_fixture() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/custom-main.docx");
        let package = opensuite_opc::Package::open(path).unwrap();
        let (part, source) = super::super::open_main_source(&package).unwrap();

        assert_eq!(part.name.as_str(), "/custom/main.xml");
        assert_eq!(
            source.original_bytes(),
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<document/>\n"
        );
    }
}
