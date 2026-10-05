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
- Complete Node paragraph patches reuse Rust alignment, spacing/line spacing, all
  indents, keep rules, and clear-to-inherit semantics; text patches also clear all
  supported properties. No new formatting semantics or version change.
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
  selections, and cross-paragraph authoring remain unsupported. Comments and
  revisions may coexist on independent ordinary ranges in the same paragraph.
- Reopen and semantic postcondition verification for mutations, while untouched
  package content remains preserved where practical.

DOCX mutations, including section editing, are exposed through Node/N-API.
Capability details are kept in [status.md](status.md).


## Structural deletion safety

Deleting a table, row, or column refuses before mutation when any affected source
region intersects a protected Word range or dependent structure. Comments,
bookmarks, complex/simple fields, permission ranges, move/custom-XML ranges, and
proof/error ranges remain intact. Fully contained protected structures are also
refused in this pass: table deletion does not remove comment records or repair
bookmark references. Affected content controls, hyperlinks, notes, media/objects,
section properties, and tracked property changes fail safely. Malformed ranges
or field markers in the main part conservatively block structural deletion.

The existing Rust/N-API failure response identifies the unsafe structure with
`UNSUPPORTED_STRUCTURAL_DELETE` and returns no output bytes. All column cells are
checked together before applying patches. Safe deletions retain existing reopen,
package, and table-shape verification; no separate whole-document range validator
is needed because accepted deletions leave every protected range untouched.

## Word fields and table of contents

`inspect_docx_fields` / Node `inspectDocxFields` uses the existing source-backed
simple/complex field parser. It recognizes PAGE, NUMPAGES, TOC, DATE, and unknown
codes without evaluating them. Instruction and cached result are separate;
normal document text retains its existing cached-text semantics. Locations are
package part names and zero-based paragraph indices within each part. The main
part is followed by all header/footer package parts (including unused parts) in
name order; totals count XML fields, including duplicated footer variants.
Default 20 records, maximum 100, offset paging, 2,000-character instruction/result
bounds, at most 100 part diagnostics and 20 reason codes per record. Indexed text
ranges avoid scanning the whole XML again for every field. Nested fields are
reported as unsupported with unavailable result text; missing instructions,
invalid flags/markers, and unterminated structures have diagnostics. A complete
complex field without a separator has no cached result. Dirty/locked flags are
nullable: absence does not prove that a cached result is current.

Typed `insert_fields` / Node `executeDocxInsertFields` creates a new paragraph
with 1–20 plain-text/PAGE/NUMPAGES items. Body placement reuses existing body-block
handles; footer placement appends to an unshared single-section default footer,
reusing the existing header/footer relationship and package writer. PAGE writing
is shared with the earlier page-number feature; its existing output stays the
same. New fields contain `?` and are marked dirty. `insert_toc` / Node
`executeDocxInsertToc` inserts a real complex TOC field and optional plain title
paragraph. Heading levels 1–3 by default in Node, configurable through 1–9, with
common `\o`, `\h`, `\z`, and `\u` switches. Its dirty field contains an explicit
refresh placeholder. Word-compatible software calculates TOC entries, page
numbers, and total pages; Rust never does. Reopen checks verify authored
instructions/results/dirty flags and unchanged source bytes around the patch.
Insertion refuses malformed markers or an existing field crossing the insertion
point; text replacement refuses field content and field-crossing selections.
Unknown imported instructions/results are preserved without normalization.

OpenSuite exposes these APIs lazily under `document.fields`. No arbitrary field
code writer, DATE authoring, existing-field update/removal, TOC removal, custom
style mapping, or field evaluator. LibreOffice smoke open/save recognized the
TOC and refreshed PAGE/NUMPAGES, but retained the TOC placeholder; an explicit
TOC update in Word/LibreOffice remains required. Manual Word/Google Docs review
is pending. No engine version or package pin changes.

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
XML byte-for-byte.

Typed `insert_tracked_text`, `delete_tracked_text`, and
`replace_text_with_tracked_change` author real Word revisions in one ordinary
body paragraph. Insert before/after an exact text anchor (default after in Node),
track-delete an exact selection, or replace with a deletion followed by insertion.
Simple multi-run replacement requires identical source run properties; deletion
retains each run's formatting. Rust receives explicit author and UTC ISO date;
engine-client supplies current UTC when omitted. Text is limited to 32,000 bytes
and excludes control characters/newlines. IDs are allocated above numeric IDs
already present in the main document, without renumbering imported records.
Missing/invalid imported revision IDs and exhausted IDs fail safely.

