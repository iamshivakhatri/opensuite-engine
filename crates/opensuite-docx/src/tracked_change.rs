use crate::{NodeId, SemanticError, SourceDocument, SourceNodeKind};

const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];

/// A source-backed content revision in a WordprocessingML part.
pub struct TrackedChange<'a> {
    source: &'a SourceDocument,
    source_id: NodeId,
    kind: TrackedChangeKind,
}

impl TrackedChange<'_> {
    pub fn kind(&self) -> TrackedChangeKind {
        self.kind
    }

    pub fn metadata(&self) -> TrackedChangeMetadata {
        let node = self.source.node(self.source_id);
        TrackedChangeMetadata {
            id: node
                .and_then(|node| node.attribute("id"))
                .map(str::to_owned),
            author: node
                .and_then(|node| node.attribute("author"))
                .map(str::to_owned),
            date: node
                .and_then(|node| node.attribute("date"))
                .map(str::to_owned),
        }
    }

    pub fn text(&self) -> Result<String, SemanticError> {
        text_inside(self.source, self.source_id, RevisionView::Current, true)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackedChangeKind {
    Insertion,
    Deletion,
    MoveFrom,
    MoveTo,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrackedChangeMetadata {
    pub id: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
}

/// Selects semantic content before or after applying content revisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionView {
    Current,
    Original,
}

impl RevisionView {
    fn includes(self, kind: TrackedChangeKind) -> bool {
        matches!(
            (self, kind),
            (
                Self::Current,
                TrackedChangeKind::Insertion | TrackedChangeKind::MoveTo
            ) | (
                Self::Original,
                TrackedChangeKind::Deletion | TrackedChangeKind::MoveFrom
            )
        )
    }
}

pub(crate) fn tracked_changes(source: &SourceDocument) -> impl Iterator<Item = TrackedChange<'_>> {
    source.node_ids().filter_map(move |source_id| {
        kind(source, source_id).map(|kind| TrackedChange {
            source,
            source_id,
            kind,
        })
    })
}

pub(crate) fn text_for_view(
    source: &SourceDocument,
    container_id: NodeId,
    view: RevisionView,
) -> Result<String, SemanticError> {
    text_inside(source, container_id, view, false)
}

fn text_inside(
    source: &SourceDocument,
    container_id: NodeId,
    view: RevisionView,
    include_all_changes: bool,
) -> Result<String, SemanticError> {
    let mut text = String::new();
    for id in source.children(container_id) {
        collect_text(source, id, view, true, include_all_changes, &mut text)?;
    }
    Ok(text)
}

fn collect_text(
    source: &SourceDocument,
    id: NodeId,
    view: RevisionView,
    included: bool,
    include_all_changes: bool,
    text: &mut String,
) -> Result<(), SemanticError> {
    let included = match kind(source, id) {
        Some(kind) if !include_all_changes => included && view.includes(kind),
        _ => included,
    };
    if included && (word(source, id, "t") || word(source, id, "delText")) {
        text.push_str(&crate::semantic::text_value(source, id)?);
        return Ok(());
    }
    // Revision detail text can represent run tabs and line breaks without changing
    // the existing current/original document text semantics.
    if included && include_all_changes {
        if word(source, id, "tab") {
            text.push('\t');
        }
        if word(source, id, "br") || word(source, id, "cr") {
            text.push('\n');
        }
    }
    for child in source.children(id) {
        collect_text(source, child, view, included, include_all_changes, text)?;
    }
    Ok(())
}

fn kind(source: &SourceDocument, id: NodeId) -> Option<TrackedChangeKind> {
    if word(source, id, "ins") {
        Some(TrackedChangeKind::Insertion)
    } else if word(source, id, "del") {
        Some(TrackedChangeKind::Deletion)
    } else if word(source, id, "moveFrom") {
        Some(TrackedChangeKind::MoveFrom)
    } else if word(source, id, "moveTo") {
        Some(TrackedChangeKind::MoveTo)
    } else {
        None
    }
}

fn word(source: &SourceDocument, id: NodeId, name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name: xml_name, .. }) if xml_name.local_name() == name && xml_name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}

