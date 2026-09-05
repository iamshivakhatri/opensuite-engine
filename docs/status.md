# OpenSuite Engine Status

## Current Phase

DOCX read-side semantics

## Current Format

DOCX

PPTX and XLSX have not started.

## Current Objective

Extend source-backed DOCX inspection while preserving original package and XML
content unchanged.

## Implemented

- Minimal Rust workspace, CI, and native CLI.
- Read-only OPC package opening with safe ZIP entry indexing.
- `[Content_Types].xml` Default and Override content type resolution.
- Package-level `_rels/.rels` parsing and main office document discovery through
  Transitional or Strict `officeDocument` relationships.
- `opensuite inspect <path-to-office-file>` JSON package metadata output.
- Source-aware DOCX semantic views for document text, tables, styles,
  numbering, sections, headers/footers, references, pictures, and fields.
- Read-only `fldSimple` and nested complex-field inspection, including source
  boundaries, instruction text, and cached result text when declared.
- Read-only WordprocessingML content-control (`w:sdt`) inspection, including
  source IDs, declared metadata, visible text, list/date metadata, and declared
  data bindings without Custom XML resolution.
- `opensuite inspect-content-controls <file.docx>` compact semantic output.
- Read-only DOCX tracked-content changes: insertions, deletions, move-from, and
  move-to revisions with metadata and Current/Original paragraph and cell text
  views. `opensuite inspect-tracked-changes <file.docx>` and
  `opensuite inspect-revision-view <file.docx> <current|original>` expose compact JSON.
- Runtime/protocol boundary foundation: versioned DOCX read-side capability
  discovery, compact diagnostics, and `opensuite capabilities`.

## Current Repository Target

- `opensuite-opc`
- `opensuite-docx`
- `opensuite-protocol`
- `opensuite` CLI binary

## Explicitly Not Implemented Yet

- document mutation
- transactions
- rendering
- server
- agent runtime
- Node bindings
- Python bindings
- PPTX
- XLSX

Future mutation, revision/precondition, conflict, validation, serialization,
and rendering requirements are recorded in `docs/runtime-boundary.md`; they
are not implemented capabilities.

Tracked formatting/property revisions such as `w:rPrChange`, `w:pPrChange`,
and table/section property changes remain unsupported and source-preserved.

## Next Milestone

Build the next explicitly selected DOCX capability without adding mutation or
serialization.
