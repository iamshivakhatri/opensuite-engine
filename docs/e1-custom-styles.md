# E1 Pass 2: custom Word styles

`create_style` and `update_style` author real paragraph or character styles.
They use explicit stable style IDs and the existing `StyleSheet`, inheritance
traversal, formatting patches, and source-aware XML writers. There is no second
style model. `WordStylePatch` reuses `TextFormattingPatch` and
`ParagraphFormattingPatch`: absent means unchanged; `Clear` removes a declared
property and restores inheritance.

Creation requires a unique ID and display name. Updates cannot change style type
or default selection. Parent IDs must exist, have the same style type, and form
no cycle. Paragraph `next` targets must be existing paragraph styles; a style may
name itself as its next style. Character styles support run properties and
inheritance only. Deletion is deferred because references can live in several
parts, numbering, and other styles.

Only the affected source spans in the relationship-discovered styles part change.
Unknown attributes, elements, docDefaults, latentStyles, table styles, and other
package parts remain untouched. Explicit fonts replace the relevant theme font
references; untouched theme references remain unresolved and preserved. Candidate
styles parse and resolve before saving, followed by package verification and
reopen checks. Documents without a styles relationship fail safely.

Node exposes `executeDocxCreateStyle` and `executeDocxUpdateStyle`. Inputs include
`styleId`, `styleType`, `name`, `basedOn`, `next`, supported formatting fields, and
an optional `clear` array. IDs select parents and next styles. Apply paragraph
styles with the existing `set_paragraph_style` operation using the display name.
Colors use six hex digits without `#`; sizes use half-points (32 = 16 pt), spacing
and indentation use twips (20 = 1 pt). Rust supports its existing line-spacing
patch, while Node intentionally exposes only the smaller field set in `StyleInput`.

OpenSuite exposes these two tools through dynamic `document.styles`, using the
existing run-local bytes and one-save lifecycle. Agent Core V3 and personal
StyleProfile/BrandProfile behavior are unchanged. The npm version remains 0.1.3;
local testing uses `OPENSUITE_ENGINE_PATH`.

The existing bounded snapshot now includes unused styles, next IDs, declared and
effective formatting (including defaults), and usage counts. It sorts style IDs
before selecting the bounded result and reports truncation.

Run `node --test crates/opensuite-node/test/custom-styles.test.mjs` to produce:

- `/private/tmp/opensuite-e1-custom-styles.docx` and its inspection JSON
- `/private/tmp/opensuite-e1-imported-google-docs-table.docx`
- `/private/tmp/opensuite-e1-imported-imported-style-report.docx`

The report fixture is a pre-existing programmatic scientific-paper draft copied
from OpenSuite's local dogfood document. Its existing title paragraph is styled.
The Google Docs fixture contains only empty body paragraphs, so that check adds
one paragraph, then styles it. Both checks compare unrelated stylesheet bytes and
all untouched ZIP entry payloads byte-for-byte. Neither check replaces manual
Word/Google Docs/LibreOffice review, which remains pending.

The abstraction audit found no relevant Pass 1 section issue. Shared paragraph
and run property writers were extracted, stylesheet relationship discovery was
reused, and the existing attribute editor was corrected for single quotes and
whitespace around `=`. No dependencies, generic XML editing API, publishing,
version bump, model calls, or Pass 3 work were added.
