//! Source-aware DOCX infrastructure built on OPC package discovery.

mod numbering;
mod section;
mod semantic;
mod source;
mod styles;

pub use numbering::{
    AbstractNumberingId, ListReference, NumberFormat, Numbering, NumberingError, NumberingId,
    NumberingInstance, NumberingLevel, load_numbering,
};
pub use section::{
    Columns, PageMargins, PageOrientation, PageSize, Section, SectionError, SectionProperties,
    SectionType,
};
pub use semantic::{
    BodyBlock, Cell, DocxDocument, Paragraph, Row, Run, SemanticError, Table, Text,
};
pub use source::{
    NodeId, SourceAttribute, SourceDocument, SourceError, SourceNode, SourceNodeKind, SourceSpan,
    XmlName,
};
pub use styles::{
    EffectiveParagraphFormatting, EffectiveRunFormatting, LineSpacing, LineSpacingRule,
    ParagraphAlignment, ParagraphFormatting, RunFormatting, Style, StyleError, StyleId, StyleSheet,
    StyleType, load_styles,
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
