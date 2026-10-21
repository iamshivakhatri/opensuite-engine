# OpenSuite DOCX Engine

For engine versioning and publishing, see [releasing.md](releasing.md).

## Product Goal

Provide the DOCX operations needed to create and edit ordinary business,
academic, and operational documents while preserving unfamiliar OOXML. DOCX V1
does not aim to reproduce every Microsoft Word feature.

## Runtime inventory

The protocol's `DOCX_CAPABILITIES` declares each capability once, including its
operation kind and Node export when available. N-API capability discovery derives
from this inventory and includes callable operation metadata. Feature flags are
not standalone operations; Rust-only aliases remain absent from Node. Release
checks compare the inventory, compiled exports, and generated input definitions.
N-API compiler type records generate TypeScript declarations and JSON schemas;
Rust field comments supply descriptions and extra restrictions. Native metadata
returns these embedded schemas and a JSON/binary input flag. The product uses
these definitions for mutation tools and automatic dynamic discovery. Read tools also consume generated inputs; native metadata explicitly marks
JSON-encoded read output. E3 also marks binary retrieval `backendOnly`, excluding
it from JSON tool inputs. These contracts are published in engine 0.1.7. All five platform builds and the
clean npm consumer check pass.

Phase 3 local targeting: copy search `target` or inspection `textTarget` for text
edits, and heading `paragraphTarget` or paragraph `targetOccurrence` for paragraph
formatting. Positions are zero-based and artifact-local; refresh after content
edits. Header-label column selection includes column zero and refuses duplicates
without an explicit occurrence/index. Table handles with expected header labels
must agree. No new mutation inputs or durable handle protocol; npm consumers need
the next native release.

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
  Inline insertion optionally fits the insertion section's declared page/column
  bounds without upscaling and aligns left/center/right on the picture paragraph.
  Insertion also targets fresh source-stamped table-cell handles, adding an inline
  paragraph at the cell start/end while preserving existing text and properties.
  Cell autoFit uses declared grid/cell widths, cell/table margins, section bounds,
  and default paragraph spacing/indents without upscaling. Ambiguous geometry,
  merged/nested structures, and cell floating layout fail safely; styled tables
  and exact-height rows require explicit sizing. Explicit sizes retain their
  existing semantics. Picture deletion preserves owning paragraph properties,
  including section boundaries, and keeps cell paragraphs.
  Inline header/footer insertion targets fresh section handles and explicit
  default/first/even variants, safely creates missing parts, preserves existing
  content, and refuses shared/inherited parts. It fits to header/footer margin
  geometry by default. Read-only image inspection covers body, cells and
  header/footer parts; backend-only binary retrieval returns original PNG/JPEG
  bytes through source-validated handles with size limits. Rich/linked/ambiguous
  images fail safely. Header/footer image updates and floating authoring remain
  deferred. See [native image behavior and integration](e1-image-layout.md).
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


## Content controls, cell alignment and character styles (E5A)

`inspect_docx_content_controls` / Node `inspectDocxContentControls` is a read-only,
JSON-output operation for structured document tags (Word content controls) in the
main document. It reuses the source-backed SDT parser and the existing text
mutation's selector and patch checks. Records include tag, alias, kind, current
text, main part name, zero-based paragraph index, enclosing container, exact
`target`, `targetMatchCount`, `textMutationSupported`, and refusal reasons.
The paragraph is the enclosing paragraph for inline controls or the first
contained paragraph for block controls. Header/footer controls are outside this
inspection. No SDT XML is authored or changed by inspection.

Defaults are offset zero and 20 records; limit is clamped to 1–100. Text, tag and
alias are bounded to 2,000 Unicode characters. `truncated` values disable
`textMutationSupported`; truncated selectors are not returned. Deleted/moved-from
text is excluded. Unnamed controls remain inspectable but cannot be targeted.
Targets include both available tag/alias values and an explicit zero-based
occurrence among their matches. `OCCURRENCE_REQUIRED` explains duplicates; the
returned target already resolves them. Absent tag/alias fields are omitted from
the target so it can be passed directly to the existing native mutation. Refresh
inspection after content changes. `expectedCurrentText` still guards the edit.

