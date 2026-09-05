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
- Preservation-safe `ReplaceText` for exact Current-view text in one source
  node or across compatible ordinary runs in one paragraph, with semantic
  preconditions, XML escaping, output reopen checks, and structured results.
  Incompatible run formatting, inline wrappers, and tracked revision content
  remain unsupported.
- Current-view exact semantic text search across runs within one paragraph or
  table-cell paragraph, with bounded context, deterministic occurrences, and
  `opensuite find-text <input.docx> <text>`. `ReplaceText` uses the same resolver.
- Read-only targeted text-context inspection for a `TextTarget`, returning its
  full semantic container and up to ten nearby containers in source order.
  `opensuite inspect-context <input.docx> <text> [occurrence] [before] [after]`
  reuses Current-view target resolution and is declared as `inspect_context`.
- Mutation output verification reads every ZIP payload, checks XML syntax and
  internal relationship targets, then reopens and checks the semantic result.
- Preservation-safe `InsertParagraphAfter` for a semantic text anchor in an ordinary,
  direct main-document body paragraph. It inserts a plain unformatted paragraph,
  verifies the output package, reopens it, and proves the paragraph follows its anchor.
- Preservation-safe `DeleteParagraph` for an ordinary direct body paragraph selected
  through semantic text. It removes only that source span, checks ranges and wrappers,
  preserves existing relationships, and verifies the reopened body structure.
- Preservation-safe `SetTableCellText` for a simple top-level table cell selected by
  exact row-label and column-header text. It supports one ordinary paragraph with
  direct runs, including empty paragraphs, and verifies the reopened table cells.
- Preservation-safe `SetContentControlText` for a simple text content control selected
  by exact tag and/or alias. It supports one ordinary paragraph with direct runs,
  including empty paragraphs, and verifies the reopened control text and metadata.
- Preservation-safe `SetParagraphFormatting` for an ordinary direct body paragraph
  selected through `TextTarget`. It patches only direct `w:pPr` alignment, spacing,
  indentation, and keep properties, then reopens and verifies the direct formatting.
- Preservation-safe `SetTextFormatting` for one complete ordinary direct body run selected
  through `TextTarget`. It patches only direct bold, italic, font-size, and font-family
  properties, then reopens and verifies the direct formatting.

## Current Repository Target

- `opensuite-opc`
- `opensuite-docx`
- `opensuite-protocol`
- `opensuite` CLI binary

## Explicitly Not Implemented Yet

- tracked-change and incompatible/wrapper cross-run text mutation
- paragraph insertion in tables, headers/footers, content controls, tracked wrappers,
  or after section-boundary paragraphs; insert-before, copied formatting, explicit styles,
  and numbered-list semantics
- empty paragraph targeting; table/header/footer/wrapper/section-boundary paragraph deletion;
  automatic bookmark/comment repair and relationship garbage collection
- merged or nested tables, table cells with multiple paragraphs or inline wrappers,
  and generic table editing
- data-bound, locked, placeholder-state, repeating, group, picture, checkbox, dropdown,
  combo-box, nested, or multi-paragraph content-control mutation
- paragraph styles, numbering, tabs, borders, shading, section properties, and paragraph
  formatting outside ordinary direct main-body paragraphs
- substring, cross-run, table/header/footer, tracked, wrapped, or general character formatting
- mutation batches or transactions
- rendering
- server
- agent runtime
- Node bindings
- Python bindings
- PPTX
- XLSX

`ReplaceText` writes a new artifact atomically after reopening it, while
leaving the input artifact untouched. It copies unchanged part payloads as-is
and patches only the main document XML text source region. The engine checks
the semantic expected text; opaque application revision metadata is accepted
but not enforced because application history remains outside the engine.

Tracked-change edits, incompatible/wrapper cross-run edits, batches, broader validation,
serialization APIs, and rendering remain unimplemented.

Small hand-assembled fixtures remain useful for parser tests, but are not
Office-interoperability evidence. Mutation interoperability checks use a
known Office-openable input and still require manual Word/Google Docs review.

Tracked formatting/property revisions such as `w:rPrChange`, `w:pPrChange`,
and table/section property changes remain unsupported and source-preserved.

## Next Milestone

Extend typed DOCX mutation only when a new operation has a similarly narrow,
preservation-safe source mapping.
