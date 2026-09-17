# OpenSuite Engine Status

## Current Scope

OpenSuite is a preservation-first DOCX engine. The Rust workspace contains the
OPC package layer, DOCX engine, stable protocol types, native CLI, and Node
N-API adapter. PPTX currently has package validation, slide/shape inspection,
cheap paragraph text search, and safe replacement of one directly present
DrawingML text run; XLSX has not started.

## Current DOCX Engine

- Opens DOCX packages from files or owned bytes, discovers the main document,
  resolves content types and relationships, and writes verified in-memory or
  file output while retaining untouched payloads.
- Inspects semantic document content: text, headings, body blocks, tables,
  styles, numbering, sections, headers/footers, references, pictures, fields,
  content controls, comments, and tracked-change views. Exact text search and
  bounded context inspection use source order and opaque local handles.
- Creates blank DOCX files and inserts or deletes safe direct-body paragraphs.
  It replaces text, assigns existing paragraph styles, and applies supported
  direct paragraph and text formatting, including color, underline, highlight,
  strikethrough, and superscript/subscript.
- Creates, deletes, and safely edits simple direct-body tables, including rows,
  columns, cells, explicit column widths, cell shading, and basic table formatting.
- Supports simple text content controls and typed bullet or decimal lists at
  levels zero through two, including safe continuation and restart; one bullet
  request may safely target separate source-ordered direct-body runs,
  external HTTP(S) hyperlinks, inline PNG/JPEG insertion and replacement,
  supported picture deletion and proportional resizing.
- Supports dedicated page breaks, single-section page setup, simple default
  header/footer text, and simple header/footer PAGE fields.
- Uses typed operations, structured diagnostics, source-local safety checks,
  package verification, reopen checks, and operation-specific postconditions.

## Node/N-API

The adapter exposes capability discovery, blank-document creation, inspection,
text search, text replacement, paragraph insertion/deletion/style/formatting,
text formatting, table operations, page breaks, page setup, default
headers/footers, page numbers, lists, hyperlinks, and picture insertion,
deletion, resizing, replacement, and targeting inspection, plus simple
content-control text updates. DOCX mutation capabilities are Node-exposed.

## Immediate Direction

1. Validate complete application and agent workflows.
2. Compare benchmark documents with current capabilities and identify real gaps.
3. Add remaining DOCX P0 capabilities only when those workflows require them.
4. Freeze DOCX V1 once benchmark and interoperability criteria are met, then
   begin PPTX.

See [the DOCX engine reference](docx-engine.md) for product scope and the DOCX
V1 stop rule.
