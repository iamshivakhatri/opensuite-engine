# Word semantics and preservation

Phase 7 stabilizes the existing DOCX operations. It adds no operation IDs,
field evaluator, revision framework, or new comment system. The changes are local
and unpublished; the npm package remains 0.1.7.

## Existing coverage and boundaries

| Area | Classification | Actual behavior |
| --- | --- | --- |
| Ordinary revision inspection | Implemented and covered | Main-document insertion/deletion records, including table paragraphs; IDs, authors, dates, bounded text, source-order locations, and fresh handles. |
| Revision authoring | Partially supported and covered | Insert, delete, or replace exact ordinary text in one direct body paragraph. Independent revisions/comments can coexist nearby. Replacement requires compatible run properties. Fields, wrappers, table authoring, property revisions, and protected selections refuse safely. |
| Revision decisions | Partially supported and covered | Accept/reject one ordinary insertion/deletion consisting of direct paragraph runs, including simple table-cell paragraphs. Formatting and unrelated revisions survive. Re-inspect after each decision; numeric IDs alone are not targets. |
| Move, property, structural, nested, and conflict revisions | Inspection implemented; decisions explicitly unsupported | Count recognized XML records/markers and report the Word element name as `revisionType`. No logical move grouping or interpretation of unsupported revision text. |
| Standard comments | Partially supported and covered | Inspect comments, metadata, ranges, references, attached text, and main-document paragraph locations, including table anchors. Add exact body-paragraph selections; update plain bodies and delete standard records/markers with fresh handles. First creation writes the relationship and content type. |
| Modern/threaded comments | Explicitly unsupported for mutation | Replies, resolution, modern threading metadata, rich-body replacement, and cross-paragraph/table authoring are outside the current authoring boundary. Existing unrelated parts remain intact. |
| Simple/complex fields | Inspection implemented and covered | Main document and header/footer parts, including unused header/footer parts. Instructions and cached results are separate. PAGE, NUMPAGES, DATE, TOC are named; REF, PAGEREF, HYPERLINK and other codes remain `unknown` with their instruction preserved. |
| Field and TOC authoring | Partially supported and covered | Existing typed PAGE/NUMPAGES and new-paragraph TOC writers remain available. Results are dirty placeholders, not calculations. No arbitrary field-code edit, field removal, dynamic TOC generation, pagination, or native refresh. |
| Bookmarks and hyperlinks | Inspection and narrow hyperlink authoring implemented | Existing references are preserved. Text inside a hyperlink cannot use ordinary replacement. Ordinary text inside an existing bookmark can be edited without changing its markers; replacements crossing markers refuse. Bookmark authoring/repair is not added. |
| Text and batch edits | Partially supported and covered | Source-local text patches across compatible ordinary runs. Hidden boundaries, field content, and unsupported wrappers refuse. Native multi-cell operations are atomic; separate API operations retain their existing ordered failure/save behavior. |
| Paragraph and style edits | Partially supported and covered | Safe unrelated paragraphs and source-local properties remain editable. Paragraph removal/insertion cannot cross protected ranges or fields. Paragraph style changes preserve field markup. Text formatting cannot duplicate pending run-property revisions. |
| Table/content-control/header/footer edits | Partially supported and covered | Existing simple targets remain supported. Whole-cell replacement refuses local comment/field/wrapper content; unrelated simple cells can be edited. Header/footer writers refuse parts with pending revisions, including property changes. Unrelated parts survive body edits. |
| OPC serialization | Implemented and covered | Writes only selected package payloads, verifies the package, reparses XML, and checks operation-specific results. This is not full OOXML schema validation or a guarantee about every Office producer. |
| Protocol, N-API, and API | Implemented and covered | Existing inventory, generated descriptions/schemas, diagnostics, optional native gates, and lazy revisions/comments/fields groups. No extra agent tools or published dependency changes. |

“Supported” revision structure describes inspectable content, not permission to
perform every decision. Decisions additionally check container/run shape,
namespace context, fields, protected ranges, and comment removal. Simple record
counts can include malformed IDs; inspect `structure` and diagnostics before a
decision. Total/unsupported counts describe XML records, not logical edit groups.

