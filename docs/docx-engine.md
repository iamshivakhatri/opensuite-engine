# OpenSuite DOCX Engine

## Product Goal

Provide the DOCX operations needed to create and edit ordinary business,
academic, and operational documents while preserving unfamiliar OOXML. DOCX V1
does not aim to reproduce every Microsoft Word feature.

## Current Capability Groups

- Package opening, source-aware semantic inspection, exact text search, bounded
  context inspection, capability discovery, and structured diagnostics.
- Bounded style-system inspection covering typography origins, paragraph and
  list patterns, table appearance, sections, headers/footers, and theme references.
- Custom paragraph and character style creation/update, with clear-to-inherit
  patches, parent/cycle and next-style validation. See [E1 style details](e1-custom-styles.md).
- Read-only structural layout inspection, bounded section/block details, inherited
  pagination controls, and table/image width diagnostics. Optional LibreOffice PDF
  rendering lives outside Rust; exact block-to-page mapping remains deferred.
  See [E1 layout inspection](e1-layout-inspection.md).
- Paragraph creation and deletion, text replacement, existing paragraph styles,
  direct paragraph formatting, and direct text formatting including color,
  underline, highlight, strikethrough, and superscript/subscript.
- Simple-table creation, deletion, row/column lifecycle (including semantic row
  deletion), cell edits, explicit
  column widths, cell shading, atomic structural formatting of several simple cells,
  direct text formatting in simple cells, and basic table formatting.
- Read and safe targeted mutation support for simple content controls, typed
  bullet and decimal lists at levels zero through two (including separate,
  source-ordered bullet runs), external hyperlinks,
  and PNG/JPEG pictures with typed inline/floating insertion, proportional or exact
  size, common wrapping and positioning, and preservation-aware layout patches.
  See [E1 image layout](e1-image-layout.md).
- Page breaks and real section boundaries (next/continuous/odd/even page),
  independent section page setup, first-page behavior, numbering restart/continue,
  default/first/even simple headers/footers and PAGE fields, and link/unlink.
  Odd/even headers are explicitly document-wide. See [E1 details](e1-sections.md).
- Standard comment inspection with bounded paging, metadata, attached text,
  paragraph locations, and malformed/orphan diagnostics. Add comments to exact
  text across simple direct runs in one body paragraph; update plain comment
  text and delete matching records/markers through fresh source-stamped handles.
  Creates missing comments package pieces and preserves unrelated comments/parts.
  Threaded metadata, replies/resolve, rich-content editing, field/wrapper/revision
  selections, and cross-paragraph authoring remain unsupported.
- Reopen and semantic postcondition verification for mutations, while untouched
  package content remains preserved where practical.

DOCX mutations, including section editing, are exposed through Node/N-API.
Capability details are kept in [status.md](status.md).

## Tracked-change inspection

Read-only `inspect_docx_tracked_changes` / Node `inspectDocxTrackedChanges` returns
main-document revisions in source order, including table paragraphs. The snapshot
contains ID, insertion/deletion kind, author/date, text, paragraph index, structural
status, and reason codes. Default 20 entries, maximum 100, offset paging, and
2,000-character text/metadata bounds. Totals distinguish supported insertions,
supported deletions, and unsupported revision structures. Move/range markers,
property/row/cell changes, and nested revisions are counted with diagnostics;
their text is not interpreted. Counts describe XML revision records/markers,
not logical edit groups. Headers/footers and other parts are outside this snapshot.

Existing current-text inspection includes insertions/move-to and excludes
deletions/move-from; original-text views do the reverse. Those semantics and
mutation guards remain unchanged. Unrelated safe text edits preserve revision
XML byte-for-byte. No revision authoring or accept/reject operations are exposed.

## Add Only When Benchmarks Need It

- Long-document structure: table of contents, bookmarks, and cross-references.
- Review workflow: safe accept/reject of revisions.
- Footnotes/endnotes.
- Other page, header/footer, section, field, or layout features demonstrated as
  necessary by the benchmark documents.

## Outside DOCX V1

Preserve these when imported, but do not prioritize authoring them: SmartArt,
WordArt, advanced floating shapes and wrapping, advanced DrawingML effects,
equations, mail-merge internals, macros, embedded OLE objects, advanced fields,
elaborate newsletter layout, obscure numbering systems, full tracked-change
authoring, and every header/footer or section variant.

## Benchmark Documents

DOCX V1 is assessed through real engine creation and editing of:

1. A business report with headings, rich text, lists, tables, images, links,
   page composition, headers/footers, page numbers, and a table of contents.
2. A business proposal.
3. A resume and cover letter.
4. A research or lab paper.
5. A company SOP or policy.
6. A technical design or system specification.
7. A reviewed, Word-created imported document.
8. An ordinary office pack: letter, memo, agenda, and meeting minutes.

## Preservation and Freeze Rule

Mutations must be typed, narrow, and source-aware; they must retain unfamiliar
content and pass reopen plus semantic postcondition checks. Representative
artifacts must open correctly in Microsoft Word, Google Docs, and LibreOffice.

Freeze DOCX V1 when the benchmark set can be created and edited end-to-end,
unknown content is preserved, representative artifacts interoperate, and the
Node/application layer can compose the supported operations reliably. Then move
active format work to PPTX; return to DOCX only when real workflows show a
meaningful gap.
