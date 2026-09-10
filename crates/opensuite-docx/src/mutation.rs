use std::{collections::HashSet, path::Path};

use quick_xml::escape::escape;

use opensuite_opc::{Package, PackageError, Part, PartName, RelationshipTarget};
use opensuite_protocol::{
    AffordanceReason, ContentControlTarget, CreateTable, DeletePageBreak, DeleteParagraph,
    DeletePicture, DeleteTable, DeleteTableColumn, DeleteTableRow, HeaderFooterKind,
    InsertPageBreak, InsertParagraph, InsertParagraphAfter, InsertParagraphs, InsertPicture,
    InsertTableColumnAfter, InsertTableRowAfter, InsertTableRowsAfter, OperationResult,
    PageBreakTarget, PageMargins, PageNumberAlignment, PageOrientation, PaperSize,
    ParagraphFormattingPatch, ParagraphListKind, ParagraphPlacement, PropertyPatch, ReplacePicture,
    ReplaceText, SetContentControlText, SetHeaderFooterText, SetHyperlink, SetPageNumber,
    SetPageSetup, SetParagraphFormatting, SetParagraphStyle, SetParagraphsList, SetPictureSize,
    SetTableCellText, SetTableCellsText, SetTableFormatting, SetTextFormatting, TableAlignment,
    TableBorders, TableCellMargins, TableCellTarget, TableFormattingPatch, TableRowTarget,
    TableTarget, TextFormattingPatch, TextTarget,
};

use crate::{NodeId, RevisionView, SemanticError, SourceDocument, SourceNodeKind, SourceSpan};

pub(super) const NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
pub(super) const MAX_TABLE_ROWS_PER_OPERATION: usize = 100;
pub(super) const MAX_TABLE_CELL_UPDATES: usize = 100;
pub(super) const MAX_PARAGRAPHS_PER_OPERATION: usize = 100;
pub(super) const NUMBERING_RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
pub(super) const NUMBERING_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
pub(super) const HYPERLINK_RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
pub(super) const HEADER_RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
pub(super) const FOOTER_RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
pub(super) const HEADER_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
pub(super) const FOOTER_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
pub(super) const LETTER: (u32, u32) = (12_240, 15_840);
pub(super) const A4: (u32, u32) = (11_906, 16_838);
pub(super) const DEFAULT_MARGIN_TWIPS: i32 = 1_440;
pub(super) const MAX_MARGIN_TWIPS: i32 = 31_680;

enum ResolvedParagraphPlacement {
    Start,
    End,
    Before(NodeId),
    After(NodeId),
}

mod common;
mod content_control;
mod formatting;
mod header_footer;
mod hyperlink;
mod list;
mod page_break;
mod page_number;
mod page_setup;
mod paragraph;
mod picture;
mod table;
mod text;

use common::*;
use formatting::*;
use header_footer::*;
use page_break::*;
use page_number::*;
use page_setup::*;
use paragraph::*;
use picture::*;
use table::*;
use text::*;

pub use common::*;
pub use content_control::*;
pub use formatting::*;
pub use header_footer::*;
pub use hyperlink::*;
pub use list::*;
pub use page_break::*;
pub use page_number::*;
pub use page_setup::*;
pub use paragraph::*;
pub use picture::*;
pub use table::*;
pub use text::*;

#[cfg(test)]
mod tests;
