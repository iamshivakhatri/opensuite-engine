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
}
