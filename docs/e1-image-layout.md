# E1 image positioning and layout

The existing picture parser already recognizes `wp:inline` and `wp:anchor`,
resolves images against their owning package part, and reads display extents.
The previous insertion/resizing and body-block picture handles were inline-only.
Replacement already changes only asset bytes. This pass extends those paths;
there is no second drawing parser, relationship model, dependency, or renderer.

## Typed operations and inspection

- `InsertPicture` keeps its paragraph placement and media/package handling. An
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
Deletion still removes a plain dedicated picture paragraph. When any paragraph
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
Inline↔floating conversion is deferred. Existing header/footer picture parsing
is reused unchanged; layout snapshots/handles and mutations remain main-part
only. Header/footer image authoring/update needs scoped targeting and is deferred.
There is no generic drawing editor, exact rendering, crop/effects editing, or
branding concept in Rust. Automatic workspace logos keep their inline behavior.

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
