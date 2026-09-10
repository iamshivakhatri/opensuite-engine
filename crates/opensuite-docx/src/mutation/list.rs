use super::*;

/// Sets or clears one ordinary level-zero list over consecutive direct body paragraphs.
pub fn set_paragraphs_list_to_vec(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetParagraphsList,
) -> Result<Vec<u8>, OperationResult> {
    let resolved = resolve_list_paragraphs(source, &operation.targets)?;
    let body_before = body_texts(source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    let (numbering_name, numbering_bytes, abstract_id, num_id) = match operation.kind {
        ParagraphListKind::None => (None, None, None, None),
        _ => {
            let name =
                existing_numbering_part(package, main)?.unwrap_or(numbering_part_name(main)?);
            let existing = match package.read_part_by_name(&name) {
                Ok(bytes) => Some(bytes),
                Err(PackageError::MissingTargetPart(_)) => None,
                Err(error) => return Err(OperationResult::failed(error.code(), error.to_string())),
            };
            let (abstract_id, num_id) = numbering_ids(existing.as_deref())?;
            (Some(name), existing, Some(abstract_id), Some(num_id))
        }
    };
    let patches = list_paragraph_patches(source, &resolved, operation.kind, num_id)?;
    let document = apply_patches(source, patches)?;
    let mut replaced = vec![(main.name.clone(), document.as_slice())];
    let mut added = Vec::new();
    if let (Some(name), Some(abstract_id), Some(num_id)) = (numbering_name, abstract_id, num_id) {
        let addition = numbering_xml(operation.kind, abstract_id, num_id)?;
        let bytes = numbering_bytes
            .as_deref()
            .map(|bytes| append_xml_element(bytes, &addition))
            .transpose()?
            .unwrap_or_else(|| numbering_document(&addition).into_bytes());
        if numbering_bytes.is_some() {
            replaced.push((name.clone(), bytes.as_slice()));
        } else {
            added.push((name.clone(), bytes.as_slice()));
        }
        let relationship_bytes;
        if existing_numbering_part(package, main)?.is_none() {
            let (rels_name, rels_exist, rels, relationship_id) =
                picture_relationships(package, main)?;
            let target = name
                .as_str()
                .rsplit('/')
                .next()
                .expect("numbering file name");
            relationship_bytes = append_xml_element(
                &rels,
                &format!(
                    r#"<Relationship Id="{relationship_id}" Type="{NUMBERING_RELATIONSHIP_TYPE}" Target="{target}"/>"#
                ),
            )?;
            let rels = relationship_bytes.as_slice();
            if rels_exist {
                replaced.push((rels_name, rels));
            } else {
                added.push((rels_name, rels));
            }
        }
        let content_types_name = PartName::parse("/[Content_Types].xml")
            .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
        let content_types = package
            .read_part_by_name(&content_types_name)
            .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
        let numbering_override = format!("PartName=\"{}\"", name.as_str());
        if !content_types
            .windows(numbering_override.len())
            .any(|value| value == numbering_override.as_bytes())
        {
            let types = append_xml_element(
                &content_types,
                &format!(
                    r#"<Override PartName="{}" ContentType="{NUMBERING_CONTENT_TYPE}"/>"#,
                    name.as_str()
                ),
            )?;
            replaced.push((content_types_name, types.as_slice()));
            return write_list_package(
                package,
                replaced,
                added,
                &operation.targets,
                operation.kind,
                body_before,
            );
        }
        return write_list_package(
            package,
            replaced,
            added,
            &operation.targets,
            operation.kind,
            body_before,
        );
    }
    write_list_package(
        package,
        replaced,
        added,
        &operation.targets,
        operation.kind,
        body_before,
    )
}

pub fn set_paragraphs_list(
    package: &Package,
    main: &Part,
    source: &SourceDocument,
    operation: &SetParagraphsList,
    output: impl AsRef<Path>,
) -> OperationResult {
    let output = output.as_ref();
    if output_matches_input(package, output) {
        return OperationResult::failed(
            "OUTPUT_MATCHES_INPUT",
            "output path must differ from input path",
        );
    }
    match set_paragraphs_list_to_vec(package, main, source, operation) {
        Ok(bytes) => match std::fs::write(output, bytes) {
            Ok(()) => OperationResult::applied(String::new(), String::new()),
            Err(error) => OperationResult::failed("SERIALIZATION_FAILED", error.to_string()),
        },
        Err(error) => error,
    }
}

pub(super) fn resolve_list_paragraphs(
    source: &SourceDocument,
    targets: &[TextTarget],
) -> Result<Vec<(String, NodeId)>, OperationResult> {
    if targets.is_empty() || targets.len() > MAX_PARAGRAPHS_PER_OPERATION {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            format!(
                "set_paragraphs_list requires between 1 and {MAX_PARAGRAPHS_PER_OPERATION} targets"
            ),
        ));
    }
    let resolved = targets
        .iter()
        .map(|target| resolve_paragraph_anchor(source, target))
        .collect::<Result<Vec<_>, _>>()?;
    let body = body_texts(source)?;
    let positions = resolved
        .iter()
        .map(|(_, paragraph)| {
            body.iter()
                .position(|(id, _)| id == paragraph)
                .ok_or_else(|| {
                    OperationResult::failed(
                        "UNSUPPORTED_STRUCTURE",
                        "list targets must be direct body paragraphs",
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if positions.windows(2).any(|pair| pair[1] != pair[0] + 1) {
        return Err(OperationResult::failed(
            "INVALID_OPERATION",
            "list targets must be consecutive direct body paragraphs in source order",
        ));
    }
    Ok(resolved)
}

pub(super) fn list_paragraph_patches(
    source: &SourceDocument,
    paragraphs: &[(String, NodeId)],
    kind: ParagraphListKind,
    num_id: Option<u32>,
) -> Result<Vec<Patch>, OperationResult> {
    let mut patches = Vec::new();
    for (_, paragraph) in paragraphs {
        let prefix = word_prefix(source, *paragraph)?;
        let name = |local: &str| qualify(prefix, local);
        let value = match (kind, num_id) {
            (ParagraphListKind::None, _) => String::new(),
            (_, Some(num_id)) => format!(
                r#"<{}><{} {}val="0"/><{} {}val="{num_id}"/></{}>"#,
                name("numPr"),
                name("ilvl"),
                attr_prefix(prefix),
                name("numId"),
                attr_prefix(prefix),
                name("numPr")
            ),
            _ => {
                return Err(OperationResult::failed(
                    "DOCUMENT_INVALID",
                    "list numbering was not allocated",
                ));
            }
        };
        if let Some(ppr) = source
            .children(*paragraph)
            .find(|id| word(source, *id, "pPr"))
        {
            simple_property(source, ppr, "numPr", Some(value), &mut patches)?;
        } else if kind != ParagraphListKind::None {
            let at = source
                .children(*paragraph)
                .find_map(|id| {
                    source
                        .node(id)
                        .filter(|node| matches!(node.kind(), SourceNodeKind::Element { .. }))
                        .map(|node| node.span().start)
                })
                .ok_or_else(|| unsupported("paragraph has no insertion boundary"))?;
            patches.push(Patch {
                span: SourceSpan { start: at, end: at },
                replacement: format!("<{}>{}</{}>", name("pPr"), value, name("pPr")).into_bytes(),
            });
        }
    }
    Ok(patches)
}

pub(super) fn numbering_part_name(main: &Part) -> Result<PartName, OperationResult> {
    let directory = main
        .name
        .as_str()
        .rsplit_once('/')
        .map(|(directory, _)| directory)
        .ok_or_else(|| {
            OperationResult::failed("PACKAGE_CONFLICT", "main document part has no directory")
        })?;
    PartName::parse(format!("{directory}/numbering.xml"))
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))
}

pub(super) fn existing_numbering_part(
    package: &Package,
    main: &Part,
) -> Result<Option<PartName>, OperationResult> {
    let relationships = match package.part_relationships(main) {
        Ok(relationships) => relationships,
        Err(PackageError::MissingPartRelationships(_)) => return Ok(None),
        Err(error) => return Err(OperationResult::failed(error.code(), error.to_string())),
    };
    let Some(relationship) = relationships.into_iter().find(|relationship| {
        relationship.relationship_type.as_str() == NUMBERING_RELATIONSHIP_TYPE
    }) else {
        return Ok(None);
    };
    match relationship.target {
        opensuite_opc::RelationshipTarget::Internal { part_name, .. } => Ok(Some(part_name)),
        opensuite_opc::RelationshipTarget::External { .. } => Err(OperationResult::failed(
            "UNSUPPORTED_STRUCTURE",
            "external numbering relationships are unsupported",
        )),
    }
}

pub(super) fn numbering_ids(bytes: Option<&[u8]>) -> Result<(u32, u32), OperationResult> {
    let Some(bytes) = bytes else {
        return Ok((1, 1));
    };
    let numbering = crate::Numbering::parse(bytes.to_vec())
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    let abstract_id = numbering
        .source()
        .children(numbering.source().root())
        .filter(|id| word(numbering.source(), *id, "abstractNum"))
        .filter_map(|id| {
            numbering
                .source()
                .node(id)?
                .attribute("abstractNumId")?
                .parse::<u32>()
                .ok()
        })
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| {
            OperationResult::failed("PACKAGE_CONFLICT", "cannot allocate abstract numbering ID")
        })?;
    let num_id = numbering
        .instances()
        .map(|instance| instance.num_id.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| {
            OperationResult::failed("PACKAGE_CONFLICT", "cannot allocate numbering ID")
        })?;
    Ok((abstract_id, num_id))
}

pub(super) fn numbering_xml(
    kind: ParagraphListKind,
    abstract_id: u32,
    num_id: u32,
) -> Result<String, OperationResult> {
    let (format, text) = match kind {
        ParagraphListKind::Bullet => ("bullet", "•"),
        ParagraphListKind::Decimal => ("decimal", "%1."),
        ParagraphListKind::None => {
            return Err(OperationResult::failed(
                "INVALID_OPERATION",
                "clear-list does not allocate numbering",
            ));
        }
    };
    Ok(format!(
        r#"<w:abstractNum w:abstractNumId="{abstract_id}"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="{format}"/><w:lvlText w:val="{text}"/><w:suff w:val="space"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:num w:numId="{num_id}"><w:abstractNumId w:val="{abstract_id}"/></w:num>"#
    ))
}

pub(super) fn numbering_document(addition: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="{}">{addition}</w:numbering>"#,
        NS[0]
    )
}

