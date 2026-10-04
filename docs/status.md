# OpenSuite Engine Status

## Current Scope

OpenSuite is a preservation-first DOCX engine. The Rust workspace contains the
OPC package layer, DOCX engine, stable protocol types, native CLI, and Node
N-API adapter. PPTX currently has package validation, slide/shape inspection,
paragraph text search, safe direct text and picture replacement, top-level
picture insertion, direct shape geometry, and direct run formatting; XLSX has
not started.

## Current DOCX Engine

- Opens DOCX packages from files or owned bytes, discovers the main document,
  resolves content types and relationships, and writes verified in-memory or
  file output while retaining untouched payloads.
- Inspects semantic document content: text, headings, body blocks, tables,
  styles, numbering, sections, headers/footers, references, pictures, fields,
  content controls, comments, and tracked-change views. Exact text search and
  bounded context inspection use source order and opaque local handles.
- Produces a bounded DOCX style snapshot with declared, direct, and resolved
  typography; paragraph and list patterns; table appearance; section setup;
  header/footer facts; and unresolved theme references.
- Creates and patches real paragraph and character Word styles with explicit IDs,
  validated inheritance/next targets, and source-local preservation. Default style
  selection and deletion remain unavailable.
- Creates blank DOCX files and inserts or deletes safe direct-body paragraphs.
  It replaces text, assigns existing paragraph styles, and applies supported
  direct paragraph and text formatting, including color, underline, highlight,
  strikethrough, and superscript/subscript.
- Creates, deletes, and safely edits simple direct-body tables, including rows,
  columns, cells, explicit column widths, cell shading, direct formatting of simple cell text,
  atomic structural formatting of several simple cells, semantic row deletion by
  label, occurrence, or checked index, and basic table formatting.
- Supports simple text content controls and typed bullet or decimal lists at
  levels zero through two, including safe continuation and restart; one bullet
  request may safely target separate source-ordered direct-body runs,
  external HTTP(S) hyperlinks, inline PNG/JPEG insertion and replacement,
  supported picture deletion and proportional resizing.
- Supports real next/continuous/odd/even-page section breaks, independently
  targeted page setup, first-page behavior, numbering restart/continue, simple
  default/first/even header/footer text and PAGE fields, and safe link/unlink.
  Odd/even behavior is explicitly document-wide; legacy single-section APIs remain.
- Uses typed operations, structured diagnostics, source-local safety checks,
  package verification, reopen checks, and operation-specific postconditions.
  Semantic edit failures now report bounded ambiguity candidates, failed row or
  column selectors, unsafe-source reasons, and the failing atomic update index.

- Provides bounded, read-only structural layout snapshots: section geometry and
  block ownership, effective paragraph controls, table row/width inputs, and image
  dimensions with safe width diagnostics. See [E1 layout inspection](e1-layout-inspection.md).
  Rendered page counts are supplied only by an optional application-side renderer.

## Node/N-API

The adapter exposes capability discovery, blank-document creation, inspection,
bounded style and layout snapshots,
text search, text replacement, paragraph insertion/deletion/style/formatting,
text formatting, table operations, page breaks, page setup, default
headers/footers, page numbers, section inspection/editing/linkage, lists, hyperlinks, and picture insertion,
deletion, resizing, replacement, and targeting inspection, plus simple
content-control text updates. DOCX mutation capabilities are Node-exposed.

## Immediate Direction

1. Publish `@opensuitehq/engine@0.1.2` (table-cell `set_text_formatting`) and bump app pins off local binaries.
2. Validate complete application and agent workflows.
3. Compare benchmark documents with current capabilities and identify real gaps.
4. Add remaining DOCX P0 capabilities only when those workflows require them.
5. Freeze DOCX V1 once benchmark and interoperability criteria are met, then
   begin PPTX.

See [the DOCX engine reference](docx-engine.md) for product scope and the DOCX
V1 stop rule.
