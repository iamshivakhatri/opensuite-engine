# E1 Pass 3: Layout inspection

`inspect_docx_layout` and N-API `inspectDocxLayout(bytes, options?)` return a
read-only `LayoutSnapshot` with `kind: structural`. The application client exposes
typed results and a lazy `document.layout` group with `document.inspect_layout`
and `document.render_layout`. Inspection does not save or create a document version.

## Structural facts

The existing section parser supplies page size, orientation, margins, header/footer
distances, gutter, columns, section type, numbering, and section handles. Usable
width/height subtract the corresponding margins. Missing inputs, negative margins,
and unresolved nonzero gutter placement remain unknown rather than guessed.
Nonpositive derived geometry is diagnosed.

Direct-body paragraphs and tables map to the section ending at the next declared
boundary. Images inherit ownership from their enclosing body block where known.
Effective paragraph formatting reuses the existing style resolver: pageBreakBefore,
keepNext, keepLines, widowControl, spacing, line spacing, and indentation. Explicit
page breaks and paragraph section boundaries are counted separately.

Existing table inspection supplies preferred width/mode, grid widths, cell margins,
row count and complexity; layout inspection also reads row heights and cantSplit.
Existing picture semantics supply inline/anchored classification, relationship and
asset references, display dimensions and intrinsic PNG/JPEG dimensions. Main XML
and styles are loaded once; image relationships and intrinsic dimensions are reused.
No second document tree or paginator is introduced.

`TABLE_WIDTH_EXCEEDS_PAGE` and `IMAGE_WIDTH_EXCEEDS_PAGE` identify structural width
inputs exceeding usable width. These warnings do not prove final overflow. Complex
tables and anchored drawings carry diagnostics; actual wrapping requires rendering.

## Bounds and limitations

Default output contains summaries, with no block detail. Collections and diagnostics
are capped at 64; optional `blockLimit` is 0–100, with `blockOffset` and `sectionIndex`.
Document counts remain global; selected-section summaries and block details are
filtered. `hasMoreBlocks`, `matchingBlockCount`, and `truncated` identify omissions.
Paragraph patterns cover direct-body paragraphs; cell paragraph controls and revision
visibility are not modeled as final rendered layout. Font metrics, automatic breaks,
line wrapping, and exact block-to-page positions are unavailable structurally.

## Optional rendered output

The engine-client `renderDocxLayout` adapter uses Node standard-library subprocesses,
a separate temporary DOCX and isolated LibreOffice profile, PDF export, and `pdfinfo`.
It returns `kind: rendered`, provider `libreoffice-pdf`, PDF page count and dimensions
for the first 64 pages. Each subprocess has a 30-second timeout; temporary files are
removed. Missing tools or failures return diagnostics with no estimated fallback.
An optional PDF is available to callers for manual review.

This count is exact for the produced LibreOffice PDF, not a guarantee of identical
Microsoft Word pagination. Exact block-to-page mapping remains unavailable; PDF
text matching would be ambiguous and UNO integration would add unnecessary scope.
Ordinary Rust and Node structural inspection requires neither external tool.

## Verification

Focused tests cover geometry, section ownership, inherited controls, explicit breaks,
row controls, oversized tables/images, bounded 1,000/5,000-paragraph documents, and
unchanged imported DOCX bytes. Application tests verify lazy loading and zero saves.
LibreOffice/PDF smoke results: simple report 1 page; three-section report 3 pages;
explicit-pagination fixture 2 pages. Counts were independently checked with pdfinfo.
Structural debug-native timings: approximately 1.7–3.4 ms small and 9.5–9.9 ms for 1,000
paragraphs, measured locally rather than treated as a benchmark guarantee.

Manual artifacts are generated under `/private/tmp/opensuite-layout-*`: DOCX,
structural JSON, and PDF/rendered JSON for the simple and three-section fixtures;
DOCX/PDF/rendered JSON for explicit pagination; DOCX/JSON for wide content.
These artifacts are not committed. User review in Word remains useful.
