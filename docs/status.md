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

- Supports typed inline/floating image insertion, exact or proportional sizing, and
  source-local floating layout updates. Square/top-and-bottom/behind/front wrapping;
  page/margin/column horizontal and page/margin/paragraph vertical references.
  Layout inspection returns fresh image handles, anchor facts, ownership, and width/position
  diagnostics. Replacement preserves layout and rejects assets shared across document parts.
  See [E1 image layout](e1-image-layout.md) for supported and deferred cases.
- Inspects standard Word comments with bounded text, attached text, source-order
  paragraph locations, metadata, marker diagnostics, and fresh handles. Adds
  comments to exact selections across simple runs in one body paragraph; edits
  plain comment text and safely deletes records/markers without deleting document
  text. First-comment creation adds the part, relationship, and content type.
  Untouched comments and package payloads remain source-backed. Cross-paragraph
  authoring, fields/wrappers/revisions, rich comment editing, replies, resolved
  state, and mutation of threaded-comment metadata are deferred. Rust/N-API takes
  explicit author and UTC ISO date; engine-client supplies current UTC if omitted.
- Provides bounded, read-only structural layout snapshots: section geometry and
  block ownership, effective paragraph controls, table row/width inputs, and image
  dimensions with safe width diagnostics. See [E1 layout inspection](e1-layout-inspection.md).
  Rendered page counts are supplied only by an optional application-side renderer.


## Protected structural deletion

Table, row, and column deletion validate every planned source region before
writing output. One shared guard uses the existing parsed source index and field
parser to refuse intersections with comment/bookmark/field ranges, including
ranges enclosing deleted content. Permission, move/custom-XML, and proof/error
ranges are also protected. Fully contained protected ranges, comment references,
content controls, hyperlinks, note/media/object references, section properties,
and tracked property changes are conservatively refused; no dependent cleanup
or range repair is attempted. Malformed paired ranges or fields in the main part
also refuse deletion. Existing table shape and revision-wrapper guards remain.

Failures return `UNSUPPORTED_OPERATION` with `UNSUPPORTED_STRUCTURAL_DELETE`,
a concise structure-specific message, and no output artifact through Rust and
N-API. Column regions are checked together before any patch; ordinary simple
deletions still reopen and verify table shape while retaining unrelated parts.
Three generated regressions cover unsafe table/row/column deletion. No protocol,
application, dependency, or version change.

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
unrelated payloads. Fields, inline wrappers, tables, marker-crossing ranges, and
paragraphs containing revisions are unsupported authoring targets. Reopen checks
verify original/current text plus new revision type, ID, order, author/date, and
full text using existing revision inspection.

Typed `accept_revision` / `reject_revision` decide one insertion/deletion through a fresh source-stamped inspection handle (source order plus byte location). Any changed main XML requires fresh inspection; duplicate numeric IDs remain safe. Accept insertion unwraps runs; reject insertion removes content. Accept deletion removes content; reject deletion unwraps runs and renames only `delText` tags to `t`. Original run formatting, whitespace, tabs/breaks, other revision source bytes, and untouched parts are preserved. Reopen verifies current text, revision count, and remaining source bytes with the existing inspector. Node exposes `executeDocxAcceptRevision` / `executeDocxRejectRevision` with `{ handle }`.

Decisions require ordinary runs directly inside a paragraph, including simple table-cell paragraphs. Nested/move/property/structural revisions, malformed wrappers/IDs, richer containers, incompatible text tags, and retained content depending on wrapper XML context fail safely. Replacement uses two decisions, with fresh inspection between them. No accept-all or batch operation.

## Complete formatting contracts

The Node bridge now exposes all existing paragraph patch fields: five alignment
values, before/after spacing, line spacing, all four indents, and keep-with-next /
keep-lines. `clear` removes declarations to restore inheritance; legacy clear
flags remain compatible. Auto line spacing uses 240 units per line, while exact
and at-least spacing use twips. Word-style updates share the same value parsers
and now expose line spacing plus existing highlight, strike, and vertical text
alignment. Direct text patches can clear every supported property; oversized
font sizes and invalid alignment values fail rather than wrap or disappear.
Rust mutation semantics, engine version, and npm packages are unchanged.

## Node/N-API

The adapter exposes capability discovery, blank-document creation, inspection,
bounded style and layout snapshots,
text search, text replacement, paragraph insertion/deletion/style/formatting,
text formatting, table operations, page breaks, page setup, default
headers/footers, page numbers, section inspection/editing/linkage, lists, hyperlinks, and picture insertion,
deletion, resizing, floating layout updates, replacement, and targeting inspection, plus
standard comment inspection/add/update/delete and simple content-control text updates. DOCX mutation capabilities are Node-exposed.

## Immediate Direction

1. Publish `@opensuitehq/engine@0.1.2` (table-cell `set_text_formatting`) and bump app pins off local binaries.
2. Validate complete application and agent workflows.
3. Compare benchmark documents with current capabilities and identify real gaps.
4. Add remaining DOCX P0 capabilities only when those workflows require them.
5. Freeze DOCX V1 once benchmark and interoperability criteria are met, then
   begin PPTX.

See [the DOCX engine reference](docx-engine.md) for product scope and the DOCX
V1 stop rule.
