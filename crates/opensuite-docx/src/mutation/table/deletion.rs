use super::*;

/// Check all source regions before any deletion patch is applied.
/// Even contained ranges are refused: removing dependent records is outside
/// table deletion, and bookmarks may have references in other package parts.
pub(super) fn ensure_safe_deletion(
    source: &SourceDocument,
    regions: &[SourceSpan],
) -> Result<(), OperationResult> {
    // Callers supply disjoint regions in source order (including column cells).
    let intersects = |span: SourceSpan| {
        let index = regions.partition_point(|region| region.end <= span.start);
        regions
            .get(index)
            .is_some_and(|region| region.start < span.end)
    };
    let refuse = |structure: &str| {
        unsupported(format!(
            "deletion intersects {structure}; protected ranges and dependent structures must remain intact"
        ))
        .with_reason_code("UNSUPPORTED_STRUCTURAL_DELETE")
    };
    let mut starts = std::collections::HashMap::new();
    let mut has_fields = false;
    for id in source.node_ids() {
        let node = source.node(id).expect("source node exists");
        let SourceNodeKind::Element { name, .. } = node.kind() else {
            continue;
        };
        if !name.namespace_uri().is_some_and(|uri| NS.contains(&uri)) {
            continue;
        }
        let local = name.local_name();
        let span = node.span();
        let pair = match local {
            "bookmarkStart" => Some(("bookmark", true, node.attribute("id"))),
            "bookmarkEnd" => Some(("bookmark", false, node.attribute("id"))),
            "permStart" => Some(("permission", true, node.attribute("id"))),
            "permEnd" => Some(("permission", false, node.attribute("id"))),
            "proofErr" => {
                let kind = node.attribute("type").unwrap_or("");
                kind.strip_suffix("Start")
                    .map(|key| ("proof/error", true, Some(key)))
                    .or_else(|| {
                        kind.strip_suffix("End")
                            .map(|key| ("proof/error", false, Some(key)))
                    })
            }
            _ => local
                .strip_suffix("RangeStart")
                .map(|kind| (kind, true, node.attribute("id")))
                .or_else(|| {
                    local
                        .strip_suffix("RangeEnd")
                        .map(|kind| (kind, false, node.attribute("id")))
                }),
        };
        if let Some((kind, start, key)) = pair {
            if intersects(span) {
                return Err(refuse(&format!("{kind} range")));
            }
            let key = key.ok_or_else(|| refuse("malformed range markers"))?;
            if start {
                if starts.insert((kind, key), span.start).is_some() {
                    return Err(refuse("duplicate range starts"));
                }
            } else {
                let begin = starts
                    .remove(&(kind, key))
                    .ok_or_else(|| refuse("unmatched range end"))?;
                if intersects(SourceSpan {
                    start: begin,
                    end: span.end,
                }) {
                    return Err(refuse(&format!("{kind} range")));
                }
            }
        }
        has_fields |= matches!(local, "fldChar" | "fldSimple" | "instrText");
        if intersects(span) {
            let structure = match local {
                "fldChar" | "fldSimple" | "instrText" => "field range",
                "commentReference" => "comment reference",
                "sdt" => "content control",
                "hyperlink" => "hyperlink reference",
                "footnoteReference" | "endnoteReference" => "note reference",
                "drawing" | "pict" | "object" | "altChunk" => "dependent package content",
                "ins" | "del" | "moveFrom" | "moveTo" => "tracked revision",
                "sectPr" => "section properties",
                "proofErr" => "unsupported proof/error marker",
                _ if local.ends_with("Change")
                    || matches!(local, "cellIns" | "cellDel" | "cellMerge") =>
                {
                    "tracked property change"
                }
                _ => continue,
            };
            return Err(refuse(structure));
        }
    }
    if !starts.is_empty() {
        return Err(refuse("unmatched range start"));
    }
    if has_fields {
        let fields = crate::field::fields(source);
        if fields.errors().next().is_some() || fields.iter().any(|field| intersects(field.span())) {
            return Err(refuse("field range or malformed field markers"));
        }
    }
    Ok(())
}
