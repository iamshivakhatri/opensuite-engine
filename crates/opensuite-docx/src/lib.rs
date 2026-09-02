//! Source-aware DOCX infrastructure built on OPC package discovery.

mod source;

pub use source::{
    NodeId, SourceDocument, SourceError, SourceNode, SourceNodeKind, SourceSpan, XmlName,
};

use opensuite_opc::{Package, Part};

pub const LAYER: &str = "docx";
pub const PACKAGE_LAYER: &str = opensuite_opc::LAYER;

/// Loads and indexes the OPC-discovered main XML part without interpreting DOCX semantics.
pub fn open_main_source(package: &Package) -> Result<(Part, SourceDocument), SourceError> {
    let part = package
        .main_office_document()
        .map_err(SourceError::Package)?;
    let bytes = package.read_part(&part).map_err(SourceError::Package)?;
    let source = SourceDocument::parse(bytes)?;
    Ok((part, source))
}