Supported text mutations retain their existing limits: explicitly typed,
unlocked, unbound text/rich-text controls without placeholder state, containing
one ordinary direct paragraph. Inline controls, other control kinds, multiple
paragraphs, incompatible run formatting, hidden boundaries, fields, wrappers and
unsafe source text are reported with reasons. A complete inspection record does
not bypass package validation or the mutation's existing preservation checks.

```js
const snapshot = JSON.parse(await engine.inspectDocxContentControls(bytes, { limit: 20 }))
if (!snapshot.ok) throw new Error(JSON.stringify(snapshot.diagnostics))
const control = snapshot.controls.find(item => item.textMutationSupported)
if (!control) throw new Error('No supported content control on this page')
const edit = await engine.executeDocxSetContentControlText(bytes, {
  target: control.target,
  expectedCurrentText: control.text,
  replacement: 'Updated client name',
})
if (!edit.result.ok) throw new Error(JSON.stringify(edit.result.diagnostics))
bytes = edit.output
```

The existing `set_table_cells_formatting` /
`executeDocxSetTableCellsFormatting` accepts two optional fields on each update:

- `alignment`: `left`, `center`, or `right`, authored as `w:jc` on **every direct
  paragraph in the selected cell**, including empty paragraphs. There is no
  first-paragraph-only behavior or paragraph selection inside a cell.
- `verticalAlignment`: `top`, `center`, or `bottom`, authored as one `w:vAlign`
  in the cell's `w:tcPr` properties.
- `clear: ['alignment', 'verticalAlignment']`: remove either/both direct
  properties and restore inheritance. Clearing an absent property is a no-op;
  it does not write an explicit default. As with other formatting patches,
  clearing a property takes precedence over a simultaneous set value.

Existing table, row/column, legacy label and cell-handle selectors are reused.
All updates remain atomic, with the existing 1–100 update bound. Omitted fields
preserve imported properties and existing defaults. Alignment supports simple
multi-paragraph cells; fill/text-only updates keep their earlier single-paragraph
limits. Combining text formatting with alignment still requires ordinary text
runs in each paragraph. Cell text, run properties, styles, widths, margins,
borders, row heights and other package parts remain source-backed. Repeated
alignment requests do not duplicate properties. Changed scalar properties with
unknown attributes/children, duplicate property containers, merged/omitted,
nested, floating/wrapped, protected or ambiguous table structures fail safely.
Active document protection and pending property history are refused. Reopen
checks verify cell text and the requested alignment values.

```js
await engine.executeDocxSetTableCellsFormatting(bytes, {
  table: { handle: 't0' }, // Use a current inspected table handle or semantic selector.
  updates: [{
    target: { row: { kind: 'header' }, column: { kind: 'first' } },
    alignment: 'center',
    verticalAlignment: 'center',
  }],
})
```

The existing `set_text_formatting` / `executeDocxSetTextFormatting` accepts
`characterStyleId`, an existing character style ID from style inspection. It
writes `w:rStyle` before other run properties, using the current exact-text,
occurrence, and partial/multiple-run splitting infrastructure. Style IDs are
used, not display names. Missing IDs return `MISSING_STYLE`; paragraph-style IDs
return `STYLE_TYPE_MISMATCH`. `clear: ['characterStyleId']` removes only the
selected run reference, including unknown imported references, and restores
inheritance. Other direct formatting, unselected style references, imported style
definitions and unrelated XML remain unchanged. No style definitions are created
or rewritten by application/removal, and direct formatting can still override
style properties.

Ordinary body text and existing supported simple table-cell text are accepted.
Ambiguous text requires an explicit occurrence. Wrappers, revisions, protected
ranges/documents, fields, unsafe runs, duplicate `rPr`/`rStyle`, and unfamiliar
attributes/children on a changed style reference fail without output. Existing
single-paragraph cell targeting limits remain. There is no new character-style
operation or general style cascade; `TextFormattingPatch.character_style_id`
uses the existing Rust `PropertyPatch::Set/Clear` convention. Style definitions
continue to use `basedOn` for inheritance rather than a run style reference.

