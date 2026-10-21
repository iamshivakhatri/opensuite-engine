# Native image positioning and layout

The existing picture parser already recognizes `wp:inline` and `wp:anchor`,
resolves images against their owning package part, and reads display extents.
The previous insertion/resizing and body-block picture handles were inline-only.
Replacement already changes only asset bytes. This pass extends those paths;
there is no second drawing parser, relationship model, dependency, or renderer.

## Typed operations and inspection

- `InsertPicture` reuses paragraph placement and media/package handling. An
  optional size accepts width, height, or both. One dimension derives the other
  with integer arithmetic. Neither retains intrinsic 96-DPI sizing, capped at
  the existing 6.5-inch default width without automatic upscaling. Explicit
  dimensions may upscale. Insertion creates a dedicated image paragraph.
- Optional `InsertPicture.auto_fit` (Node `autoFit: true`) fits an inline image to
  the section at the insertion point. Declared page width/height are authoritative
  for portrait and landscape pages. Deduct nonnegative margins, a conservative
  gutter allowance on both axes, equal-column spacing or the narrowest declared
  unequal column, and resolved default paragraph indents and before/after spacing.
  New paragraphs inherit document defaults, not adjacent paragraph formatting.
  Choose the limiting axis with integer arithmetic, preserve intrinsic aspect
  ratio (round down by less than one EMU), and never upscale from 96-DPI intrinsic
  size. Height is a full-section bound, never estimated remaining page space.
  Missing/invalid/impossible geometry fails with `INVALID_PAGE_GEOMETRY`;
  ambiguous properties and inherited exact line height fail safely as unsupported.
  Multiple equal columns require declared spacing; unequal columns require
  positive widths and spacing between columns that fit the page text area.
  Automatic fit cannot combine with explicit size or floating layout. Omission
  keeps the legacy width cap and sizing behavior.
- Optional inline `alignment` accepts left/center/right. It authors `w:jc` inside
  `w:pPr` on the actual picture paragraph, including paragraphs without text, and
  creates no extra paragraphs. Omission keeps the previous paragraph XML.
  Alignment with floating layout is refused; anchor positioning stays separate.
- An optional floating layout requires both position axes. Each axis has exactly
  one start/center/end alignment or signed offset. Horizontal references are
  page, margin, or column; vertical references are page, margin, or paragraph.
  Paragraph-relative vertical placement requires an offset. Start/end mean
  left/right horizontally and top/bottom vertically.
- Wrapping supports square, top-and-bottom, behind text, and in front of text.
  Text distances accept top/bottom/left/right. Geometry uses English Metric
  Units (EMU), with 914,400 per inch, consistent with existing image operations.
- `SetPictureSize` supports inline and anchored simple images; both dimensions
  explicitly supplied set exact size. One dimension preserves the current display
  aspect ratio, including after an asset with different intrinsic dimensions replaces it.
- `SetPictureLayout` patches supported anchored images. Omitted properties remain
  untouched. Same-kind positions patch their existing text; changing alignment
  to offset replaces only that property child. Imported position attributes and
  sibling extensions remain intact. Unreplaceable imported properties fail safely.
- Layout snapshots add image handles, semantic main-part paragraph indexes,
  body-block and section ownership, display aspect ratio, intrinsic dimensions,
  asset/relationship facts, and anchor position/wrap/text-distance facts.
  Imported tight/through wraps keep their actual names and are not authored.
  Wrap-level distance overrides take precedence in inspection and are patched
  alongside anchor distances. Changing wrap type with imported wrap-level
  distances/extensions is rejected rather than discarding them.
- New opaque image handles reuse the section/artifact fingerprint convention.
  They cover simple images in body paragraphs and table cells. A changed source
  fingerprint or invalid index fails safely; legacy inline body-block handles
  remain available. Inspect again after each edit. Matching identical artifact
  contents has the same fingerprint; this is a content check, not persistent history.
- Width and horizontal-offset diagnostics use declared section dimensions and
  margins. These are structural checks, never exact rendered coordinates.

## Preservation and unsupported cases

Resizing patches only the drawing and picture-transform extent attributes.
Moving patches only requested position properties, wrap, behind-text flag, or
text-distance attributes. Unknown extensions, effects, crop, alternate text,
IDs, anchor flags, and unrelated document/package parts stay intact.
Deletion removes a plain dedicated body picture paragraph. Cell picture deletion
keeps the paragraph so a cell always retains valid paragraph content. When any paragraph
properties exist, it removes only the drawing, keeping the paragraph/run and all
properties, including `w:sectPr`, section boundaries, and property history.
The shared protected-source guard permits only that selected drawing: enclosing
ranges/fields/revisions, mixed content, and protected structures within the drawing
fail safely. Media and relationships remain even when unused. Verification reopens
output and requires exactly the intended source removal and unchanged body order.