Shared exact-selection/run-splitting helpers preserve source formatting and
unrelated payloads. Independent ordinary ranges in one paragraph can contain
several tracked edits and comments. Existing revisions and comment markers remain source-backed.
A shared source-range guard refuses targets inside/crossing revisions, comment
boundaries, and other protected Word ranges; tracked authoring also refuses
text enclosed by an existing comment. Comment authoring retains supported
overlap with existing comments without moving their markers. Fields, inline
wrappers, tables, and paragraph property revisions remain unsupported targets.
Reopen checks verify original/current text plus new revision type, ID, order, author/date, and
full text using existing revision inspection.

Typed `accept_revision` / `reject_revision` decide one insertion/deletion through a fresh source-stamped inspection handle (source order plus byte location). Any changed main XML requires fresh inspection; duplicate numeric IDs remain safe. Accept insertion unwraps runs; reject insertion removes content. Accept deletion removes content; reject deletion unwraps runs and renames only `delText` tags to `t`. Original run formatting, whitespace, tabs/breaks, other revision source bytes, and untouched parts are preserved. Reopen verifies current text, revision count, and remaining source bytes with the existing inspector. Node exposes `executeDocxAcceptRevision` / `executeDocxRejectRevision` with `{ handle }`.

Decisions require ordinary runs directly inside a paragraph, including simple table-cell paragraphs. Nested/move/property/structural revisions, malformed wrappers/IDs, richer containers, incompatible text tags, and retained content depending on wrapper XML context fail safely. Replacement uses two decisions, with fresh inspection between them. Removing a revision that intersects an existing comment anchor fails safely;
comments on unrelated ordinary text survive decisions unchanged. No accept-all
or batch operation.


## Footnotes and endnotes

Bounded `inspect_docx_notes` / Node `inspectDocxNotes` reports ordinary footnotes
and endnotes together, with separate totals, IDs, fresh source-stamped handles,
body paragraph and source reference indexes, text, and structural diagnostics.
Default 20 records, maximum 100, 2,000-character text bounds. Separator,
continuation separator, and continuation notice records are excluded and
protected by type. Imported document/section numbering format, restart, position,
and start settings are inspectable and remain unchanged; IDs are not displayed
note numbers.

Typed `insert_note`, `update_note`, and `delete_note` share one source-backed
implementation with a footnote/endnote kind. Insertion places a reference after
exact ordinary body text using the existing selection, run-splitting, and
protected-range checks. First use adds the related part, relationship, content
type, and separator records; existing parts are patched. IDs are allocated above
all existing IDs of that kind, including special records, without renumbering.
Existing FootnoteText/EndnoteText paragraph styles are reused when present.

Updates patch the text source of a simple one-paragraph, one-text-run note;
formatting, automatic note marks, IDs, and body references stay unchanged.
Deletion removes the reference and record together, retaining the valid related
part and remaining IDs. All writes reopen, verify record/reference coherence,
expected text/counts, and unchanged body text. Fresh inspection is required after
any body or note-part mutation. Malformed/orphan/duplicate records or references
refuse mutation. Rich/multi-paragraph notes remain inspectable and preserved,
but update/delete refuse; custom reference marks and protected body references
cannot be deleted.

Rust/N-API expose the same typed primitives. OpenSuite's engine-client and lazy
`document.notes` tools use them with existing run-local save and blocking
verification. No numbering configuration, header/footer note workflows,
notes-body review authoring, rich note authoring, citation/bibliography framework,
new dependencies, engine publication, or version changes.

Generated lifecycle, preservation, review/field coexistence, malformed refusal,
and 30-note paging/timing scenarios passed. A LibreOffice-produced 3-footnote /
1-endnote import was inspected, edited, extended, and deleted with unrelated note,
separator, settings, style, and body payloads preserved. LibreOffice rendered the
manual-review DOCX; Microsoft Word/Google Docs review remains pending.

## Add Only When Benchmarks Need It

- Long-document structure beyond the common TOC: bookmarks and cross-references.
- Review workflow: richer revision decisions beyond ordinary run insertions/deletions.
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
