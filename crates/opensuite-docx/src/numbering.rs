use std::{collections::HashMap, fmt};

use crate::{NodeId, SourceDocument, SourceError, SourceNodeKind};
use opensuite_opc::{Package, PackageError, Part};

const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
const REL: [&str; 2] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering",
    "http://purl.oclc.org/ooxml/officeDocument/relationships/numbering",
];

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NumberingId(pub u32);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AbstractNumberingId(pub u32);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListReference {
    pub num_id: NumberingId,
    pub level: u8,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NumberFormat {
    Decimal,
    UpperRoman,
    LowerRoman,
    UpperLetter,
    LowerLetter,
    Bullet,
    None,
    Unknown(String),
}
pub struct NumberingLevel {
    pub source_id: NodeId,
    pub level: u8,
    pub start: Option<u32>,
    pub format: NumberFormat,
    pub text: Option<String>,
    pub suffix: Option<String>,
    pub left_indent_twips: Option<i32>,
    pub hanging_indent_twips: Option<i32>,
}
pub struct NumberingInstance {
    pub source_id: NodeId,
    pub num_id: NumberingId,
    pub abstract_num_id: AbstractNumberingId,
}
pub struct Numbering {
    source: SourceDocument,
    abstracts: HashMap<AbstractNumberingId, HashMap<u8, NumberingLevel>>,
    instances: HashMap<NumberingId, NumberingInstance>,
}

impl Numbering {
    pub fn parse(bytes: impl Into<std::sync::Arc<[u8]>>) -> Result<Self, NumberingError> {
        Self::from_source(SourceDocument::parse(bytes).map_err(NumberingError::Source)?)
    }
    pub fn from_source(source: SourceDocument) -> Result<Self, NumberingError> {
        if !word(&source, source.root(), "numbering") {
            return Err(NumberingError::InvalidNumberingRoot);
        }
        let mut abstracts = HashMap::new();
        let mut instances = HashMap::new();
        for id in source.children(source.root()) {
            if word(&source, id, "abstractNum") {
                let node = source.node(id).ok_or(NumberingError::MalformedNumbering)?;
                let abstract_id = AbstractNumberingId(attr_u32(node, "abstractNumId")?);
                let mut levels = HashMap::new();
                for level_id in source.children(id).filter(|x| word(&source, *x, "lvl")) {
                    let level = parse_level(&source, level_id)?;
                    if levels.insert(level.level, level).is_some() {
                        return Err(NumberingError::DuplicateId);
                    }
                }
                if abstracts.insert(abstract_id, levels).is_some() {
                    return Err(NumberingError::DuplicateId);
                }
            }
            if word(&source, id, "num") {
                let node = source.node(id).ok_or(NumberingError::MalformedNumbering)?;
                let num_id = NumberingId(attr_u32(node, "numId")?);
                let abstract_id = child(&source, id, "abstractNumId")
                    .and_then(|x| source.node(x))
                    .map(|x| attr_u32(x, "val"))
                    .transpose()?
                    .ok_or(NumberingError::MalformedNumbering)?;
                if instances
                    .insert(
                        num_id,
                        NumberingInstance {
                            source_id: id,
                            num_id,
                            abstract_num_id: AbstractNumberingId(abstract_id),
                        },
                    )
                    .is_some()
                {
                    return Err(NumberingError::DuplicateId);
                }
            }
        }
        Ok(Self {
            source,
            abstracts,
            instances,
        })
    }
    pub fn source(&self) -> &SourceDocument {
        &self.source
    }
    pub fn abstract_count(&self) -> usize {
        self.abstracts.len()
    }
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }
    pub fn instances(&self) -> impl Iterator<Item = &NumberingInstance> {
        self.instances.values()
    }
    pub fn resolve(&self, reference: ListReference) -> Result<&NumberingLevel, NumberingError> {
        let instance = self
            .instances
            .get(&reference.num_id)
            .ok_or(NumberingError::MissingNumberingInstance(reference.num_id))?;
        self.abstracts
            .get(&instance.abstract_num_id)
            .ok_or(NumberingError::MissingAbstractNumbering(
                instance.abstract_num_id,
            ))?
            .get(&reference.level)
            .ok_or(NumberingError::MissingLevel(reference.level))
    }
}
pub fn load_numbering(package: &Package, main: &Part) -> Result<Option<Numbering>, NumberingError> {
    let rels = match package.part_relationships(main) {
        Ok(x) => x,
        Err(PackageError::MissingPartRelationships(_)) => return Ok(None),
        Err(e) => return Err(NumberingError::Package(e)),
    };
    let Some(rel) = rels
        .into_iter()
        .find(|r| REL.contains(&r.relationship_type.as_str()))
    else {
        return Ok(None);
    };
    let opensuite_opc::RelationshipTarget::Internal { part_name, .. } = rel.target else {
        return Err(NumberingError::ExternalNumberingPart);
    };
    let part = package.part(&part_name).map_err(NumberingError::Package)?;
    Ok(Some(Numbering::parse(
        package.read_part(&part).map_err(NumberingError::Package)?,
    )?))
}
#[derive(Debug)]
pub enum NumberingError {
    Package(PackageError),
    Source(SourceError),
    InvalidNumberingRoot,
    MalformedNumbering,
    DuplicateId,
    MissingNumberingInstance(NumberingId),
    MissingAbstractNumbering(AbstractNumberingId),
    MissingLevel(u8),
    ExternalNumberingPart,
}
impl NumberingError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Package(e) => e.code(),
            Self::Source(e) => e.code(),
            Self::InvalidNumberingRoot => "INVALID_NUMBERING_ROOT",
            Self::MalformedNumbering => "MALFORMED_NUMBERING",
            Self::DuplicateId => "DUPLICATE_NUMBERING_ID",
            Self::MissingNumberingInstance(_) => "MISSING_NUMBERING_INSTANCE",
            Self::MissingAbstractNumbering(_) => "MISSING_ABSTRACT_NUMBERING",
            Self::MissingLevel(_) => "MISSING_NUMBERING_LEVEL",
            Self::ExternalNumberingPart => "EXTERNAL_NUMBERING_PART",
        }
    }
}
impl fmt::Display for NumberingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "numbering resolution failed: {}", self.code())
    }
}
impl std::error::Error for NumberingError {}
pub(crate) fn list_reference(
    source: &SourceDocument,
    ppr: NodeId,
) -> Result<Option<ListReference>, NumberingError> {
    let Some(numpr) = child(source, ppr, "numPr") else {
        return Ok(None);
    };
    let num = child(source, numpr, "numId")
        .and_then(|x| source.node(x))
        .map(|x| attr_u32(x, "val"))
        .transpose()?;
    let lvl = child(source, numpr, "ilvl")
        .and_then(|x| source.node(x))
        .map(|x| attr_u32(x, "val"))
        .transpose()?;
    let level = lvl
        .unwrap_or(0)
        .try_into()
        .map_err(|_| NumberingError::MalformedNumbering)?;
    Ok(num.map(|num_id| ListReference {
        num_id: NumberingId(num_id),
        level,
    }))
}
fn parse_level(source: &SourceDocument, id: NodeId) -> Result<NumberingLevel, NumberingError> {
    let node = source.node(id).ok_or(NumberingError::MalformedNumbering)?;
    let level = attr_u32(node, "ilvl")?
        .try_into()
        .map_err(|_| NumberingError::MalformedNumbering)?;
    let fmt = child(source, id, "numFmt")
        .and_then(|x| source.node(x))
        .and_then(|x| x.attribute("val"))
        .map(format)
        .unwrap_or(NumberFormat::Unknown(String::new()));
    let start = child(source, id, "start")
        .and_then(|x| source.node(x))
        .map(|x| attr_u32(x, "val"))
        .transpose()?;
    let text = child(source, id, "lvlText")
        .and_then(|x| source.node(x))
        .and_then(|x| x.attribute("val"))
        .map(str::to_owned);
    let suffix = child(source, id, "suff")
        .and_then(|x| source.node(x))
        .and_then(|x| x.attribute("val"))
        .map(str::to_owned);
    let ind = child(source, id, "pPr").and_then(|x| child(source, x, "ind"));
    let left_indent_twips = ind
        .and_then(|x| source.node(x))
        .and_then(|x| x.attribute("left"))
        .map(|x| x.parse().map_err(|_| NumberingError::MalformedNumbering))
        .transpose()?;
    let hanging_indent_twips = ind
        .and_then(|x| source.node(x))
        .and_then(|x| x.attribute("hanging"))
        .map(|x| x.parse().map_err(|_| NumberingError::MalformedNumbering))
        .transpose()?;
    Ok(NumberingLevel {
        source_id: id,
        level,
        start,
        format: fmt,
        text,
        suffix,
        left_indent_twips,
        hanging_indent_twips,
    })
}
fn format(v: &str) -> NumberFormat {
    match v {
        "decimal" => NumberFormat::Decimal,
        "upperRoman" => NumberFormat::UpperRoman,
        "lowerRoman" => NumberFormat::LowerRoman,
        "upperLetter" => NumberFormat::UpperLetter,
        "lowerLetter" => NumberFormat::LowerLetter,
        "bullet" => NumberFormat::Bullet,
        "none" => NumberFormat::None,
        x => NumberFormat::Unknown(x.to_owned()),
    }
}
fn attr_u32(n: &crate::SourceNode, name: &str) -> Result<u32, NumberingError> {
    n.attribute(name)
        .ok_or(NumberingError::MalformedNumbering)?
        .parse()
        .map_err(|_| NumberingError::MalformedNumbering)
}
fn child(s: &SourceDocument, p: NodeId, n: &str) -> Option<NodeId> {
    s.children(p).find(|id| word(s, *id, n))
}
fn word(s: &SourceDocument, id: NodeId, n: &str) -> bool {
    matches!(s.node(id).map(|x|x.kind()),Some(SourceNodeKind::Element{name,..}) if name.local_name()==n&&name.namespace_uri().is_some_and(|u|NS.contains(&u)))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{DocxDocument, StyleSheet};

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    #[test]
    fn resolves_numbering_and_prefers_direct_paragraph_references() {
        let numbering = Numbering::parse(format!(
            "<n:numbering xmlns:n=\"{WORD}\"><n:abstractNum n:abstractNumId=\"1\"><n:lvl n:ilvl=\"0\"><n:start n:val=\"1\"/><n:numFmt n:val=\"decimal\"/><n:lvlText n:val=\"%1.\"/><n:suff n:val=\"space\"/><n:pPr><n:ind n:left=\"720\" n:hanging=\"360\"/></n:pPr></n:lvl><n:lvl n:ilvl=\"1\"><n:numFmt n:val=\"bullet\"/><n:lvlText n:val=\"•\"/></n:lvl></n:abstractNum><n:num n:numId=\"4\"><n:abstractNumId n:val=\"1\"/></n:num><n:num n:numId=\"5\"><n:abstractNumId n:val=\"1\"/></n:num></n:numbering>"
        ).into_bytes()).unwrap();
        let styles = StyleSheet::parse(format!(
            "<s:styles xmlns:s=\"{WORD}\"><s:style s:type=\"paragraph\" s:styleId=\"List\"><s:pPr><s:numPr><s:numId s:val=\"4\"/></s:numPr></s:pPr></s:style></s:styles>"
        ).into_bytes()).unwrap();
        let source = SourceDocument::parse(format!(
            "<d:document xmlns:d=\"{WORD}\"><d:body><d:p><d:pPr><d:pStyle d:val=\"List\"/></d:pPr><d:r><d:t>Style</d:t></d:r></d:p><d:p><d:pPr><d:pStyle d:val=\"List\"/><d:numPr><d:numId d:val=\"5\"/><d:ilvl d:val=\"1\"/></d:numPr></d:pPr><d:r><d:t>Direct</d:t></d:r></d:p></d:body></d:document>"
        ).into_bytes()).unwrap();
        let document = DocxDocument::new(&source).unwrap();
        let paragraphs: Vec<_> = document.paragraphs().collect();

        assert_eq!(
            paragraphs[0].list_reference(&styles).unwrap(),
            Some(ListReference {
                num_id: NumberingId(4),
                level: 0
            })
        );
        let reference = paragraphs[1].list_reference(&styles).unwrap().unwrap();
        assert_eq!(
            reference,
            ListReference {
                num_id: NumberingId(5),
                level: 1
            }
        );
        let level = numbering.resolve(reference).unwrap();
        assert_eq!(level.format, NumberFormat::Bullet);
        assert_eq!(level.text.as_deref(), Some("•"));
        assert_eq!(level.left_indent_twips, None);
        assert!(numbering.source().node(level.source_id).is_some());
        assert!(
            numbering
                .instances()
                .all(|instance| numbering.source().node(instance.source_id).is_some())
        );
        let decimal = numbering
            .resolve(paragraphs[0].list_reference(&styles).unwrap().unwrap())
            .unwrap();
        assert_eq!(decimal.format, NumberFormat::Decimal);
        assert_eq!(decimal.left_indent_twips, Some(720));
    }

    #[test]
    fn discovers_numbering_from_relationships_and_reports_missing_parts() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/numbering.docx");
        let package = Package::open(fixture).unwrap();
        let main = package.main_office_document().unwrap();
        let numbering = load_numbering(&package, &main).unwrap().unwrap();
        assert_eq!(main.name.as_str(), "/custom/main.xml");
        assert_eq!(numbering.abstract_count(), 1);
        assert_eq!(numbering.instance_count(), 2);

        let no_numbering =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/semantic.docx");
        let package = Package::open(no_numbering).unwrap();
        let main = package.main_office_document().unwrap();
        assert!(load_numbering(&package, &main).unwrap().is_none());
    }

    #[test]
    fn returns_typed_errors_for_missing_abstracts_and_levels() {
        let missing_abstract = Numbering::parse(format!(
            "<numbering xmlns=\"{WORD}\"><num numId=\"4\"><abstractNumId val=\"9\"/></num></numbering>"
        ).into_bytes()).unwrap();
        assert!(matches!(
            missing_abstract.resolve(ListReference {
                num_id: NumberingId(4),
                level: 0
            }),
            Err(NumberingError::MissingAbstractNumbering(
                AbstractNumberingId(9)
            ))
        ));

        let missing_level = Numbering::parse(format!(
            "<numbering xmlns=\"{WORD}\"><abstractNum abstractNumId=\"1\"><lvl ilvl=\"0\"><numFmt val=\"decimal\"/></lvl></abstractNum><num numId=\"4\"><abstractNumId val=\"1\"/></num></numbering>"
        ).into_bytes()).unwrap();
        assert!(matches!(
            missing_level.resolve(ListReference {
                num_id: NumberingId(4),
                level: 1
            }),
            Err(NumberingError::MissingLevel(1))
        ));
    }
}
