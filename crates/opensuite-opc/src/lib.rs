//! Office Open XML package discovery.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use quick_xml::{Reader, events::Event};
use zip::ZipArchive;

pub const LAYER: &str = "opc";

const CONTENT_TYPES_ENTRY: &str = "[Content_Types].xml";
const PACKAGE_RELATIONSHIPS_ENTRY: &str = "_rels/.rels";
const OFFICE_DOCUMENT_RELATIONSHIPS: [&str; 2] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
    "http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument",
];

/// An OPC package name such as `/word/document.xml`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PartName(String);

impl PartName {
    pub fn parse(value: impl Into<String>) -> Result<Self, PackageError> {
        let value = value.into();
        if !value.starts_with('/') || value == "/" || value.contains(['\\', '?', '#']) {
            return Err(PackageError::InvalidPartName(value));
        }
        validate_path(&value[1..])?;
        Ok(Self(value))
    }

    fn from_entry(entry: &str) -> Result<Self, PackageError> {
        validate_path(entry)?;
        Self::parse(format!("/{entry}"))
    }

    fn resolve_root_target(target: &str) -> Result<Self, PackageError> {
        if target.is_empty() || target.starts_with('/') || target.contains(['\\', '?', '#']) {
            return Err(PackageError::InvalidRelationshipTarget(target.to_owned()));
        }

        let mut segments = Vec::new();
        for segment in target.split('/') {
            match segment {
                "" => return Err(PackageError::InvalidRelationshipTarget(target.to_owned())),
                "." => {}
                ".." => {
                    if segments.pop().is_none() {
                        return Err(PackageError::InvalidRelationshipTarget(target.to_owned()));
                    }
                }
                value => segments.push(value),
            }
        }

        Self::parse(format!("/{}", segments.join("/")))
            .map_err(|_| PackageError::InvalidRelationshipTarget(target.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PartName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// An OPC content type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentType(String);

impl ContentType {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A relationship identifier from a `.rels` part.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipId(String);

impl RelationshipId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A relationship type URI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationshipType(String);

impl RelationshipType {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A relationship target, kept separate from the resolved package part.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationshipTarget {
    Internal {
        original: String,
        part_name: PartName,
    },
    External {
        original: String,
    },
}

/// A package-level OPC relationship.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relationship {
    pub id: RelationshipId,
    pub relationship_type: RelationshipType,
    pub target: RelationshipTarget,
}

/// A discovered OPC package part.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Part {
    pub name: PartName,
    pub content_type: ContentType,
}

/// Typed failures while reading OPC package metadata.
#[derive(Debug)]
pub enum PackageError {
    Io(io::Error),
    InvalidZip(zip::result::ZipError),
    UnsafeEntry(String),
    InvalidPartName(String),
    MissingContentTypes,
    MalformedContentTypes,
    MissingPackageRelationships,
    MalformedRelationships,
    MissingOfficeDocument,
    ExternalOfficeDocument,
    InvalidRelationshipTarget(String),
    MissingTargetPart(PartName),
    MissingContentType(PartName),
}

impl PackageError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "IO_ERROR",
            Self::InvalidZip(_) => "INVALID_ZIP",
            Self::UnsafeEntry(_) => "UNSAFE_ENTRY",
            Self::InvalidPartName(_) => "INVALID_PART_NAME",
            Self::MissingContentTypes => "MISSING_CONTENT_TYPES",
            Self::MalformedContentTypes => "MALFORMED_CONTENT_TYPES",
            Self::MissingPackageRelationships => "MISSING_PACKAGE_RELATIONSHIPS",
            Self::MalformedRelationships => "MALFORMED_RELATIONSHIPS",
            Self::MissingOfficeDocument => "MISSING_OFFICE_DOCUMENT",
            Self::ExternalOfficeDocument => "EXTERNAL_OFFICE_DOCUMENT",
            Self::InvalidRelationshipTarget(_) => "INVALID_RELATIONSHIP_TARGET",
            Self::MissingTargetPart(_) => "MISSING_TARGET_PART",
            Self::MissingContentType(_) => "MISSING_CONTENT_TYPE",
        }
    }
}

impl fmt::Display for PackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "could not read package: {error}"),
            Self::InvalidZip(error) => write!(formatter, "invalid ZIP package: {error}"),
            Self::UnsafeEntry(entry) => write!(formatter, "unsafe package entry: {entry}"),
            Self::InvalidPartName(name) => write!(formatter, "invalid part name: {name}"),
            Self::MissingContentTypes => write!(formatter, "missing [Content_Types].xml"),
            Self::MalformedContentTypes => write!(formatter, "malformed [Content_Types].xml"),
            Self::MissingPackageRelationships => write!(formatter, "missing _rels/.rels"),
            Self::MalformedRelationships => write!(formatter, "malformed package relationships"),
            Self::MissingOfficeDocument => write!(
                formatter,
                "package does not contain an officeDocument relationship"
            ),
            Self::ExternalOfficeDocument => {
                write!(formatter, "officeDocument relationship target is external")
            }
            Self::InvalidRelationshipTarget(target) => {
                write!(formatter, "invalid relationship target: {target}")
            }
            Self::MissingTargetPart(part) => {
                write!(formatter, "relationship target part does not exist: {part}")
            }
            Self::MissingContentType(part) => {
                write!(formatter, "content type cannot be resolved: {part}")
            }
        }
    }
}