pub(super) fn write_list_package(
    package: &Package,
    replaced: Vec<(PartName, &[u8])>,
    added: Vec<(PartName, &[u8])>,
    targets: &[TextTarget],
    kind: ParagraphListKind,
    body_before: Vec<String>,
) -> Result<Vec<u8>, OperationResult> {
    let output = package
        .write_package_with_named_changes_to_vec(&replaced, &added)
        .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
    let reopened = Package::from_bytes(output.clone()).map_err(document_invalid)?;
    reopened.verify().map_err(document_invalid)?;
    let (main, source) = crate::open_main_source(&reopened).map_err(document_invalid)?;
    let body_after = body_texts(&source)?
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    if body_before != body_after {
        return Err(OperationResult::failed(
            "DOCUMENT_INVALID",
            "list operation changed paragraph text or body ordering",
        ));
    }
    let mut references = Vec::new();
    for target in targets {
        let (_, paragraph) = resolve_paragraph_anchor(&source, target)?;
        let direct = source
            .children(paragraph)
            .find(|id| word(&source, *id, "pPr"))
            .and_then(|ppr| source.children(ppr).find(|id| word(&source, *id, "numPr")));
        if (kind == ParagraphListKind::None) != direct.is_none() {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "output list membership does not match request",
            ));
        }
        if let Some(num_pr) = direct {
            let level = source
                .children(num_pr)
                .find(|id| word(&source, *id, "ilvl"))
                .and_then(|id| source.node(id))
                .and_then(|node| node.attribute("val"))
                .and_then(|value| value.parse::<u8>().ok());
            let num_id = source
                .children(num_pr)
                .find(|id| word(&source, *id, "numId"))
                .and_then(|id| source.node(id))
                .and_then(|node| node.attribute("val"))
                .and_then(|value| value.parse::<u32>().ok());
            if level != Some(0) || num_id.is_none() {
                return Err(OperationResult::failed(
                    "DOCUMENT_INVALID",
                    "output list reference is invalid",
                ));
            }
            references.push(num_id.expect("checked above"));
        }
    }
    if kind != ParagraphListKind::None {
        if references.windows(2).any(|pair| pair[0] != pair[1]) {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "output list paragraphs do not share one numbering instance",
            ));
        }
        let numbering = crate::load_numbering(&reopened, &main)
            .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?
            .ok_or_else(|| {
                OperationResult::failed("DOCUMENT_INVALID", "output numbering part is unavailable")
            })?;
        let level = numbering
            .resolve(crate::ListReference {
                num_id: crate::NumberingId(references[0]),
                level: 0,
            })
            .map_err(|error| OperationResult::failed(error.code(), error.to_string()))?;
        let expected_format = match kind {
            ParagraphListKind::Bullet => crate::NumberFormat::Bullet,
            ParagraphListKind::Decimal => crate::NumberFormat::Decimal,
            ParagraphListKind::None => unreachable!(),
        };
        if level.format != expected_format
            || (kind == ParagraphListKind::Decimal && level.start != Some(1))
        {
            return Err(OperationResult::failed(
                "DOCUMENT_INVALID",
                "output numbering definition does not match request",
            ));
        }
    }
    Ok(output)
}
