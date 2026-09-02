use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use opensuite_opc::{Package, PackageError, Part};

use crate::{NodeId, SourceDocument, SourceError, SourceNodeKind};

const WORDPROCESSINGML_NAMESPACES: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
const STYLES_RELATIONSHIPS: [&str; 2] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles",
    "http://purl.oclc.org/ooxml/officeDocument/relationships/styles",
];

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StyleId(String);

impl StyleId {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StyleType {
    Paragraph,
    Character,
}

/// Formatting values where `None` means the property was not specified.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RunFormatting {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub font_size_half_points: Option<u16>,
    pub font_family: Option<String>,
}

impl RunFormatting {
    fn apply(&mut self, other: &Self) {
        if other.bold.is_some() {
            self.bold = other.bold;
        }
        if other.italic.is_some() {
            self.italic = other.italic;
        }
        if other.font_size_half_points.is_some() {
            self.font_size_half_points = other.font_size_half_points;
        }
        if other.font_family.is_some() {
            self.font_family = other.font_family.clone();
        }
    }

    pub fn font_size_points(&self) -> Option<f32> {
        self.font_size_half_points
            .map(|value| f32::from(value) / 2.0)
    }
}

pub type EffectiveRunFormatting = RunFormatting;

pub struct Style {
    source_id: NodeId,
    id: StyleId,
    style_type: StyleType,
    based_on: Option<StyleId>,
    formatting: RunFormatting,
}

impl Style {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }
    pub fn id(&self) -> &StyleId {
        &self.id
    }
    pub fn style_type(&self) -> StyleType {
        self.style_type
    }
    pub fn based_on(&self) -> Option<&StyleId> {
        self.based_on.as_ref()
    }
}

/// Read-only styles.xml model, loaded only on request.
pub struct StyleSheet {
    source: SourceDocument,
    run_defaults: RunFormatting,
    styles: HashMap<StyleId, Style>,
    default_paragraph_style: Option<StyleId>,
}

impl StyleSheet {
    pub fn parse(bytes: impl Into<std::sync::Arc<[u8]>>) -> Result<Self, StyleError> {
        Self::from_source(SourceDocument::parse(bytes).map_err(StyleError::Source)?)
    }

    pub fn from_source(source: SourceDocument) -> Result<Self, StyleError> {
        if !is_word_element(&source, source.root(), "styles") {
            return Err(StyleError::InvalidStylesRoot);
        }
        let run_defaults = source
            .children(source.root())
            .find(|id| is_word_element(&source, *id, "docDefaults"))
            .and_then(|id| child(&source, id, "rPrDefault"))
            .and_then(|id| child(&source, id, "rPr"))
            .map(|id| run_formatting(&source, id))
            .transpose()?
            .unwrap_or_default();
        let mut styles = HashMap::new();
        let mut default_paragraph_style = None;

        for source_id in source
            .children(source.root())
            .filter(|id| is_word_element(&source, *id, "style"))
        {
            let node = source.node(source_id).ok_or(StyleError::MalformedStyles)?;
            let id = StyleId::new(
                node.attribute("styleId")
                    .filter(|value| !value.is_empty())
                    .ok_or(StyleError::MalformedStyles)?
                    .to_owned(),
            );
            let style_type = match node.attribute("type") {
                Some("paragraph") => StyleType::Paragraph,
                Some("character") => StyleType::Character,
                _ => continue,
            };
            let based_on = child(&source, source_id, "basedOn")
                .and_then(|id| source.node(id))
                .and_then(|node| node.attribute("val"))
                .filter(|value| !value.is_empty())
                .map(|value| StyleId::new(value.to_owned()));
            let formatting = child(&source, source_id, "rPr")
                .map(|id| run_formatting(&source, id))
                .transpose()?
                .unwrap_or_default();
            if styles.contains_key(&id) {
                return Err(StyleError::DuplicateStyle(id));
            }
            if style_type == StyleType::Paragraph && is_enabled(node.attribute("default"))? {
                default_paragraph_style = Some(id.clone());
            }
            styles.insert(
                id.clone(),
                Style {
                    source_id,
                    id,
                    style_type,
                    based_on,
                    formatting,
                },
            );
        }
        Ok(Self {
            source,
            run_defaults,
            styles,
            default_paragraph_style,
        })
    }

    pub fn source(&self) -> &SourceDocument {
        &self.source
    }
    pub fn style_count(&self) -> usize {
        self.styles.len()
    }
    pub fn styles(&self) -> impl Iterator<Item = &Style> {
        self.styles.values()
    }
    pub fn default_paragraph_style_id(&self) -> Option<&StyleId> {
        self.default_paragraph_style.as_ref()
    }