impl std::error::Error for PackageError {}

/// Read-only OPC package metadata. ZIP entry storage remains private.
pub struct Package {
    path: PathBuf,
    entry_count: usize,
    parts: HashSet<PartName>,
    content_types: ContentTypes,
    relationships: Vec<Relationship>,
}

impl Package {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PackageError> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(PackageError::Io)?;
        let mut archive = ZipArchive::new(file).map_err(PackageError::InvalidZip)?;
        let entry_count = archive.len();
        let mut entries = HashSet::new();

        for index in 0..entry_count {
            let entry = archive.by_index(index).map_err(PackageError::InvalidZip)?;
            if !entry.is_dir() {
                entries.insert(PartName::from_entry(entry.name())?);
            }
        }

        let content_types = parse_content_types(read_entry(&mut archive, CONTENT_TYPES_ENTRY)?)?;
        let relationships =
            parse_relationships(read_entry(&mut archive, PACKAGE_RELATIONSHIPS_ENTRY)?)?;

        Ok(Self {
            path,
            entry_count,
            parts: entries,
            content_types,
            relationships,
        })
    }

    pub fn entry_count(&self) -> usize {
        self.entry_count
    }

    pub fn part_count(&self) -> usize {
        self.parts
            .iter()
            .filter(|part| !is_metadata_part(part))
            .count()
    }

    pub fn package_relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    pub fn content_type(&self, part_name: &PartName) -> Result<ContentType, PackageError> {
        self.content_types.resolve(part_name)
    }

    /// Reads one known package part without reading other ZIP entries.
    pub fn read_part(&self, part: &Part) -> Result<Vec<u8>, PackageError> {
        if !self.parts.contains(&part.name) {
            return Err(PackageError::MissingTargetPart(part.name.clone()));
        }
        let file = File::open(&self.path).map_err(PackageError::Io)?;
        let mut archive = ZipArchive::new(file).map_err(PackageError::InvalidZip)?;
        read_entry(&mut archive, part.name.as_str().trim_start_matches('/'))
    }

    pub fn main_office_document(&self) -> Result<Part, PackageError> {
        let relationship = self
            .relationships
            .iter()
            .find(|relationship| {
                OFFICE_DOCUMENT_RELATIONSHIPS.contains(&relationship.relationship_type.as_str())
            })
            .ok_or(PackageError::MissingOfficeDocument)?;

        let part_name = match &relationship.target {
            RelationshipTarget::Internal { part_name, .. } => part_name,
            RelationshipTarget::External { .. } => {
                return Err(PackageError::ExternalOfficeDocument);
            }
        };
        if !self.parts.contains(part_name) {
            return Err(PackageError::MissingTargetPart(part_name.clone()));
        }

        Ok(Part {
            name: part_name.clone(),
            content_type: self.content_type(part_name)?,
        })
    }
}

#[derive(Default)]
struct ContentTypes {
    defaults: HashMap<String, ContentType>,
    overrides: HashMap<PartName, ContentType>,
}

impl ContentTypes {
    fn resolve(&self, part_name: &PartName) -> Result<ContentType, PackageError> {
        if let Some(content_type) = self.overrides.get(part_name) {
            return Ok(content_type.clone());
        }

        let extension = part_name
            .as_str()
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
            .filter(|extension| !extension.is_empty());
        extension
            .and_then(|extension| self.defaults.get(&extension))
            .cloned()
            .ok_or_else(|| PackageError::MissingContentType(part_name.clone()))
    }
}

