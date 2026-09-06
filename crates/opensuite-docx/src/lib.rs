//! Source-aware DOCX infrastructure built on OPC package discovery.

mod comment;
mod content_control;
mod execution;
mod field;
mod header_footer;
mod inspection;
mod mutation;
mod numbering;
mod picture;
mod references;
mod section;
mod semantic;
mod source;
mod styles;
mod text_context;
mod text_search;
mod tracked_change;

pub use execution::{
    DocxExecutionResult, execute_docx_insert_table_column, execute_docx_insert_table_row,
    execute_docx_insert_table_rows, execute_docx_replace_text, execute_docx_set_table_cells_text,
    find_docx_text, inspect_docx, inspect_docx_context,
};
pub use field::{Field, FieldError, FieldKind, FieldSet, FieldState};
pub use header_footer::{
    HeaderFooter, HeaderFooterError, HeaderFooterKind, HeaderFooterReference, HeaderFooterType,
    load_footer, load_header,
};
pub use inspection::inspect_docx_document;
pub use mutation::{
    delete_paragraph, insert_paragraph_after, insert_table_column_after_to_vec,
    insert_table_row_after_to_vec, insert_table_rows_after_to_vec, replace_picture, replace_text,
    replace_text_to_vec, set_content_control_text, set_paragraph_formatting, set_paragraph_style,
    set_table_cell_text, set_table_cells_text_to_vec, set_text_formatting,
};
pub use numbering::{
    AbstractNumberingId, ListReference, NumberFormat, Numbering, NumberingError, NumberingId,
    NumberingInstance, NumberingLevel, load_numbering,
};
pub use picture::{
    ImagePart, ImageReference, Picture, PictureError, PictureExtent, PictureKind, PictureMetadata,
};
pub use references::{Bookmark, BookmarkId, Hyperlink, HyperlinkTarget, ReferenceError};
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
pub use text_context::inspect_text_context;
pub use text_search::find_text;
pub use tracked_change::{RevisionView, TrackedChange, TrackedChangeKind, TrackedChangeMetadata};

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
pub use comment::{Comment, CommentError, CommentIssue, CommentMetadata, CommentSet};
pub use content_control::{
    ContentControl, ContentControlKind, ContentControlListItem, ContentControlProperties,
    DataBinding, DateMetadata,
};