```js
await engine.executeDocxSetTextFormatting(bytes, {
  target: { text: 'Selected phrase' }, characterStyleId: 'Accent',
})
// Removal: same target with clear: ['characterStyleId'].
```

Native capability metadata, declarations and input schemas include the read
operation and both formatting extensions. Consumers need a native build from
this change, must JSON-parse the inspection result, and must expose these optional
fields through their existing formatting tools. No application changes, package
publication, version/pin changes, or paid model calls are part of E5A.

Focused Rust/native tests cover inspection-to-mutation, duplicate/alias targets,
unsupported controls, bounds, all alignments, multiple/empty paragraphs, clearing,
existing formatting and package preservation, style creation/application/removal,
invalid IDs/types, protected structures, alternate prefixes and save/reopen.
Four temporary LibreOffice renders of applied and cleared formatting each produced
one visually reviewed page, including a LibreOffice-produced DOCX edited through
the native binding. LibreOffice renamed the imported character style ID; fresh
style inspection supplied the correct ID. Native edits preserved its stylesheet,
unselected style references and all nine unrelated package parts byte-for-byte. Left/center/right and top/center/bottom positions were
correct for single and multiple paragraphs. The character style changed size,
color and italics while retaining direct bold; clearing restored inherited text
formatting and left/top cell positions while preserving shading/borders. No
clipping or missing text was observed. Word/Google Docs review remains untested.
E5B is unstarted; application integration and broader interoperability checks
remain follow-up work. Richer SDT authoring/editing and a general style cascade
remain outside this change.

## Native table layout controls (E4)

Rust `SetTableSpacing` / Node `executeDocxSetTableSpacing` inserts a gap only
between two directly adjacent body tables. `before` and `after` each contain a
fresh `tables.items[].layoutHandle` from `inspectDocx`. These source-stamped
handles validate the main XML, relationships and related section/settings parts;
old `t0` handles are not accepted by these new operations. Existing table APIs
keep their targeting behavior. Re-inspect after edits. `baseRevision` remains
opaque caller metadata, not an engine revision check.

`spacingPoints` accepts 0.05–1584 points in 0.05-point steps (one twip, or 1/20
point). An 8 pt request inserts one empty paragraph with `w:line="160"` and
`w:lineRule="exact"`. Before/after spacing, line-based before/after spacing and
automatic spacing are explicitly zero; grid snapping, contextual spacing,
numbering and inherited paragraph pagination flags are disabled. This avoids
Normal-style spacing turning the requested gap into a taller paragraph. Tables,
cells, widths, styles and other parts remain unchanged. There is no automatic
separator insertion. Repeated identical requests with fresh targets preserve the
same separator XML. A different spacing request updates that separator. An
existing arbitrary paragraph, nonadjacent/reversed tables, or a separator changed
by another producer is refused rather than silently reformatted or duplicated.
Only the exact paragraph shape authored by this operation is recognized.

Rust `SetTableRowLayout` / Node `executeDocxSetTableRowLayout` takes a fresh table
layout target and one or both of:

- `repeatingHeaderRows`: a count from zero through the row count. It writes
  `w:tblHeader w:val="1"` on that many consecutive leading rows and explicit
  `w:val="0"` on the rest, overriding inherited flags. Zero clears repetition;
  omission preserves imported header flags. A list of non-leading header rows
  is not a supported selection. Rust never infers which rows are headers.
- `row` plus `preventSplit`: select one row with an existing row handle such as
  `t0:r2`, or exact data-row `firstCellText` plus optional zero-based `occurrence`.
  A row handle is checked against the fresh table identity; optional supplied
  first-cell text must match. Header rows can use row handles. `true` writes
  `w:cantSplit w:val="1"`; `false` writes `w:val="0"` to explicitly allow
  splitting, including overriding an inherited setting. Omission preserves all
  splitting flags. Unselected rows and unrelated row properties remain unchanged.

