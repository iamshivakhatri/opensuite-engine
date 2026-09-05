# OpenSuite Engine Status

## Current Phase

Preservation-first DOCX mutation

## Current Format

DOCX

PPTX and XLSX have not started.

## Current Objective

Prove one source-backed DOCX mutation while preserving untouched package parts
and XML source bytes.

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
- Read-only standard DOCX comments, including comments-part relationship discovery,
  metadata, body text, document range/reference pairing, and compact
  `opensuite inspect-comments <file.docx>` output. Modern/threaded comments remain
  unsupported and source-preserved.
- Runtime/protocol boundary foundation: versioned DOCX read-side capability
  discovery, compact diagnostics, and `opensuite capabilities`.
- Preservation-safe `ReplaceText` v0 for one exact current-view `w:t` node,
  with a semantic expected-text precondition, XML escaping, output reopen check,
  structured result, and `opensuite replace-text <input.docx> <output.docx> <target> <replacement>`.
- Current-view exact semantic text search across runs within one paragraph or
  table-cell paragraph, with bounded context, deterministic occurrences, and
  `opensuite find-text <input.docx> <text>`. `ReplaceText` v0 uses the same
  resolver; cross-run matches are found but remain unsupported for mutation.
- Mutation output verification reads every ZIP payload, checks XML syntax and
  internal relationship targets, then reopens and checks the semantic result.

## Current Repository Target

- `opensuite-opc`
- `opensuite-docx`
- `opensuite-protocol`
- `opensuite` CLI binary

## Explicitly Not Implemented Yet

- cross-run or tracked-change text mutation
- mutation batches or transactions
- rendering
- server
- agent runtime
- Node bindings
- Python bindings
- PPTX
- XLSX

`ReplaceText` v0 writes a new artifact atomically after reopening it, while
leaving the input artifact untouched. It copies unchanged part payloads as-is
and patches only the main document XML text source region. The engine checks
the semantic expected text; opaque application revision metadata is accepted
but not enforced because application history remains outside the engine.

Cross-run edits, tracked-change edits, batches, broader validation,
serialization APIs, and rendering remain unimplemented.

Small hand-assembled fixtures remain useful for parser tests, but are not
Office-interoperability evidence. Mutation interoperability checks use a
known Office-openable input and still require manual Word/Google Docs review.

Tracked formatting/property revisions such as `w:rPrChange`, `w:pPrChange`,
and table/section property changes remain unsupported and source-preserved.

## Next Milestone

Extend typed DOCX mutation only when a new operation has a similarly narrow,
preservation-safe source mapping.