Replacement changes only the media part; dimensions, positioning, wrapping,
and document XML remain unchanged. Assets shared by multiple images, legacy
VML references, or other package parts (including headers) are rejected.

Grouped/mixed drawings, ambiguous containers/positions, linked assets for resize,
unsupported reference/alignment combinations, simple-position anchors, and
polygon wraps fail safely. Tight/through images remain inspectable; resizing a
simple imported image can preserve its unsupported wrapping without editing it.
Inline↔floating conversion is deferred. E3 reuses header/footer picture parsing
for scoped inline insertion and binary retrieval (below). Existing layout snapshot
handles and resize/replace/delete/layout mutations remain main-part only.
There is no generic drawing editor, exact rendering, crop/effects editing, or
branding concept in Rust. Automatic workspace logos keep their inline behavior.

## E2 table-cell picture insertion

The same Rust `InsertPicture` and native `executeDocxInsertPicture` now insert
inline PNG/JPEG images into an exact cell of a direct body table. Inspection with
`focus: { kind: "tables" }` adds `rows[].pictureCellHandles[]` (Rust
`picture_cell_handles`), in column order. These opaque handles combine the
existing table/row/column identity with the existing picture/section source
fingerprint. Copy a fresh handle after every edit; legacy unstamped `cellHandles`
are not picture insertion targets. Tables inside content controls expose an empty
picture-handle list. Their presence does not shift a later direct table's picture
target. Changed source or relationships invalidate prior picture-cell handles.

```js
const snapshot = await engine.inspectDocx(bytes, { focus: { kind: "tables" } });
const handle = snapshot.tables.items[0].rows[1].pictureCellHandles[1];
const result = await engine.executeDocxInsertPicture(bytes, {
  imageBytes,
  placement: { kind: "tableCell", handle, position: "end" },
  autoFit: true,
  alignment: "center",
});
if (!result.result.ok) throw new Error(JSON.stringify(result.result.diagnostics));
bytes = result.output;
const layout = JSON.parse(await engine.inspectDocxLayout(bytes, {}));
// layout.images contains the existing fresh image handles and display dimensions.
```

`position` is `start` or `end` (default `end`). A dedicated picture paragraph goes
before the first or after the last existing cell paragraph. Existing text,
paragraphs, numbering, shading, borders, cell properties, section properties, and
unrelated XML remain unchanged. Multiple cell paragraphs and repeated picture
insertions are supported. Alignment and explicit sizing use the existing inline
writer: one explicit dimension preserves aspect ratio; both specify exact size.
Explicit sizing can upscale. Cell placement always requires inline layout;
floating layout is refused even without autoFit. Existing body placements and
binary input/output remain compatible. Rust callers wrap existing
`ParagraphPlacement` with `.into()` or `PicturePlacement::Body`; cell placement is
`PicturePlacement::TableCell { handle, position: CellPicturePosition::End }`.

Cell `autoFit` uses the existing integer extent calculation, preserves the
intrinsic aspect ratio, and never upscales. It requires one positive grid width
per column. Every declared cell width must be absolute (`dxa`, meaning twips)
and agree with its grid column. A declared absolute table width must agree with
the grid sum; an omitted width or `auto` width of zero may use that explicit grid.
The grid plus any nonnegative table indent must fit the insertion section's
page/column width. Width is the selected column minus left/right cell margins.
Cell margins override table margins side by side; without either, OOXML defaults
are 108 twips left/right and zero top/bottom. Vertical margins reduce the existing
full-section height bound. Resolved default paragraph indents and before/after
spacing are then deducted by the existing section fitter. This does not estimate
remaining page space or remaining height beside existing cell text.

Supported structures are simple rectangular direct body tables with ordinary
rows/cells and one or more direct paragraphs in the selected cell. Merges
(`gridSpan`, `vMerge`, or legacy `hMerge`), nested tables, omitted grid cells,
wrapped rows/cells, tracked table property changes, and protected insertion
boundaries are refused. AutoFit additionally refuses missing/duplicate/invalid
geometry, conflicting or percentage widths, table styles (including an inherited
default table style), floating or bidirectional tables, logical start/end margins,
nonzero cell spacing, vertical text, no-wrap/fit-text cells, row property exceptions,
and exact-height rows. A fixed row's remaining height depends on its existing
content. These cases return `INVALID_CELL_GEOMETRY`; unsupported structure and
stale-target failures retain the existing structured diagnostic convention.
Explicit sizing needs no cell-width calculation, but still applies structural,
source-handle, and protection guards.

