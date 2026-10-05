//! Bounded field facts from the existing source-backed field parser.
use crate::{FieldKind, FieldState, SourceDocument, field};
use opensuite_opc::Package;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInspection {
    pub ok: bool,
    pub total: usize,
    pub offset: usize,
    pub has_more: bool,
    pub fields: Vec<FieldSnapshot>,
    pub diagnostics: Vec<crate::StyleInspectionDiagnostic>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSnapshot {
    pub index: usize,
    pub kind: &'static str,
    pub representation: &'static str,
    pub instruction: Option<String>,
    pub cached_result: Option<String>,
    pub part_name: String,
    pub paragraph_index: Option<usize>,
    pub structure: &'static str,
    pub dirty: Option<bool>,
    pub locked: Option<bool>,
    pub heading_levels: Option<[u8; 2]>,
    pub truncated: bool,
    pub diagnostics: Vec<&'static str>,
}

/// Main document, then header/footer parts sorted by package name. Paragraph
/// indices are local to each part. Paging and text bounds do not change the XML.
pub fn inspect_docx_fields(input: Vec<u8>, offset: usize, limit: usize) -> FieldInspection {
    let mut result = FieldInspection {
        ok: true,
        total: 0,
        offset,
        has_more: false,
        fields: Vec::new(),
        diagnostics: Vec::new(),
    };
    let read = (|| -> Result<_, String> {
        let package = Package::from_bytes(input).map_err(|e| e.code().to_owned())?;
        let (main, source) = crate::open_main_source(&package).map_err(|e| e.code().to_owned())?;
        Ok((package, main, source))
    })();
    let (package, main, source) = match read {
        Ok(v) => v,
        Err(code) => {
            result.ok = false;
            diagnostic(&mut result, code, "could not inspect fields".into());
            return result;
        }
    };
    append_source(
        &mut result,
        &source,
        main.name.as_str(),
        limit.clamp(1, 100),
    );
    let mut names: Vec<_> = package
        .part_names_with_prefix("/")
        .filter(|name| {
            package.content_type(name).is_ok_and(|t| matches!(t.as_str(),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml" |
            "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"))
        })
        .cloned()
        .collect();
    names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    for name in names {
        match package
            .read_part_by_name(&name)
            .map_err(|e| e.code().to_owned())
            .and_then(|bytes| SourceDocument::parse(bytes).map_err(|e| e.code().to_owned()))
        {
            Ok(source) => append_source(&mut result, &source, name.as_str(), limit.clamp(1, 100)),
            Err(code) => {
                result.ok = false;
                diagnostic(
                    &mut result,
                    code,
                    format!("could not inspect {}", name.as_str()),
                );
            }
        }
    }
    result.has_more = result.offset.saturating_add(result.fields.len()) < result.total;
    result
}
fn diagnostic(result: &mut FieldInspection, code: String, message: String) {
    if result.diagnostics.len() < 100 {
        result
            .diagnostics
            .push(crate::StyleInspectionDiagnostic { code, message });
    }
}
fn flag(source: &SourceDocument, id: crate::NodeId, name: &str) -> Option<bool> {
    match source.node(id)?.attribute(name)? {
        "1" | "true" | "on" => Some(true),
        "0" | "false" | "off" => Some(false),
        _ => None,
    }
}
fn append_source(result: &mut FieldInspection, source: &SourceDocument, part: &str, limit: usize) {
    let fields = field::fields(source);
    for error in fields.errors() {
        diagnostic(
            result,
            error.code().into(),
            format!("field structure diagnostic in {part}"),
        );
    }
    let mut errors: Vec<_> = fields
        .errors()
        .filter_map(|e| {
            e.source_id()
                .map(|id| (source.node(id).unwrap().span().start, e.code()))
        })
        .collect();
    errors.sort_by_key(|v| v.0);
    let paragraphs: std::collections::HashMap<_, _> = source
        .node_ids()
        .filter(|id| word(source, *id, "p"))
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();
    let all: Vec<_> = fields.iter().collect();
    // Sorted intervals let us detect both outer and inner nested fields in one pass.
    let mut nested = std::collections::HashSet::new();
    let mut active: Vec<usize> = Vec::new();
    for (i, f) in all.iter().enumerate() {
        while active
            .last()
            .is_some_and(|j| all[*j].span().end <= f.span().start)
        {
            active.pop();
        }
        if let Some(j) = active.last() {
            nested.insert(*j);
            nested.insert(i);
        }
        active.push(i);
    }
    for (i, f) in all.iter().enumerate() {
        let index = result.total;
        result.total += 1;
        if index < result.offset || result.fields.len() == limit {
            continue;
        }
        let mut codes = Vec::new();
        let (instruction, cut_instruction) = f.bounded_text(true, 2000).unwrap_or_else(|_| {
            codes.push("FIELD_INSTRUCTION_UNAVAILABLE");
            (None, false)
        });
        let (mut cached_result, cut_result) = f.bounded_text(false, 2000).unwrap_or_else(|_| {
            codes.push("FIELD_RESULT_UNAVAILABLE");
            (None, false)
        });
        let kind = match instruction
            .as_deref()
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("")
            .to_ascii_uppercase()
            .as_str()
        {
            "PAGE" => "page",
            "NUMPAGES" => "numPages",
            "TOC" => "toc",
            "DATE" => "date",
            _ => "unknown",
        };
        let mut structure = "complete";
        if f.state() == FieldState::Unterminated {
            structure = "malformed";
            codes.push("UNTERMINATED_FIELD_BEGIN");
        }
        let span = f.span();
        let first_error = errors.partition_point(|(at, _)| *at < span.start);
        let last_error = errors.partition_point(|(at, _)| *at < span.end);
        if first_error < last_error {
            structure = "malformed";
            codes.extend(
                errors[first_error..last_error]
                    .iter()
                    .take(20)
                    .map(|(_, code)| *code),
            );
        }
        if instruction.as_deref().is_none_or(|s| s.trim().is_empty()) {
            structure = "malformed";
            codes.push("MISSING_FIELD_INSTRUCTION");
        }
        if f.kind() == FieldKind::Complex && !f.has_separator() {
            codes.push("FIELD_RESULT_UNAVAILABLE");
        }
        if nested.contains(&i) {
            structure = if structure == "malformed" {
                structure
            } else {
                "unsupported"
            };
            codes.push("NESTED_FIELD_UNSUPPORTED");
            cached_result = None;
        }
        if kind == "unknown" {
            codes.push("UNSUPPORTED_FIELD_KIND");
        }
        for name in ["dirty", "fldLock"] {
            if source
                .node(f.source_id())
                .unwrap()
                .attribute(name)
                .is_some()
                && flag(source, f.source_id(), name).is_none()
            {
                codes.push("INVALID_FIELD_FLAG");
                structure = "malformed";
            }
        }
        let mut parent = Some(f.source_id());
        let mut paragraph_index = None;
        while let Some(id) = parent {
            if let Some(index) = paragraphs.get(&id) {
                paragraph_index = Some(*index);
                break;
            }
            parent = source.node(id).and_then(|n| n.parent());
        }
        if paragraph_index.is_none() {
            codes.push("FIELD_OUTSIDE_PARAGRAPH");
            if structure == "complete" {
                structure = "unsupported";
            }
        }
        let heading_levels = if kind == "toc" && !cut_instruction {
            instruction.as_deref().and_then(toc_levels)
        } else {
            None
        };
        codes.sort_unstable();
        codes.dedup();
        codes.truncate(20);
        result.fields.push(FieldSnapshot {
            index,
            kind,
            representation: if f.kind() == FieldKind::Simple {
                "simple"
            } else {
                "complex"
            },
            instruction,
            cached_result,
            part_name: part.into(),
            paragraph_index,
            structure,
            dirty: flag(source, f.source_id(), "dirty"),
            locked: flag(source, f.source_id(), "fldLock"),
            heading_levels,
            truncated: cut_instruction || cut_result,
            diagnostics: codes,
        });
    }
}
fn toc_levels(instruction: &str) -> Option<[u8; 2]> {
    let words: Vec<_> = instruction.split_whitespace().collect();
    let i = words.iter().position(|s| *s == "\\o")?;
    let range = words.get(i + 1)?.strip_prefix('"')?.strip_suffix('"')?;
    let (a, b) = range.split_once('-')?;
    let (a, b) = (a.parse::<u8>().ok()?, b.parse::<u8>().ok()?);
    (a >= 1 && b <= 9 && a <= b).then_some([a, b])
}
fn word(source: &SourceDocument, id: crate::NodeId, name: &str) -> bool {
    matches!(source.node(id).map(|n| n.kind()), Some(crate::SourceNodeKind::Element { name: n,.. }) if n.local_name() == name && n.namespace_uri().is_some_and(|s| super::mutation::NS.contains(&s)))
}
