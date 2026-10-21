//! Bounded, read-only facts and exact selectors for existing content controls.
use crate::{ContentControlKind, SourceDocument, SourceNodeKind};
use opensuite_opc::Package;
use opensuite_protocol::ContentControlTarget;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentControlInspection {
    pub ok: bool,
    pub total: usize,
    pub offset: usize,
    pub has_more: bool,
    pub controls: Vec<ContentControlSnapshot>,
    pub diagnostics: Vec<crate::StyleInspectionDiagnostic>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentControlSnapshot {
    pub index: usize,
    pub tag: Option<String>,
    pub alias: Option<String>,
    pub kind: &'static str,
    pub text: Option<String>,
    pub truncated: bool,
    pub part_name: String,
    /// Zero-based paragraph in the main part: enclosing or first contained paragraph.
    pub paragraph_index: Option<usize>,
    pub container: &'static str,
    pub text_mutation_supported: bool,
    pub target: Option<ContentControlSelector>,
    pub target_match_count: usize,
    pub reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentControlSelector {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    pub occurrence: usize,
}

/// Main-document controls in source order; default 20/max 100 records and 2000 characters per string.
/// Selectors use the existing mutation matcher, including explicit duplicate occurrences.
pub fn inspect_docx_content_controls(
    input: Vec<u8>,
    offset: usize,
    limit: usize,
) -> ContentControlInspection {
    let mut result = ContentControlInspection {
        ok: true,
        total: 0,
        offset,
        has_more: false,
        controls: Vec::new(),
        diagnostics: Vec::new(),
    };
    let read = (|| {
        let package = Package::from_bytes(input).map_err(|e| e.code().to_owned())?;
        let (main, source) = crate::open_main_source(&package).map_err(|e| e.code().to_owned())?;
        crate::DocxDocument::new(&source).map_err(|e| e.code().to_owned())?;
        Ok::<_, String>((main, source))
    })();
    let (main, source) = match read {
        Ok(value) => value,
        Err(code) => {
            result.ok = false;
            result.diagnostics.push(crate::StyleInspectionDiagnostic {
                code,
                message: "could not inspect content controls".into(),
            });
            return result;
        }
    };
    let controls = crate::content_control::content_controls(&source).collect::<Vec<_>>();
    let properties = controls.iter().map(|c| c.properties()).collect::<Vec<_>>();
    let paragraphs = source
        .node_ids()
        .filter(|id| word(&source, *id, "p"))
        .collect::<Vec<_>>();
    result.total = controls.len();
    for (index, control) in controls
        .iter()
        .enumerate()
        .skip(offset)
        .take(limit.clamp(1, 100))
    {
        let p = &properties[index];
        let (tag, tag_cut) = bounded(p.tag.as_deref());
        let (alias, alias_cut) = bounded(p.alias.as_deref());
        let mut reasons = Vec::new();
        let (text, text_cut) = match control.bounded_visible_text(2000) {
            Ok((text, cut)) => (Some(text), cut),
            Err(_) => {
                reasons.push("TEXT_UNAVAILABLE".into());
                (None, false)
            }
        };
        let matches = properties
            .iter()
            .enumerate()
            .filter(|(_, other)| {
                p.tag
                    .as_ref()
                    .is_none_or(|tag| other.tag.as_ref() == Some(tag))
                    && p.alias
                        .as_ref()
                        .is_none_or(|alias| other.alias.as_ref() == Some(alias))
            })
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        let target = (p.tag.is_some() || p.alias.is_some()).then(|| ContentControlTarget {
            tag: p.tag.clone(),
            alias: p.alias.clone(),
            occurrence: matches.iter().position(|i| *i == index),
        });
        if matches.len() > 1 && target.is_some() {
            reasons.push("OCCURRENCE_REQUIRED".into());
        }
        let mut supported = false;
        if let Some(target) = &target {
            match if text_cut {
                Err(opensuite_protocol::OperationResult::failed(
                    "INSPECTION_VALUE_TRUNCATED",
                    "control text exceeds the inspection bound",
                ))
            } else {
                crate::mutation::content_control_text_support(&source, target)
            } {
                Ok(_) => supported = true,
                Err(error) => {
                    for diagnostic in error.diagnostics.iter().take(5) {
                        reasons.push(
                            diagnostic
                                .reason_code
                                .clone()
                                .unwrap_or_else(|| diagnostic.code.clone()),
                        );
                        reasons.push(diagnostic.message.chars().take(2000).collect());
                    }
                }
            }
        } else {
            reasons.push("MISSING_TAG_AND_ALIAS".into());
        }
        let truncated = tag_cut || alias_cut || text_cut;
        if truncated {
            reasons.push("INSPECTION_VALUE_TRUNCATED".into());
        }
        supported &= !truncated && text.is_some();
        let span = source.node(control.source_id()).unwrap().span();
        let paragraph_index = paragraphs.iter().position(|id| {
            let p = source.node(*id).unwrap().span();
            (p.start < span.start && p.end > span.end) || (p.start > span.start && p.end < span.end)
        });
        let mut parent = source.node(control.source_id()).and_then(|n| n.parent());
        let mut container = "body";
        while let Some(id) = parent {
            if word(&source, id, "tc") {
                container = "tableCell";
                break;
            }
            if word(&source, id, "p") {
                container = "inline";
                break;
            }
            if word(&source, id, "sdtContent") {
                container = "contentControl";
                break;
            }
            parent = source.node(id).and_then(|n| n.parent());
        }
        result.controls.push(ContentControlSnapshot {
            index,
            tag,
            alias,
            kind: kind_name(p.kind),
            text,
            truncated,
            part_name: main.name.as_str().into(),
            paragraph_index,
            container,
            text_mutation_supported: supported,
            target: target
                .filter(|_| !tag_cut && !alias_cut)
                .map(|t| ContentControlSelector {
                    tag: t.tag,
                    alias: t.alias,
                    occurrence: t.occurrence.unwrap_or(0),
                }),
            target_match_count: if p.tag.is_some() || p.alias.is_some() {
                matches.len()
            } else {
                0
            },
            reasons,
        });
    }
    result.has_more = offset.saturating_add(result.controls.len()) < result.total;
    result
}
fn bounded(value: Option<&str>) -> (Option<String>, bool) {
    (
        value.map(|v| v.chars().take(2000).collect()),
        value.is_some_and(|v| v.chars().count() > 2000),
    )
}
fn word(source: &SourceDocument, id: crate::NodeId, local: &str) -> bool {
    matches!(source.node(id).map(|n| n.kind()), Some(SourceNodeKind::Element { name, .. }) if name.local_name() == local && matches!(name.namespace_uri(), Some("http://schemas.openxmlformats.org/wordprocessingml/2006/main" | "http://purl.oclc.org/ooxml/wordprocessingml/main")))
}
fn kind_name(kind: ContentControlKind) -> &'static str {
    match kind {
        ContentControlKind::Text => "text",
        ContentControlKind::RichText => "richText",
        ContentControlKind::Date => "date",
        ContentControlKind::DropDownList => "dropDownList",
        ContentControlKind::ComboBox => "comboBox",
        ContentControlKind::CheckBox => "checkBox",
        ContentControlKind::Picture => "picture",
        ContentControlKind::Group => "group",
        ContentControlKind::RepeatingSection => "repeatingSection",
        ContentControlKind::RepeatingSectionItem => "repeatingSectionItem",
        ContentControlKind::Unknown => "unknown",
    }
}