All images reuse the existing media allocator, relationship writer, XML generator,
layout inspector, replacement, resize, and deletion operations. Reopen verification
checks the exact output main XML, insertion paragraph and owning cell, dimensions,
alignment, embedded payload, and requested layout. Cell deletion removes only the
drawing and keeps its paragraph. No new binary insertion export or image engine.

### E2 verification and limits

The focused Rust cell tests and native `test/cell-pictures.test.mjs` generate their
DOCX inputs in memory. They cover populated/multi-paragraph cells, separate and
repeated cell insertion, start/end order, margins/defaults, no upscaling, explicit
sizes, replacement, proportional resizing, deletion, media relationships, source
preservation, stale/invalid targets, wrapped-table indexing, and refused geometry.
Existing image tests cover portrait/landscape/section/column bounds, tall images,
all four wraps, supported position references, imported layout mutation, and
section-preserving deletion. The combined native fixture retains an imported
LibreOffice drawing with an unfamiliar extension byte for byte through edits;
its other original package payloads are preserved.

Native tests regenerate `/private/tmp/opensuite-e2-cells.docx` and
`/private/tmp/opensuite-e2-combined.docx`, alongside the existing A1 samples.
LibreOffice rendered cells, combined, wide, tall, and logo fixtures into
`/private/tmp/opensuite-e2-render/`: six pages across five PDFs. All pages were
visually reviewed. The cell fixture retains its captions, alignment, and image
borders inside the table. The combined fixture opens over two pages; page-relative
floating images appear at declared page edges, including the top margin. This is
renderer evidence for these fixtures, not a guarantee of collision-free floating
placement. No permanent binary fixtures or paid models were added.

Before E3, remaining image gaps include header/footer authoring, floating cell
insertion, merged/nested or styled-table automatic fitting, inline/floating
conversion, exact remaining-page/cell-space fitting, and collision detection.
Existing format/wrapping/crop/effect limits remain. Word and Google Docs review
of E2 outputs is still pending. No application TypeScript, version/pin, publication,
or push changes are included.

## E3 header/footer insertion and embedded image access

`InsertPicture.placement` adds `HeaderFooter { target, kind, variant }`. Node
extends the same `executeDocxInsertPicture` method with
`{ kind: "header" | "footer", handle: sectionHandle, variant: "default" | "first" | "even" }`.
The handle must come from fresh section inspection. Existing body/cell inputs
retain their behavior. The operation appends a dedicated inline paragraph after
existing direct paragraphs, retaining their exact XML, formatting, fields, and
imported drawings. Floating header/footer insertion is refused.

Missing parts use the existing section header/footer creation path, including
its relationship/content-type allocation and preservation of the following
section's former empty content. Existing shared parts and inherited parts are
refused: isolate/unlink them explicitly with the section API first. That API
still refuses copying parts that own relationships, so shared image-rich headers
remain a supported inspection case, not an automatic isolation workflow. First
and even variants require separately enabled first-page/document-wide odd/even
behavior. Insertion never toggles those flags or changes an existing variant link.
The section inspector's `supported` flag still describes its simple text/PAGE
writer; it is not an image-insertion or retrieval support flag.

Header/footer insertion fits by default, even when `autoFit` is omitted. It uses
96-DPI intrinsic dimensions and the shared integer proportional sizing code,
without upscaling. Width uses page width minus side margins and gutter, without
body-column narrowing. Height uses top margin minus header distance, or bottom
margin minus footer distance, less a 120-twip baseline allowance. Resolved default
paragraph indents/spacing are deducted through the same fitting helper as E1/E2.
Explicit size retains proportional/exact semantics but must fit those bounds;
`autoFit` plus explicit size is refused. Optional left/center/right alignment
uses the existing paragraph writer. These are bounds for the new image, not a
calculation of space remaining after other header/footer content. Rendering is
still required for crowded layouts.

Direct paragraph containers are supported. Header/footer tables or root wrappers,
tracked changes, malformed/crossing field/range structures, duplicate relationship
IDs, strict OOXML authoring, ambiguous geometry, inherited exact line heights,
shared/inherited parts, and inactive first/even variants fail safely. The shared
media allocator avoids existing names, and each picture relationship belongs to
the actual owning part; headers outside `/word/` use an absolute package target.
Reopen checks verify the exact patched source, alignment, geometry, image bytes,
and package validity. Untouched package payloads remain unchanged.