Both operations reuse source patches, package preservation, source-range guards,
and save/reopen verification. Simple rectangular direct body tables are
supported. Merged/omitted cells, nested/floating/wrapped tables, protected ranges,
fields, dependent content, pending property revisions, active document
protection, stale identities, duplicate row properties and ambiguous row targets
fail without output bytes. Changing a flag with unknown attributes/content is
refused. Explicit Word namespace prefixes are required for authoring; ordinary
alternate prefixes are supported. Header repetition and row splitting remain
renderer instructions: an oversized row cannot be guaranteed to fit on a page,
and gaps at page boundaries follow the renderer's pagination rules. No page
layout engine, automatic header detection, or split control for whole tables.

```js
let page = await engine.inspectDocx(bytes, { focus: { kind: 'tables' } })
let [first, second] = page.tables.items
let result = await engine.executeDocxSetTableSpacing(bytes, {
  before: { handle: first.layoutHandle },
  after: { handle: second.layoutHandle },
  spacingPoints: 8,
})
if (!result.result.ok) throw new Error(JSON.stringify(result.result.diagnostics))
bytes = result.output

// Handles must come from the current output.
page = await engine.inspectDocx(bytes, { focus: { kind: 'tables' } })
first = page.tables.items[0]
result = await engine.executeDocxSetTableRowLayout(bytes, {
  table: { handle: first.layoutHandle },
  repeatingHeaderRows: 2,
  row: { handle: first.rows[2].handle },
  preventSplit: true,
})
if (!result.result.ok) throw new Error(JSON.stringify(result.result.diagnostics))
bytes = result.output
// Use a fresh target with repeatingHeaderRows: 0 to clear headers, or
// row + preventSplit: false to restore splitting.
```

Deterministic Rust/native tests cover exact XML, both boundaries in three-table
runs, repeated requests, occupied boundaries, invalid/stale identities, multiple
headers and clearing, long multi-line rows, unrelated row/property/package
preservation, save/reopen, namespace scope and unsupported/protected inputs.
Temporary LibreOffice fixtures produced ten reviewed pages: an 8 pt gap on one
page, a three-page table with two header rows on every page, and two three-page
long-row comparisons. Default splitting divided rows 3 and 5 across pages;
`cantSplit` kept all five rows intact (two/two/one rows per page). The gap measured
8 pt between the table border edges (8.5 pt between border centerlines due to
stroke width), even with large inherited Normal spacing. No clipping or missing
content was observed. Word/Google Docs pagination has not been checked for E4.

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
TOC update in Word/LibreOffice remains required. Phase 7 includes a Microsoft Word
open check of a mixed-field output; Word save and Google Docs review remain pending. No engine version or package pin changes.

## Tracked-change inspection

Read-only `inspect_docx_tracked_changes` / Node `inspectDocxTrackedChanges` returns
main-document revisions in source order, including table paragraphs. The snapshot
contains ID, insertion/deletion kind, original XML element name (`revisionType`), author/date, text, paragraph index, structural
status, and reason codes. Default 20 entries, maximum 100, offset paging, and
2,000-character text/metadata bounds. Totals distinguish supported insertions,
supported deletions, and unsupported revision structures. Move/range markers (both starts and ends),
property/row/cell changes (including table property exceptions), Word 2010 conflicts, and nested revisions are counted with diagnostics;
their text is not interpreted. Counts describe XML revision records/markers,
not logical edit groups. Headers/footers and other parts are outside this snapshot.

Existing current-text inspection includes insertions/move-to and excludes
deletions/move-from; original-text views do the reverse. Those text semantics remain unchanged. Phase 7 adds conservative mutation guards
for hidden boundaries and enclosing protected structures. Unrelated safe text edits preserve revision
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

## Word semantics safety boundaries

Phase 7 reuses source patches and the existing protected-range checks. Text
replacement refuses hidden markup between runs; paragraph deletion/insertion
checks enclosing ranges and fields. Revision decisions refuse enclosing
move/conflict/bookmark/field ranges. Text formatting, comment-body updates, and
header/footer writers refuse pending property history that could be removed or
duplicated. Stray field instructions are diagnosed. Supported unrelated edits
retain comments, fields, bookmarks, revisions, and package payloads.
See [the coverage map and interoperability evidence](word-semantics.md).
