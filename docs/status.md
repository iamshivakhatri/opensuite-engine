# OpenSuite Engine Status

## Current Scope

OpenSuite is a preservation-first DOCX engine. The Rust workspace contains the
OPC package layer, DOCX engine, stable protocol types, native CLI, and Node
N-API adapter. PPTX and XLSX have not started.

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
  direct paragraph and text formatting.
- Creates, deletes, and safely edits simple direct-body tables, including rows,
  columns, cells, and basic table formatting.
- Supports simple text content controls, level-zero bullet or decimal lists,
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
headers/footers, and page numbers. Rust DOCX capabilities currently exceed
Node/N-API parity for hyperlinks, lists, content controls, and picture
mutations.

## Immediate Direction

1. Reach DOCX N-API parity.
2. Validate complete application and agent workflows.
3. Compare benchmark documents with current capabilities and identify real gaps.
4. Add remaining DOCX P0 capabilities only when those workflows require them.
5. Freeze DOCX V1 once benchmark and interoperability criteria are met, then
   begin PPTX.

See [the DOCX engine reference](docx-engine.md) for product scope and the DOCX
V1 stop rule.