Revision inspection covers the standard transitional/strict Word namespaces,
including move/custom-XML range starts **and ends**, `tblPrExChange`, property,
row/cell changes, and nested revisions. It also detects Word 2010 `conflictIns`,
`conflictDel`, and custom-XML conflict range starts/ends as unsupported. Unknown
extension vocabularies are preserved but not interpreted. Revision inspection
is main-document-only; it does not claim that headers, footers, notes, comments,
or other stories contain no revisions.

## Correctness fixes

- Ordinary replacement previously joined text across hidden deletions, range
  markers, or unsupported markup. It now returns `UNSUPPORTED_OPERATION` with
  `UNSAFE_TEXT_BOUNDARY`, instead of redistributing replacement across them.
  Text edits within unchanged enclosing comment/bookmark anchors stay supported.
- Paragraph deletion previously checked only children of the selected paragraph.
  It now checks enclosing ranges/fields and tracked properties with the existing
  source-range guard. Shared body insertion also checks its insertion point.
- Revision decisions now refuse enclosing move/conflict/bookmark/field ranges;
  they never reinterpret those structures as independent ordinary revisions.
- Revision inspection no longer misses orphan range ends or table property
  exceptions. The additive `revisionType` field identifies unsupported categories.
- Text-run splitting and plain comment-body updates refuse pending property
  revisions instead of duplicating or removing their history. Header/footer
  text/page writers refuse pending property/content revisions.
- Field inspection flags pending revisions in main/header/footer parts with
  `FIELD_PART_HAS_REVISIONS`, so external refresh cannot overlook those parts.
- Stray `instrText`, including instructions after a field separator, now produces
  `UNEXPECTED_FIELD_INSTRUCTION`. Existing unmatched/end/begin diagnostics remain.
  Unsafe field edits fail without an output artifact; cached results are never
  recalculated by these checks.

## Validation

Nine focused Rust regressions cover hidden boundaries, enclosing paragraph
ranges, insertion within a multi-paragraph TOC, unsupported revision discovery,
field instructions, protected revision decisions, tracked run formatting,
comment-body property history, and header/footer property history. Existing
ordinary decisions, duplicate IDs, stale handles, authoring, comment lifecycle,
fields/TOC, tables, styles, OPC, and targeting tests remain in place.

Broader checks used ten temporary generated fixtures, a real Word-authored
`test.docx`, and the latest Northstar Project Lighthouse proposal. Seventeen
native/API outputs passed ZIP CRC checks, XML parsing, internal relationship
target checks, identical package membership, and byte-identical unrelated
payloads. Ordinary text outputs matched the expected single XML substitution.
Field/comment/revision snapshots and anchors were checked, not only prose. The
consumer now declines its optional whole-document external refresh for documents
with comments or pending main-document revisions; count equality alone could not
establish preservation. Native bytes and the existing save lifecycle are retained.

Microsoft Word reopened three edited outputs without repair: the corrected
mixed fixture (moves, properties, comment, TOC, fields, links/bookmark, table),
the Word-authored template, and the real Northstar API result. Field-update
confirmation on the mixed fixture was declined to retain cached results. The
initial synthetic move used `delText`; correcting it to `t` removed its repair
prompt. The actual source and edited Northstar both retain the Finance comment.
These are open checks, not Word save round trips; the installed Word requires
activation for editing/saving. Google Docs reopen was not performed.

The bundled LibreOffice renderer produced ten Northstar page images, all
reviewed. Font and page-spacing differences from Word remain; this does not
establish identical layout across applications.

Local checks: DOCX 187, OPC 19, protocol 4, Node-crate Rust 3 tests; native Node
35 tests; generated declarations typecheck; formatting and strict Clippy pass.
Application validation and remaining risks are recorded in the consumer's
`docs/phase7_word_semantics.md`.

## Deferred work

Full structural/move/conflict decisions, modern comment threading/resolution,
field evaluation and refresh guarantees, TOC generation, layout/pagination,
other-story revision inspection, and unknown extension interpretation remain
outside this phase. Safe failure is intentional. Broader producer/schema and
cross-application save testing should continue before claiming full Word
compatibility.

Specification references: [move source example](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.movefromrangestart?view=openxml-3.0.1),
[Word 2010 conflict revisions](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.office2010.word.conflictdeletion?view=openxml-3.0.1).