fn validate_path(path: &str) -> Result<(), PackageError> {
    if path.is_empty() || path.contains(['\\', '?', '#']) {
        return Err(PackageError::UnsafeEntry(path.to_owned()));
    }
    if path
        .split('/')
        .any(|segment| matches!(segment, "" | "." | ".."))
    {
        return Err(PackageError::UnsafeEntry(path.to_owned()));
    }
    Ok(())
}

fn read_entry(archive: &mut ZipArchive<File>, name: &str) -> Result<Vec<u8>, PackageError> {
    let mut entry = archive.by_name(name).map_err(|error| match error {
        zip::result::ZipError::FileNotFound => {
            if name == CONTENT_TYPES_ENTRY {
                PackageError::MissingContentTypes
            } else {
                PackageError::MissingPackageRelationships
            }
        }
        other => PackageError::InvalidZip(other),
    })?;
    let mut content = Vec::new();
    entry.read_to_end(&mut content).map_err(PackageError::Io)?;
    Ok(content)
}

fn parse_content_types(content: Vec<u8>) -> Result<ContentTypes, PackageError> {
    let mut reader = Reader::from_reader(content.as_slice());
    reader.config_mut().trim_text(true);
    let mut result = ContentTypes::default();
    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(element)) if local_name(element.name().as_ref()) == b"Default" => {
                let extension = attribute(&reader, &element, b"Extension")?
                    .ok_or(PackageError::MalformedContentTypes)?
                    .to_ascii_lowercase();
                let content_type = attribute(&reader, &element, b"ContentType")?
                    .filter(|value| !value.is_empty())
                    .ok_or(PackageError::MalformedContentTypes)?;
                if extension.is_empty() {
                    return Err(PackageError::MalformedContentTypes);
                }
                result.defaults.insert(extension, ContentType(content_type));
            }
            Ok(Event::Empty(element)) if local_name(element.name().as_ref()) == b"Override" => {
                let part_name = attribute(&reader, &element, b"PartName")?
                    .ok_or(PackageError::MalformedContentTypes)
                    .and_then(|value| {
                        PartName::parse(value).map_err(|_| PackageError::MalformedContentTypes)
                    })?;
                let content_type = attribute(&reader, &element, b"ContentType")?
                    .filter(|value| !value.is_empty())
                    .ok_or(PackageError::MalformedContentTypes)?;
                result
                    .overrides
                    .insert(part_name, ContentType(content_type));
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(PackageError::MalformedContentTypes),
            _ => {}
        }
        buffer.clear();
    }
    Ok(result)
}

fn parse_relationships(content: Vec<u8>) -> Result<Vec<Relationship>, PackageError> {
    let mut reader = Reader::from_reader(content.as_slice());
    reader.config_mut().trim_text(true);
    let mut relationships = Vec::new();
    let mut buffer = Vec::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Empty(element)) if local_name(element.name().as_ref()) == b"Relationship" => {
                let id = required_attribute(
                    &reader,
                    &element,
                    b"Id",
                    PackageError::MalformedRelationships,
                )?;
                let relationship_type = required_attribute(
                    &reader,
                    &element,
                    b"Type",
                    PackageError::MalformedRelationships,
                )?;
                let original = required_attribute(
                    &reader,
                    &element,
                    b"Target",
                    PackageError::MalformedRelationships,
                )?;
                let target_mode = attribute(&reader, &element, b"TargetMode")?;
                let target = match target_mode.as_deref() {
                    Some("External") => RelationshipTarget::External { original },
                    None | Some("Internal") => RelationshipTarget::Internal {
                        part_name: PartName::resolve_root_target(&original)?,
                        original,
                    },
                    Some(_) => return Err(PackageError::MalformedRelationships),
                };
                relationships.push(Relationship {
                    id: RelationshipId(id),
                    relationship_type: RelationshipType(relationship_type),
                    target,
                });
            }
            Ok(Event::Eof) => break,
            Err(_) => return Err(PackageError::MalformedRelationships),
            _ => {}
        }
        buffer.clear();
    }
    Ok(relationships)
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn attribute(
    reader: &Reader<&[u8]>,
    element: &quick_xml::events::BytesStart<'_>,
    name: &[u8],
) -> Result<Option<String>, PackageError> {
    for attribute in element.attributes().with_checks(false) {
        let attribute = attribute.map_err(|_| PackageError::MalformedRelationships)?;
        if local_name(attribute.key.as_ref()) == name {
            return attribute
                .decode_and_unescape_value(reader.decoder())
                .map(|value| Some(value.into_owned()))
                .map_err(|_| PackageError::MalformedRelationships);
        }
    }
    Ok(None)
}

