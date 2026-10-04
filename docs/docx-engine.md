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
  and inline PNG/JPEG pictures.
- Page breaks and real section boundaries (next/continuous/odd/even page),
  independent section page setup, first-page behavior, numbering restart/continue,
  default/first/even simple headers/footers and PAGE fields, and link/unlink.
  Odd/even headers are explicitly document-wide. See [E1 details](e1-sections.md).
- Reopen and semantic postcondition verification for mutations, while untouched
  package content remains preserved where practical.

DOCX mutations, including section editing, are exposed through Node/N-API.
Capability details are kept in [status.md](status.md).

## Add Only When Benchmarks Need It

- Long-document structure: table of contents, bookmarks, and cross-references.
- Review workflow: simple comments and safe accept/reject of revisions.
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