Read-only Rust `inspect_docx_images(bytes, offset, limit)` / Node
`inspectDocxImages(bytes, { offset?, limit? })` returns JSON facts: owning part,
PNG/JPEG format, intrinsic pixel size, displayed EMU size, bounded alt text,
inline/floating kind, anchor/wrap facts, media part/relationship, payload size,
and a fresh retrieval handle. Main-document images (including table cells) come
first, then all header/footer parts in package-name order, including unused parts.
Default 20, maximum 100 records; media outside the requested page is not loaded.
Unsupported drawings/references have diagnostics and a null retrieval handle.
These handles are separate from body/layout mutation handles.

Rust `read_docx_embedded_image(bytes, handle)` / Node
`readDocxEmbeddedImage(bytes, { handle })` returns original embedded PNG/JPEG bytes,
without decoding, transcoding, or changing the DOCX. The Node response contains
`result` and, on success, `imageBytes: Buffer`; failure has structured diagnostics
and no image bytes. A successful read reports no changes. Retrieval loads only
the selected media. Shared media can be read from each supported referencing
location. The byte limit is checked from the ZIP entry size before decompression:
20 MiB per image and 64 million intrinsic pixels. The existing dimension parser
is reused; this is not a full raster decoder or image integrity validator.

Handles use the existing deterministic FNV-1a convention over owning part name,
local XML, owning relationship bytes, selected media name and bytes, plus the
picture index. Changes to any of those invalidate the handle. Unrelated parts
need not invalidate it. Equal content produces equal handles; these are local
source checks, not durable IDs or cryptographic security tokens. Linked/internal
or external references, embed+link combinations, missing/wrong/duplicate image
relationships, malformed/unsupported media, format/content-type mismatch,
ambiguous drawings, stale handles, and oversized images are refused. Rich
DrawingML is preserved during unrelated insertion; retrieval requires one direct
picture/blip and does not interpret effects, crops, OCR, or image meaning.

The binary method is explicitly `backendOnly: true`, `jsonInput: false`, and
`jsonOutput: false` in generated native metadata. It must never become a raw
model-callable tool or put pixels/base64 in JSON capabilities. `inspect_images`
remains a normal JSON read; declarations, schemas, inventory and loader include
both methods.

### E3 verification and later application integration

217 DOCX, 20 OPC, 4 protocol, and 3 Node-crate Rust tests pass; 51 native Node tests
cover E1/E2 regressions, variants, missing parts, PAGE and formatting preservation,
section sharing/inheritance, relationship ownership, distinct/shared media,
body/cell/header/footer retrieval, stale source/relationships/media, JPEG,
linked/malformed/duplicate relationships, payload/pixel bounds, save/reopen,
and preservation of imported floating drawings and unfamiliar extensions.
Workspace compilation, strict release-crate/CLI Clippy, formatting, SDK/schema
checks, and diff checks pass.

Four final temporary LibreOffice PDF renders (footer with PAGE and formatting,
three-page first/even/default headers and footers, two independent sections,
and a new header with an imported floating body picture) produced seven pages.
All pages were visually inspected: centered images retain aspect ratio and full
borders, text/formatting and page numbers survive, sections remain separate,
and the imported float remains on the right with text wrapping. A separate
three-page experiment with an even header but no explicit even footer omitted
the even header in this LibreOffice build; adding explicit footer variants made
it render. The engine preserves the requested links rather than compensating
for renderer behavior. Microsoft Word and Google Docs E3 review remain untested.

Later app integration must load a build containing E3; extend the engine-client
picture placement type with the header/footer alternatives and fresh section
handles; add the decoded inspection output type and bounded read wrapper; and
honor `backendOnly` as well as the JSON transport flags during tool discovery.
The backend should resolve approved retrieval handles against current artifact
bytes and pass the returned Buffer to the existing vision attachment pipeline.
Pixels stay out of model tool JSON and capability responses. Re-inspect after
relevant edits; reuse the current save/verification lifecycle for insertion.
No app TypeScript, model calls, publication, version/pin updates, or push are part
of E3. Header/footer resize/replace/delete, floating authoring, rich DrawingML
editing, richer cell fitting, and rendered collision/remaining-space fitting
remain outside this pass.

## Bridge and product