fn required_attribute(
    reader: &Reader<&[u8]>,
    element: &quick_xml::events::BytesStart<'_>,
    name: &[u8],
    error: PackageError,
) -> Result<String, PackageError> {
    attribute(reader, element, name)?
        .filter(|value| !value.is_empty())
        .ok_or(error)
}

fn is_metadata_part(part: &PartName) -> bool {
    let name = part.as_str();
    name == "/[Content_Types].xml"
        || name == "/_rels/.rels"
        || (name.contains("/_rels/") && name.ends_with(".rels"))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Write,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use zip::{ZipWriter, write::SimpleFileOptions};

    const TRANSITIONAL: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
    static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

    fn package_file(
        content_types: &str,
        relationships: &str,
        parts: &[(&str, &str)],
    ) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "opensuite-opc-{}-{}.docx",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let file = File::create(&path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        zip.start_file(CONTENT_TYPES_ENTRY, options).unwrap();
        zip.write_all(content_types.as_bytes()).unwrap();
        zip.start_file(PACKAGE_RELATIONSHIPS_ENTRY, options)
            .unwrap();
        zip.write_all(relationships.as_bytes()).unwrap();
        for (name, body) in parts {
            zip.start_file(*name, options).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    fn content_types(override_part: &str) -> String {
        format!(
            "<Types><Default Extension=\"xml\" ContentType=\"application/default+xml\"/><Override PartName=\"{override_part}\" ContentType=\"application/custom-main+xml\"/></Types>"
        )
    }

    fn relationships(target: &str) -> String {
        format!(
            "<Relationships><Relationship Id=\"rId1\" Type=\"{TRANSITIONAL}\" Target=\"{target}\"/></Relationships>"
        )
    }

    #[test]
    fn opens_a_package_and_resolves_a_nonstandard_main_part() {
        let path = package_file(
            &content_types("/custom/main.xml"),
            &relationships("custom/main.xml"),
            &[("custom/main.xml", "<document/>")],
        );
        let package = Package::open(&path).unwrap();
        fs::remove_file(path).unwrap();

        let part = package.main_office_document().unwrap();
        assert_eq!(package.entry_count(), 3);
        assert_eq!(package.part_count(), 1);
        assert_eq!(part.name.as_str(), "/custom/main.xml");
        assert_eq!(part.content_type.as_str(), "application/custom-main+xml");
    }

    #[test]
    fn uses_default_content_types_when_no_override_exists() {
        let path = package_file(
            "<Types><Default Extension=\"xml\" ContentType=\"application/default+xml\"/></Types>",
            &relationships("custom/main.xml"),
            &[("custom/main.xml", "<document/>")],
        );
        let package = Package::open(&path).unwrap();
        fs::remove_file(path).unwrap();

        assert_eq!(
            package
                .main_office_document()
                .unwrap()
                .content_type
                .as_str(),
            "application/default+xml"
        );
    }

    #[test]
    fn reports_a_missing_office_document_relationship() {
        let path = package_file("<Types/>", "<Relationships/>", &[]);
        let package = Package::open(&path).unwrap();
        fs::remove_file(path).unwrap();

        assert!(matches!(
            package.main_office_document(),
            Err(PackageError::MissingOfficeDocument)
        ));
    }

    #[test]
    fn reports_a_missing_relationship_target_part() {
        let path = package_file(
            &content_types("/missing.xml"),
            &relationships("missing.xml"),
            &[],
        );
        let package = Package::open(&path).unwrap();
        fs::remove_file(path).unwrap();

        assert!(matches!(
            package.main_office_document(),
            Err(PackageError::MissingTargetPart(_))
        ));
    }

    #[test]
    fn rejects_non_zip_input() {
        let path = std::env::temp_dir().join(format!(
            "opensuite-opc-invalid-{}-{}.docx",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, "not a zip").unwrap();
        let result = Package::open(&path);
        fs::remove_file(path).unwrap();

        assert!(matches!(result, Err(PackageError::InvalidZip(_))));
    }

    #[test]
    fn resolves_the_strict_office_document_relationship() {
        let strict = "http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument";
        let path = package_file(
            &content_types("/custom/main.xml"),
            &format!(
                "<Relationships><Relationship Id=\"rId1\" Type=\"{strict}\" Target=\"custom/main.xml\"/></Relationships>"
            ),
            &[("custom/main.xml", "<document/>")],
        );
        let package = Package::open(&path).unwrap();
        fs::remove_file(path).unwrap();

        assert_eq!(
            package.main_office_document().unwrap().name.as_str(),
            "/custom/main.xml"
        );
    }
}
