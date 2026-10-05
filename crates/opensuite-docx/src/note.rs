//! Source-backed footnotes and endnotes. Special records never become user notes.
use crate::{NodeId, RevisionView, SourceDocument, SourceNodeKind};
use opensuite_opc::{Package, PackageError, Part, RelationshipTarget};
use opensuite_protocol::{NoteKind, OperationResult};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
pub(crate) fn name(kind: NoteKind) -> &'static str {
    match kind {
        NoteKind::Footnote => "footnote",
        NoteKind::Endnote => "endnote",
    }
}
pub(crate) fn content_type(kind: NoteKind) -> String {
    format!(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.{}s+xml",
        name(kind)
    )
}
pub(crate) fn relationship(kind: NoteKind, strict: bool) -> String {
    format!(
        "{}/{}s",
        if strict {
            "http://purl.oclc.org/ooxml/officeDocument/relationships"
        } else {
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
        },
        name(kind)
    )
}
pub(crate) fn word(source: &SourceDocument, id: NodeId, local: &str) -> bool {
    matches!(source.node(id).map(|n| n.kind()), Some(SourceNodeKind::Element {name,..}) if name.local_name()==local && name.namespace_uri().is_some_and(|uri| NS.contains(&uri)))
}
pub(crate) fn error(message: impl Into<String>) -> OperationResult {
    OperationResult::failed("MALFORMED_NOTE_STRUCTURE", message)
        .with_reason_code("MALFORMED_NOTE_STRUCTURE")
}
pub(crate) struct NotePart {
    pub kind: NoteKind,
    pub part: Option<Part>,
    pub source: Option<SourceDocument>,
    pub records: Vec<NoteRecord>,
    pub ids: HashSet<i32>,
}
pub(crate) struct NoteRecord {
    pub source_id: NodeId,
    pub id: Option<i32>,
    pub references: Vec<NodeId>,
    pub reference_index: Option<usize>,
    pub text_node: Option<NodeId>,
    pub malformed: bool,
}
pub(crate) struct NoteSet {
    pub parts: Vec<NotePart>,
    pub diagnostics: Vec<NoteDiagnostic>,
}
#[derive(Clone, Debug, Serialize)]
pub struct NoteDiagnostic {
    pub code: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub kind: &'static str,
    pub id: Option<i32>,
    pub handle: Option<String>,
    pub text: String,
    pub paragraph_index: Option<usize>,
    pub reference_index: Option<usize>,
    pub reference_count: usize,
    pub structure: &'static str,
    pub truncated: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSettings {
    pub kind: &'static str,
    pub part_name: String,
    pub section_index: Option<usize>,
    pub number_format: Option<String>,
    pub restart: Option<String>,
    pub position: Option<String>,
    pub start: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteInspection {
    pub ok: bool,
    pub notes: Vec<NoteSummary>,
    pub total: usize,
    pub footnote_count: usize,
    pub endnote_count: usize,
    pub unsupported_count: usize,
    pub offset: usize,
    pub has_more: bool,
    pub settings: Vec<NoteSettings>,
    pub diagnostics: Vec<NoteDiagnostic>,
}

pub(crate) fn load(
    package: &Package,
    main: &Part,
    body: &SourceDocument,
) -> Result<NoteSet, OperationResult> {
    let relationships = match package.part_relationships(main) {
        Ok(r) => r,
        Err(PackageError::MissingPartRelationships(_)) => Vec::new(),
        Err(e) => return Err(error(e.to_string())),
    };
    let mut set = NoteSet {
        parts: Vec::new(),
        diagnostics: Vec::new(),
    };
    for kind in [NoteKind::Footnote, NoteKind::Endnote] {
        let mut links = relationships.iter().filter(|r| {
            r.relationship_type.as_str() == relationship(kind, false)
                || r.relationship_type.as_str() == relationship(kind, true)
        });
        let link = links.next();
        if links.next().is_some() {
            return Err(error("more than one notes relationship for the same kind"));
        }
        let (part, source) = if let Some(link) = link {
            let RelationshipTarget::Internal { part_name, .. } = &link.target else {
                return Err(error("external notes relationships are unsupported"));
            };
            let part = package.part(part_name).map_err(|e| error(e.to_string()))?;
            if part.content_type.as_str() != content_type(kind) {
                return Err(error("notes part has the wrong content type"));
            }
            let source =
                SourceDocument::parse(package.read_part(&part).map_err(|e| error(e.to_string()))?)
                    .map_err(|e| error(e.to_string()))?;
            if !word(&source, source.root(), &format!("{}s", name(kind))) {
                return Err(error("notes part has the wrong root"));
            }
            (Some(part), Some(source))
        } else {
            (None, None)
        };
        let mut notes = NotePart {
            kind,
            part,
            source,
            records: Vec::new(),
            ids: HashSet::new(),
        };
        let mut by_id = HashMap::new();
        let mut system_ids = Vec::new();
        if let Some(source) = &notes.source {
            for source_id in source.children(source.root()) {
                if matches!(source.node(source_id).unwrap().kind(), SourceNodeKind::Text) {
                    continue;
                }
                if !word(source, source_id, name(kind)) {
                    return Err(error("unsupported child of notes part"));
                }
                let node = source.node(source_id).unwrap();
                let id = node.attribute("id").and_then(|v| v.parse::<i32>().ok());
                let ordinary = node.attribute("type").is_none_or(|v| v == "normal");
                let duplicate = id.is_some_and(|id| !notes.ids.insert(id));
                if id.is_none() || duplicate {
                    set.diagnostics.push(NoteDiagnostic {
                        code: "MALFORMED_NOTE_STRUCTURE".into(),
                        message: format!(
                            "{} record has a missing, invalid, or duplicate ID",
                            name(kind)
                        ),
                    });
                }
                if !ordinary {
                    if let Some(id) = id {
                        system_ids.push(id);
                    }
                    if !matches!(
                        node.attribute("type"),
                        Some("separator" | "continuationSeparator" | "continuationNotice")
                    ) {
                        set.diagnostics.push(NoteDiagnostic {
                            code: "MALFORMED_NOTE_STRUCTURE".into(),
                            message: "unknown special note type".into(),
                        });
                    }
                    continue;
                }
                let index = notes.records.len();
                if let Some(id) = id {
                    if let Some(previous) = by_id.insert(id, index) {
                        notes.records[previous].malformed = true;
                    }
                }
                let invalid_text = text(source, source_id).is_err();
                if invalid_text {
                    set.diagnostics.push(NoteDiagnostic {
                        code: "MALFORMED_NOTE_STRUCTURE".into(),
                        message: "note text cannot be decoded".into(),
                    });
                }
                notes.records.push(NoteRecord {
                    source_id,
                    id,
                    references: Vec::new(),
                    reference_index: None,
                    text_node: simple_text_node(source, source_id, kind),
                    malformed: id.is_none() || duplicate || invalid_text,
                });
            }
        }
        for (reference_index, reference) in body
            .node_ids()
            .filter(|id| word(body, *id, &format!("{}Reference", name(kind))))
            .enumerate()
        {
            let id = body
                .node(reference)
                .unwrap()
                .attribute("id")
                .and_then(|v| v.parse::<i32>().ok());
            if let Some(index) = id.and_then(|id| by_id.get(&id)).copied() {
                notes.records[index].references.push(reference);
                notes.records[index]
                    .reference_index
                    .get_or_insert(reference_index);
                if body
                    .children(reference)
                    .any(|c| !matches!(body.node(c).unwrap().kind(), SourceNodeKind::Text))
                {
                    notes.records[index].malformed = true;
                    set.diagnostics.push(NoteDiagnostic {
                        code: "MALFORMED_NOTE_STRUCTURE".into(),
                        message: "note reference must be an empty marker".into(),
                    });
                }
                if id.is_some_and(|id| system_ids.contains(&id)) {
                    notes.records[index].malformed = true;
                }
            } else {
                set.diagnostics.push(NoteDiagnostic {
                    code: "ORPHANED_NOTE_REFERENCE".into(),
                    message: format!("{} reference has no ordinary record", name(kind)),
                });
            }
        }
        for record in &mut notes.records {
            if record.references.len() != 1 {
                record.malformed = true;
                set.diagnostics.push(NoteDiagnostic {
                    code: "MALFORMED_NOTE_STRUCTURE".into(),
                    message: format!(
                        "{} {:?} has {} references; exactly one is required",
                        name(kind),
                        record.id,
                        record.references.len()
                    ),
                });
            }
        }
        set.parts.push(notes);
    }
    Ok(set)
}

/// Only one plain text run is editable. Imported multi-paragraph/rich notes remain inspectable.
fn simple_text_node(source: &SourceDocument, note: NodeId, kind: NoteKind) -> Option<NodeId> {
    let children: Vec<_> = source
        .children(note)
        .filter(|id| !matches!(source.node(*id).unwrap().kind(), SourceNodeKind::Text))
        .collect();
    if children.len() != 1 || !word(source, children[0], "p") {
        return None;
    }
    let mut text = None;
    let mut markers = 0;
    for id in source.children(children[0]) {
        if word(source, id, "pPr") {
            if contains_review(source, id) {
                return None;
            }
            continue;
        }
        if matches!(source.node(id).unwrap().kind(), SourceNodeKind::Text) {
            continue;
        }
        if !word(source, id, "r") {
            return None;
        }
        for child in source.children(id) {
            if word(source, child, "rPr") {
                if contains_review(source, child) {
                    return None;
                }
            } else if word(source, child, "t") {
                if text.replace(child).is_some() || source.children(child).count() != 1 {
                    return None;
                }
            } else if word(source, child, &format!("{}Ref", name(kind))) {
                markers += 1;
            } else if !word(source, child, "tab")
                && !matches!(source.node(child).unwrap().kind(), SourceNodeKind::Text)
            {
                return None;
            }
        }
    }
    if markers != 1 {
        return None;
    }
    text
}
fn contains_review(source: &SourceDocument, id: NodeId) -> bool {
    crate::tracked_change::revision_node(source, id)
        || source
            .children(id)
            .any(|child| contains_review(source, child))
}
pub(crate) fn text(source: &SourceDocument, note: NodeId) -> Result<String, OperationResult> {
    if word(source, note, "p") {
        return crate::tracked_change::text_for_view(source, note, RevisionView::Current)
            .map_err(|e| error(e.to_string()));
    }
    let mut result = Vec::new();
    for child in source.children(note) {
        let value = text(source, child)?;
        if !value.is_empty() {
            result.push(value);
        }
    }
    Ok(result.join("\n"))
}
pub(crate) fn stamp(body: &SourceDocument, notes: &NoteSet) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for bytes in std::iter::once(body.original_bytes()).chain(
        notes
            .parts
            .iter()
            .filter_map(|p| p.source.as_ref())
            .map(SourceDocument::original_bytes),
    ) {
        for byte in bytes {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
    }
    format!("{hash:016x}")
}
fn settings(source: &SourceDocument, part_name: &str, out: &mut Vec<NoteSettings>) {
    let sections: Vec<_> = source
        .node_ids()
        .filter(|id| word(source, *id, "sectPr"))
        .collect();
    for id in source.node_ids() {
        let kind = if word(source, id, "footnotePr") {
            NoteKind::Footnote
        } else if word(source, id, "endnotePr") {
            NoteKind::Endnote
        } else {
            continue;
        };
        let node = source.node(id).unwrap();
        let value = |local| {
            source
                .children(id)
                .find(|c| word(source, *c, local))
                .and_then(|c| source.node(c).unwrap().attribute("val"))
                .map(|v| v.chars().take(100).collect())
        };
        if out.len() >= 40 {
            break;
        }
        out.push(NoteSettings {
            kind: name(kind),
            part_name: part_name.into(),
            section_index: node
                .parent()
                .and_then(|p| sections.iter().position(|id| *id == p)),
            number_format: value("numFmt"),
            restart: value("numRestart"),
            position: value("pos"),
            start: value("numStart"),
        });
    }
}
pub fn inspect_docx_notes(input: Vec<u8>, offset: usize, limit: usize) -> NoteInspection {
    let mut result = NoteInspection {
        ok: true,
        notes: Vec::new(),
        total: 0,
        footnote_count: 0,
        endnote_count: 0,
        unsupported_count: 0,
        offset,
        has_more: false,
        settings: Vec::new(),
        diagnostics: Vec::new(),
    };
    let read = || -> Result<_, OperationResult> {
        let package = Package::from_bytes(input).map_err(|e| error(e.to_string()))?;
        let (main, body) = crate::open_main_source(&package).map_err(|e| error(e.to_string()))?;
        let set = load(&package, &main, &body)?;
        Ok((package, main, body, set))
    };
    let (package, main, body, set) = match read() {
        Ok(v) => v,
        Err(e) => {
            result.ok = false;
            result.diagnostics = e
                .diagnostics
                .into_iter()
                .map(|d| NoteDiagnostic {
                    code: d.code,
                    message: d.message,
                })
                .collect();
            return result;
        }
    };
    let stamp = stamp(&body, &set);
    let paragraphs: HashMap<_, _> = body
        .node_ids()
        .filter(|id| word(&body, *id, "p"))
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();
    let mut index = 0;
    for part in &set.parts {
        if part.kind == NoteKind::Footnote {
            result.footnote_count = part.records.len();
        } else {
            result.endnote_count = part.records.len();
        }
        for record in &part.records {
            let structure = if record.malformed {
                "malformed"
            } else if record.text_node.is_none() {
                result.unsupported_count += 1;
                "unsupported"
            } else {
                "supported"
            };
            if index >= offset && result.notes.len() < limit.clamp(1, 100) {
                let full =
                    text(part.source.as_ref().unwrap(), record.source_id).unwrap_or_default();
                let mut parent = record.references.first().copied();
                let mut paragraph_index = None;
                while let Some(id) = parent {
                    if let Some(index) = paragraphs.get(&id) {
                        paragraph_index = Some(*index);
                        break;
                    }
                    parent = body.node(id).and_then(|n| n.parent());
                }
                result.notes.push(NoteSummary {
                    kind: name(part.kind),
                    id: record.id,
                    handle: record
                        .id
                        .filter(|_| !record.malformed)
                        .map(|id| format!("note:{stamp}:{}:{id}", name(part.kind))),
                    text: full.chars().take(2000).collect(),
                    paragraph_index,
                    reference_index: record.reference_index,
                    reference_count: record.references.len(),
                    structure,
                    truncated: full.chars().count() > 2000,
                });
            }
            index += 1;
        }
    }
    result.total = index;
    result.has_more = offset.saturating_add(result.notes.len()) < index;
    result.diagnostics = set.diagnostics.into_iter().take(100).collect();
    result.ok = result.diagnostics.is_empty();
    settings(&body, main.name.as_str(), &mut result.settings);
    if let Ok(links) = package.part_relationships(&main) {
        for link in links
            .iter()
            .filter(|r| r.relationship_type.as_str().ends_with("/settings"))
        {
            if let RelationshipTarget::Internal { part_name, .. } = &link.target {
                if let Ok(bytes) = package.read_part_by_name(part_name) {
                    if let Ok(source) = SourceDocument::parse(bytes) {
                        settings(&source, part_name.as_str(), &mut result.settings);
                    }
                }
            }
        }
    }
    result
}