N-API extends existing insertion/size/replacement and adds
`executeDocxSetPictureLayout`. The engine-client exposes compact typed image
operations and extends layout snapshot types. Product `document.rich_content`
lazily exposes layout updates alongside existing resize tools; dynamic
`document.inspect_layout` in the page-layout group provides facts and registers fresh handles.
The existing working-byte lifecycle rejects unregistered/stale handles and saves
one verified version at the run boundary. Binary insertion/replacement stay
programmatic. Agent Core V3, npm pins, and `OPENSUITE_ENGINE_PATH` are unchanged.

## Phase A1 application integration

The existing `executeDocxInsertPicture(bytes, operation)` now accepts optional
`autoFit: boolean` and `alignment: "left" | "center" | "right"` fields through the
same binary-buffer input. Example operation:

```js
{ imageBytes, placement: { kind: "end" }, autoFit: true, alignment: "center" }
```

Application integration requires forwarding those two fields in the existing
engine-client picture insertion type/wrapper and choosing them in the calling
image workflow. Do not send width/height or floating layout with `autoFit: true`.
Alignment may accompany explicit inline sizing. Continue using verified output
bytes and fresh handles after edits. Generated SDK declarations and operation
schemas include the options; capability IDs and export names are unchanged.
Rust `InsertPicture` callers must supply `auto_fit` and `alignment` explicitly
(`false`/`None` preserve old behavior). The CLI continues its existing defaults.
No application TypeScript, engine pin, version, or publication changed in A1.

## Phase A1 verification

Focused fixtures cover wide/tall/logo/photo proportions, explicit legacy sizing,
landscape pages, margins/gutters, equal/unequal columns, default-style indents,
section selection on both sides of a boundary, and textless paragraph alignment.
Deletion checks retain section/property history/unknown paragraph XML and every
unrelated part, and refuse enclosing ranges, fields, revisions, mixed content,
and protected structures inside a drawing. Existing inline/floating drawings
remain byte-identical after automatic insertion. All outputs reopen through the
package verifier; native deletion/reinsertion retains section count and valid
image relationships. No large permanent fixtures were added.

`node --test test/pictures.test.mjs` regenerates six temporary
`/private/tmp/opensuite-a1-{wide,tall,logo,landscape,margins,columns}.docx` files.
LibreOffice rendered each final fixture to one PDF page; every page was visually
checked for expected size/alignment and intact image borders. The tall image
initially clipped at the bottom because the inherited paragraph spacing consumed
page height; the final fixture deducts that spacing and shows all four borders.
Unequal columns, section boundaries, and deletion preservation have structural
verification only. Microsoft Word/Google Docs remain unverified for A1.

## Evidence and manual artifacts

Focused Rust checks cover sizing, all four supported wraps, stale/invalid targets,
imported extensions/crop/single quotes, unsupported draws/wraps, wrap-distance
overrides, shared-header/VML assets, replacement, package reopen, and unchanged
unrelated parts. Node dogfood covers inline/floating/page-centered images,
portrait/landscape/portrait ownership, oversized width, and many-image timing.
Bridge and API tests cover typed operations, lazy exposure, inspected-handle
validation, stale rejection, and one persisted version.

`tests/fixtures/libreoffice-floating-image.docx` was saved by local LibreOffice
from the deterministic floating dogfood. Its resize test requires exactly two
width and two height value replacements in document XML, with every other part
unchanged. Synthetic imported XML additionally checks unknown extension/crop
preservation during position updates. Microsoft Word/Google Docs review is pending.
The two PNG fixtures are deterministic geometric illustrations (480×240 and
240×480); no paid model calls were used.

Run `node --test test/image-layout.test.mjs` in `crates/opensuite-node` after its
local build to regenerate:

- `/private/tmp/opensuite-image-inline.docx`
- `/private/tmp/opensuite-image-floating.docx`
- `/private/tmp/opensuite-image-centered.docx`
- `/private/tmp/opensuite-image-multi-section.docx`
- `/private/tmp/opensuite-image-imported-update.docx`
- `/private/tmp/opensuite-image-replacement.docx`

Each has a matching `.json` inspection. A local LibreOffice PDF smoke render of
`opensuite-image-floating.docx` showed the image on the right and text flowing
beside it, with no overlap/clipping. The PDF is
`/private/tmp/opensuite-image-floating.pdf`.
A debug build measured about 2 ms small inspection, 5 ms layout update,
21 ms inspection and 46 ms resize with 40 images. Timings are rough local samples,
not a benchmark guarantee.

No engine package is published, no version is bumped, and no branch is pushed.