/// Main-document revisions only, in source order (including table paragraphs).
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionInspection {
    pub ok: bool,
    pub total: usize,
    pub insertion_count: usize,
    pub deletion_count: usize,
    pub unsupported_count: usize,
    pub offset: usize,
    pub has_more: bool,
    pub revisions: Vec<RevisionSnapshot>,
    pub diagnostics: Vec<crate::style_inspection::StyleInspectionDiagnostic>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionSnapshot {
    pub index: usize,
    pub id: Option<String>,
    pub kind: RevisionKind,
    pub author: Option<String>,
    pub date: Option<String>,
    pub text: Option<String>,
    pub paragraph_index: Option<usize>,
    pub structure: &'static str,
    pub truncated: bool,
    pub diagnostics: Vec<&'static str>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RevisionKind {
    Insertion,
    Deletion,
    Unsupported,
}

fn revision_node(source: &SourceDocument, id: NodeId) -> bool {
    kind(source, id).is_some()
        || [
            "pPrChange",
            "rPrChange",
            "sectPrChange",
            "tblPrChange",
            "tblGridChange",
            "trPrChange",
            "tcPrChange",
            "cellIns",
            "cellDel",
            "cellMerge",
            "numberingChange",
            "moveFromRangeStart",
            "moveToRangeStart",
            "customXmlInsRangeStart",
            "customXmlDelRangeStart",
            "customXmlMoveFromRangeStart",
            "customXmlMoveToRangeStart",
        ]
        .iter()
        .any(|name| word(source, id, name))
}

// Only interpret text containers we already understand. Property snapshots do
// not contribute text; fields, drawings, and unfamiliar content stay unsupported.
fn simple_revision_content(source: &SourceDocument, id: NodeId) -> bool {
    source.children(id).all(|child| {
        if ["rPr", "sdtPr"]
            .iter()
            .any(|name| word(source, child, name))
        {
            return true;
        }
        if matches!(
            source.node(child).map(|n| n.kind()),
            Some(SourceNodeKind::Text)
        ) {
            return true;
        }
        [
            "r",
            "t",
            "delText",
            "tab",
            "br",
            "cr",
            "hyperlink",
            "sdt",
            "sdtContent",
            "bookmarkStart",
            "bookmarkEnd",
            "commentRangeStart",
            "commentRangeEnd",
            "commentReference",
        ]
        .iter()
        .any(|name| word(source, child, name))
            && simple_revision_content(source, child)
    })
}

fn bounded_revision(value: String) -> (String, bool) {
    let truncated = value.chars().count() > 2_000;
    (value.chars().take(2_000).collect(), truncated)
}

pub fn inspect_docx_tracked_changes(
    input: Vec<u8>,
    offset: usize,
    limit: usize,
) -> RevisionInspection {
    let mut result = RevisionInspection {
        ok: true,
        total: 0,
        insertion_count: 0,
        deletion_count: 0,
        unsupported_count: 0,
        offset,
        has_more: false,
        revisions: Vec::new(),
        diagnostics: Vec::new(),
    };
    let read = || -> Result<_, String> {
        let package = opensuite_opc::Package::from_bytes(input).map_err(|e| e.code().to_owned())?;
        crate::open_main_source(&package).map_err(|e| e.code().to_owned())
    };
    let (_, source) = match read() {
        Ok(value) => value,
        Err(code) => {
            result.ok = false;
            result
                .diagnostics
                .push(crate::style_inspection::StyleInspectionDiagnostic {
                    code,
                    message: "could not inspect tracked changes".into(),
                });
            return result;
        }
    };
    // Reuse the source index; no additional XML parser or document tree.
    let mut paragraphs = std::collections::HashMap::new();
    let mut changes = Vec::new();
    let mut nested = std::collections::HashSet::new();
    for id in source.node_ids() {
        if word(&source, id, "p") {
            paragraphs.insert(id, paragraphs.len());
        }
        if !revision_node(&source, id) {
            continue;
        }
        let mut paragraph = None;
        let mut property = false;
        let mut parent = source.node(id).and_then(|n| n.parent());
        while let Some(ancestor) = parent {
            if paragraph.is_none() {
                paragraph = paragraphs.get(&ancestor).copied();
            }
            property |= ["pPr", "rPr", "trPr", "tcPr"]
                .iter()
                .any(|name| word(&source, ancestor, name));
            if revision_node(&source, ancestor) {
                nested.insert(id);
                nested.insert(ancestor);
            }
            parent = source.node(ancestor).and_then(|n| n.parent());
        }
        changes.push((id, paragraph, property));
    }
    result.total = changes.len();
    let limit = limit.clamp(1, 100);
    for (index, (id, paragraph_index, property)) in changes.into_iter().enumerate() {
        let kind = match kind(&source, id) {
            Some(TrackedChangeKind::Insertion) => RevisionKind::Insertion,
            Some(TrackedChangeKind::Deletion) => RevisionKind::Deletion,
            _ => RevisionKind::Unsupported,
        };
        let unavailable = !property
            && !nested.contains(&id)
            && !matches!(kind, RevisionKind::Unsupported)
            && !simple_revision_content(&source, id);
        let unsupported = unavailable
            || property
            || paragraph_index.is_none()
            || nested.contains(&id)
            || matches!(kind, RevisionKind::Unsupported);
        if unsupported {
            result.unsupported_count += 1;
        } else {
            match kind {
                RevisionKind::Insertion => result.insertion_count += 1,
                RevisionKind::Deletion => result.deletion_count += 1,
                _ => {}
            }
        }
        if index < offset || index - offset >= limit {
            continue;
        }
        let mut diagnostics = Vec::new();
        if unsupported {
            diagnostics.push("UNSUPPORTED_REVISION_TYPE");
        }
        if unavailable {
            diagnostics.push("REVISION_TEXT_UNAVAILABLE");
        }
        if nested.contains(&id) {
            diagnostics.push("NESTED_REVISION_UNSUPPORTED");
        }
        let node = source.node(id).unwrap();
        let mut truncated = false;
        let mut metadata = |name| {
            node.attribute(name).map(|value| {
                let (value, cut) = bounded_revision(value.to_owned());
                truncated |= cut;
                value
            })
        };
        let revision_id = metadata("id");
        let author = metadata("author");
        let date = metadata("date");
        let malformed = node
            .attribute("id")
            .is_none_or(|v| v.parse::<i32>().is_err());
        if malformed {
            diagnostics.push("MALFORMED_REVISION");
        }
        let text = if unsupported {
            None
        } else {
            match (TrackedChange {
                source: &source,
                source_id: id,
                kind: self::kind(&source, id).unwrap(),
            })
            .text()
            {
                Ok(value) => {
                    let (text, cut) = bounded_revision(value);
                    truncated |= cut;
                    Some(text)
                }
                Err(_) => {
                    diagnostics.push("REVISION_TEXT_UNAVAILABLE");
                    None
                }
            }
        };
        result.revisions.push(RevisionSnapshot {
            index,
            id: revision_id,
            kind,
            author,
            date,
            text,
            paragraph_index,
            structure: if unsupported {
                "unsupported"
            } else if malformed {
                "malformed"
            } else {
                "supported"
            },
            truncated,
            diagnostics,
        });
    }
    result.has_more = offset.saturating_add(result.revisions.len()) < result.total;
    if result.unsupported_count > 0 {
        result
            .diagnostics
            .push(crate::style_inspection::StyleInspectionDiagnostic {
                code: "UNSUPPORTED_REVISION_TYPE".into(),
                message:
                    "Complex revisions are counted and preserved; their text is not interpreted."
                        .into(),
            });
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::{BodyBlock, DocxDocument, SourceDocument};

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    #[test]
    fn discovers_content_revisions_and_filters_current_and_original_text() {
        let source = SourceDocument::parse(format!(
            "<x:document xmlns:x=\"{WORD}\"><x:body><x:p><x:r><x:t>A </x:t></x:r><x:del x:id=\"5\" x:author=\"Bob\" x:date=\"2026-09-04T12:05:00Z\"><x:r><x:delText>old </x:delText></x:r><x:r><x:delText>value</x:delText></x:r></x:del><x:ins x:id=\"4\" x:author=\"Alice\" x:date=\"2026-09-04T12:00:00Z\"><x:r><x:t>new </x:t></x:r><x:r><x:t>value</x:t></x:r></x:ins><x:r><x:t>!</x:t></x:r></x:p><x:tbl><x:tr><x:tc><x:p><x:moveFrom x:id=\"6\"><x:r><x:delText>Before</x:delText></x:r></x:moveFrom><x:moveTo x:id=\"7\"><x:r><x:t>After</x:t></x:r></x:moveTo></x:p></x:tc></x:tr></x:tbl><x:p><x:ins><x:r><x:t>Missing metadata</x:t></x:r></x:ins></x:p></x:body></x:document>"
        ).into_bytes()).unwrap();
        let document = DocxDocument::new(&source).unwrap();
        let changes: Vec<_> = document.tracked_changes().collect();

        assert_eq!(changes.len(), 5);
        assert_eq!(changes[0].kind(), TrackedChangeKind::Deletion);
        assert_eq!(changes[0].text().unwrap(), "old value");
        assert_eq!(changes[1].kind(), TrackedChangeKind::Insertion);
        assert_eq!(changes[1].text().unwrap(), "new value");
        assert_eq!(changes[1].metadata().author.as_deref(), Some("Alice"));
        assert_eq!(changes[1].metadata().id.as_deref(), Some("4"));
        assert_eq!(changes[2].kind(), TrackedChangeKind::MoveFrom);
        assert_eq!(changes[3].kind(), TrackedChangeKind::MoveTo);
        assert_eq!(changes[4].metadata(), TrackedChangeMetadata::default());
        assert_eq!(
            document
                .paragraphs()
                .next()
                .unwrap()
                .text_for_view(RevisionView::Current)
                .unwrap(),
            "A new value!"
        );
        assert_eq!(
            document
                .paragraphs()
                .next()
                .unwrap()
                .text_for_view(RevisionView::Original)
                .unwrap(),
            "A old value!"
        );
        let BodyBlock::Table(table) = document.blocks().nth(1).unwrap() else {
            panic!("second block must be a table")
        };
        let cell = table.rows().next().unwrap().cells().next().unwrap();
        assert_eq!(cell.text_for_view(RevisionView::Current).unwrap(), "After");
        assert_eq!(
            cell.text_for_view(RevisionView::Original).unwrap(),
            "Before"
        );
    }

    #[test]
    fn collects_hyperlink_and_content_control_text_inside_an_insertion() {
        let source = SourceDocument::parse(format!(
            "<document xmlns=\"{WORD}\"><body><p><ins><hyperlink><r><t>Link </t></r></hyperlink><sdt><sdtContent><r><t>Control</t></r></sdtContent></sdt></ins></p></body></document>"
        ).into_bytes()).unwrap();
        let change = DocxDocument::new(&source)
            .unwrap()
            .tracked_changes()
            .next()
            .unwrap();

        assert_eq!(change.text().unwrap(), "Link Control");
    }
    #[test]
    fn revision_snapshot_bounds_text_and_reports_complex_changes() {
        let metadata = r#"w:id="1" w:author="Ada" w:date="2026-10-03T12:00:00Z""#;
        let xml = format!(
            r#"<w:document xmlns:w="{WORD}"><w:body><w:p><w:ins {metadata}><w:r><w:t>{}</w:t></w:r></w:ins><w:ins {metadata}><w:del {metadata}><w:r><w:delText>nested</w:delText></w:r></w:del></w:ins><w:moveFrom {metadata}><w:r><w:delText>moved</w:delText></w:r></w:moveFrom><w:pPr><w:pPrChange {metadata}><w:pPr/></w:pPrChange><w:rPr><w:ins {metadata}/></w:rPr></w:pPr><w:ins><w:r><w:t>missing metadata</w:t></w:r></w:ins></w:p></w:body></w:document>"#,
            "é".repeat(2_010)
        );
        let package = opensuite_opc::Package::from_bytes(crate::create_blank_docx()).unwrap();
        let (main, _) = crate::open_main_source(&package).unwrap();
        let input = package
            .write_replaced_part_to_vec(&main, xml.as_bytes())
            .unwrap();
        let snapshot = crate::inspect_docx_tracked_changes(input.clone(), 0, 1);
        assert_eq!(snapshot.total, 7);
        assert_eq!(snapshot.unsupported_count, 5);
        assert_eq!(snapshot.insertion_count, 2);
        assert_eq!(snapshot.revisions.len(), 1);
        assert!(snapshot.has_more);
        assert!(snapshot.revisions[0].truncated);
        assert_eq!(
            snapshot.revisions[0].text.as_ref().unwrap().chars().count(),
            2_000
        );
        let snapshot = crate::inspect_docx_tracked_changes(input.clone(), 1, 1000);
        assert_eq!(snapshot.revisions.len(), 6);
        assert!(!snapshot.has_more);
        assert!(
            snapshot.revisions[0]
                .diagnostics
                .contains(&"NESTED_REVISION_UNSUPPORTED")
        );
        assert!(snapshot.revisions[0].text.is_none());
        assert!(
            snapshot
                .revisions
                .last()
                .unwrap()
                .diagnostics
                .contains(&"MALFORMED_REVISION")
        );
        assert!(
            crate::inspect_docx_tracked_changes(input, usize::MAX, 20)
                .revisions
                .is_empty()
        );
        assert!(!crate::inspect_docx_tracked_changes(vec![0], 0, 20).ok);
    }
}