    pub fn effective_run_formatting(
        &self,
        paragraph_style: Option<&StyleId>,
        character_style: Option<&StyleId>,
        direct: &RunFormatting,
    ) -> Result<EffectiveRunFormatting, StyleError> {
        let mut result = self.run_defaults.clone();
        let paragraph_style = paragraph_style.or(self.default_paragraph_style.as_ref());
        if let Some(style) = paragraph_style {
            result.apply(&self.style_formatting(style, StyleType::Paragraph)?);
        }
        if let Some(style) = character_style {
            result.apply(&self.style_formatting(style, StyleType::Character)?);
        }
        result.apply(direct);
        Ok(result)
    }

    fn style_formatting(
        &self,
        id: &StyleId,
        expected_type: StyleType,
    ) -> Result<RunFormatting, StyleError> {
        let mut result = RunFormatting::default();
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut current = Some(id);
        while let Some(current_id) = current {
            if !seen.insert(current_id.clone()) {
                return Err(StyleError::InheritanceCycle(current_id.clone()));
            }
            let style = self
                .styles
                .get(current_id)
                .ok_or_else(|| StyleError::MissingStyle(current_id.clone()))?;
            if style.style_type != expected_type {
                return Err(StyleError::StyleTypeMismatch(current_id.clone()));
            }
            chain.push(style);
            current = style.based_on.as_ref();
        }
        for style in chain.into_iter().rev() {
            result.apply(&style.formatting);
        }
        Ok(result)
    }
}

/// Loads styles.xml through the main part's OPC relationships.
pub fn load_styles(package: &Package, main_part: &Part) -> Result<Option<StyleSheet>, StyleError> {
    let relationships = match package.part_relationships(main_part) {
        Ok(relationships) => relationships,
        Err(PackageError::MissingPartRelationships(_)) => return Ok(None),
        Err(error) => return Err(StyleError::Package(error)),
    };
    let Some(relationship) = relationships.into_iter().find(|relationship| {
        STYLES_RELATIONSHIPS.contains(&relationship.relationship_type.as_str())
    }) else {
        return Ok(None);
    };
    let opensuite_opc::RelationshipTarget::Internal { part_name, .. } = relationship.target else {
        return Err(StyleError::ExternalStylesPart);
    };
    let part = package.part(&part_name).map_err(StyleError::Package)?;
    let bytes = package.read_part(&part).map_err(StyleError::Package)?;
    Ok(Some(StyleSheet::parse(bytes)?))
}

#[derive(Debug)]
pub enum StyleError {
    Package(PackageError),
    Source(SourceError),
    InvalidStylesRoot,
    MalformedStyles,
    DuplicateStyle(StyleId),
    InheritanceCycle(StyleId),
    MissingStyle(StyleId),
    StyleTypeMismatch(StyleId),
    InvalidFormattingValue,
    ExternalStylesPart,
}

impl StyleError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Package(error) => error.code(),
            Self::Source(error) => error.code(),
            Self::InvalidStylesRoot => "INVALID_STYLES_ROOT",
            Self::MalformedStyles => "MALFORMED_STYLES",
            Self::DuplicateStyle(_) => "DUPLICATE_STYLE_ID",
            Self::InheritanceCycle(_) => "STYLE_INHERITANCE_CYCLE",
            Self::MissingStyle(_) => "MISSING_STYLE",
            Self::StyleTypeMismatch(_) => "STYLE_TYPE_MISMATCH",
            Self::InvalidFormattingValue => "INVALID_FORMATTING_VALUE",
            Self::ExternalStylesPart => "EXTERNAL_STYLES_PART",
        }
    }
}
impl fmt::Display for StyleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Package(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::InvalidStylesRoot => write!(f, "root is not WordprocessingML styles"),
            Self::MalformedStyles => write!(f, "malformed styles part"),
            Self::DuplicateStyle(id) => write!(f, "duplicate style ID: {}", id.as_str()),
            Self::InheritanceCycle(id) => write!(f, "style inheritance cycle at: {}", id.as_str()),
            Self::MissingStyle(id) => write!(f, "missing style: {}", id.as_str()),
            Self::StyleTypeMismatch(id) => write!(f, "style type mismatch: {}", id.as_str()),
            Self::InvalidFormattingValue => write!(f, "invalid formatting value"),
            Self::ExternalStylesPart => write!(f, "styles relationship target is external"),
        }
    }
}
impl std::error::Error for StyleError {}

pub(crate) fn run_formatting(
    source: &SourceDocument,
    rpr: NodeId,
) -> Result<RunFormatting, StyleError> {
    let mut formatting = RunFormatting::default();
    for id in source.children(rpr) {
        let Some(node) = source.node(id) else {
            continue;
        };
        let SourceNodeKind::Element { name, .. } = node.kind() else {
            continue;
        };
        if !is_word_element(source, id, name.local_name()) {
            continue;
        }
        match name.local_name() {
            "b" => formatting.bold = Some(is_enabled(node.attribute("val"))?),
            "i" => formatting.italic = Some(is_enabled(node.attribute("val"))?),
            "sz" => {
                formatting.font_size_half_points = Some(
                    node.attribute("val")
                        .ok_or(StyleError::InvalidFormattingValue)?
                        .parse()
                        .map_err(|_| StyleError::InvalidFormattingValue)?,
                )
            }
            "rFonts" => {
                formatting.font_family = node
                    .attribute("ascii")
                    .or_else(|| node.attribute("hAnsi"))
                    .map(str::to_owned)
            }
            _ => {}
        }
    }
    Ok(formatting)
}

fn is_enabled(value: Option<&str>) -> Result<bool, StyleError> {
    match value.unwrap_or("true") {
        "true" | "1" | "on" => Ok(true),
        "false" | "0" | "off" => Ok(false),
        _ => Err(StyleError::InvalidFormattingValue),
    }
}
fn child(source: &SourceDocument, parent: NodeId, local_name: &str) -> Option<NodeId> {
    source
        .children(parent)
        .find(|id| is_word_element(source, *id, local_name))
}
fn is_word_element(source: &SourceDocument, id: NodeId, local_name: &str) -> bool {
    let Some(node) = source.node(id) else {
        return false;
    };
    let SourceNodeKind::Element { name, .. } = node.kind() else {
        return false;
    };
    name.local_name() == local_name
        && name
            .namespace_uri()
            .is_some_and(|namespace| WORDPROCESSINGML_NAMESPACES.contains(&namespace))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{DocxDocument, SourceDocument};

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    #[test]
    fn resolves_defaults_style_chains_and_direct_formatting() {
        let styles = StyleSheet::parse(format!(
            "<s:styles xmlns:s=\"{WORD}\"><s:docDefaults><s:rPrDefault><s:rPr><s:b/><s:sz s:val=\"20\"/><s:rFonts s:ascii=\"Default\"/></s:rPr></s:rPrDefault></s:docDefaults><s:style s:type=\"paragraph\" s:styleId=\"Base\" s:default=\"1\"><s:rPr><s:i/></s:rPr></s:style><s:style s:type=\"paragraph\" s:styleId=\"Body\"><s:basedOn s:val=\"Base\"/><s:rPr><s:b s:val=\"false\"/></s:rPr></s:style><s:style s:type=\"character\" s:styleId=\"Emphasis\"><s:rPr><s:i s:val=\"0\"/><s:sz s:val=\"22\"/><s:rFonts s:hAnsi=\"Character\"/></s:rPr></s:style></s:styles>"
        ).into_bytes()).unwrap();
        let source = SourceDocument::parse(format!(
            "<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:pPr><w:pStyle w:val=\"Body\"/></w:pPr><w:r><w:rPr><w:rStyle w:val=\"Emphasis\"/><w:b w:val=\"1\"/><w:sz w:val=\"24\"/><w:rFonts w:ascii=\"Direct\"/></w:rPr><w:t>Text</w:t></w:r></w:p></w:body></w:document>"
        ).into_bytes()).unwrap();
        let document = DocxDocument::new(&source).unwrap();
        let paragraph = document.paragraphs().next().unwrap();
        let run = paragraph.runs().next().unwrap();
        let formatting = run.effective_formatting(&styles).unwrap();

        assert_eq!(paragraph.style_id().unwrap().as_str(), "Body");
        assert_eq!(run.character_style_id().unwrap().as_str(), "Emphasis");
        assert_eq!(formatting.bold, Some(true));
        assert_eq!(formatting.italic, Some(false));
        assert_eq!(formatting.font_size_half_points, Some(24));
        assert_eq!(formatting.font_size_points(), Some(12.0));
        assert_eq!(formatting.font_family.as_deref(), Some("Direct"));
    }

    #[test]
    fn reports_inheritance_cycles() {
        let styles = StyleSheet::parse(format!(
            "<styles xmlns=\"{WORD}\"><style type=\"paragraph\" styleId=\"A\"><basedOn val=\"B\"/></style><style type=\"paragraph\" styleId=\"B\"><basedOn val=\"A\"/></style></styles>"
        ).into_bytes()).unwrap();
        let id = StyleId::new("A".to_owned());

        assert!(matches!(
            styles.effective_run_formatting(Some(&id), None, &RunFormatting::default()),
            Err(StyleError::InheritanceCycle(_))
        ));
    }

    #[test]
    fn discovers_styles_through_main_part_relationships_and_handles_absence() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/styles.docx");
        let package = Package::open(fixture).unwrap();
        let main = package.main_office_document().unwrap();
        let styles = load_styles(&package, &main).unwrap().unwrap();
        assert_eq!(main.name.as_str(), "/custom/main.xml");
        assert_eq!(styles.style_count(), 1);

        let no_styles =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/semantic.docx");
        let package = Package::open(no_styles).unwrap();
        let main = package.main_office_document().unwrap();
        assert!(load_styles(&package, &main).unwrap().is_none());
    }
}
