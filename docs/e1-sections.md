# E1 / Pass 1: section editing

## Existing code and the previous boundary

`section.rs` already reads final body and paragraph-level section properties in
order. `SectionProperties` supplies size, orientation, margins, columns, and break
type. Writing was blocked by `main_section`, which requires exactly one final body
section. Legacy APIs keep that requirement.

The new operations reuse this parser, body placement, page setup patch builder,
simple header/footer text and PAGE-field helpers, relationship allocation,
content-type updates, source byte patches, OPC verification, and reopen checks.
There is no parallel XML tree or generic editing API. The already present
serde/serde_json packages serialize typed section facts.

## Typed operations and targeting

- `inspect_sections`: ordered section facts, variant references, effective content,
  owned/inherited state, and opaque handles.
- `insert_section_break`: existing body placement plus nextPage, continuous,
  oddPage, or evenPage. Adds a dedicated boundary paragraph and sets the following
  section's start type. This is a real section boundary, not a page break.
- `set_section_properties`: one selected section, existing Letter/A4 choices,
  orientation, partial top/right/bottom/left margins, different first page, break
  type, and page-number restart or continuation. Existing formats are preserved.
- `set_section_header_footer`: header/footer, default/first/even, and text, PAGE
  field, inherit, or unlink. Empty text clears text. First variants require
  differentFirstPage for activation.
- `set_odd_even_headers`: explicit document-wide setting. Even-variant edits cannot
  silently enable it or change global behavior.

Handles include a deterministic fingerprint of main XML, relationships,
header/footer bytes, and settings. Re-inspect after mutation. Malformed/stale
handles fail with PRECONDITION_FAILED; absent section targets with TARGET_NOT_FOUND;
unsafe/ambiguous structures with UNSUPPORTED_OPERATION. Failures emit no artifact.
Legacy base_revision remains opaque metadata, not a safety identity.

## Preservation

A missing reference inherits the same variant from the previous section.
Unlink copies effective content only when needed. Text/PAGE edits reuse an
unshared owned part; shared or inherited parts are copied before editing.
Changing one section keeps the following section's previous content by adding
its old effective reference (or an explicit blank part) when necessary. This can
turn the following section's implicit linkage into explicit ownership, reported
by inspection. Other variants keep their references. Unused parts/relationships
are retained; shared parts are never removed.

Page setup leaves other section XML byte-identical. Source patches retain body
content and unrelated package payloads. Header/footer postconditions check other
variants/sections and body content. Output is package-verified and reopened.
Inspection caches shared parts and uses one main source; it does not parse the
whole document once per section. Fingerprint cost grows with combined relevant
header/footer bytes, not section count times whole-document parses.

## Node and application exposure

N-API exports inspectDocxSections, executeDocxInsertSectionBreak,
executeDocxSetSectionProperties, executeDocxSetSectionHeaderFooter, and
executeDocxSetOddEvenHeaders, with typed declarations and engine-client methods.
Engine-client maps sections inspection to the native reader. Dynamic
`document.sections` and `document.headers_footers` groups use the existing handle
registry and one-save-per-run lifecycle. Load document.sections for header/footer
targets. Root tools and agent-core-v3 stay unchanged.

Continue using OPENSUITE_ENGINE_PATH. No package publishing, version changes,
pushes, paid model calls, or Pass 2.

## Tests and manual review

Run `node --test crates/opensuite-node/test/sections.test.mjs` to create:

- `/private/tmp/opensuite-e1-three-sections.docx`: portrait Letter with one-inch
  margins and first-page header; landscape Letter with half-inch side margins,
  table, unique default/even headers, PAGE field, and restart at 1; portrait Letter
  with inherited default header, independent footer, and continued numbering.
- `/private/tmp/opensuite-e1-three-sections.json`: reopened section facts.
- `/private/tmp/opensuite-e1-imported-updated.docx`: deterministic python-docx
  fixture edited only for section 2 orientation and section 3 footer.

Rust tests cover four break types, independent properties, empty section
properties, variants, inherit/unlink, shared parts, stale/invalid targets, unsafe
boundaries, unchanged unrelated bytes, and reopen. Node exercises the real binary
and imported document; app tests cover bridge dispatch, dynamic groups, handle
expiry, and one final persisted version. ZIP payloads and relationships are checked.
Manual Word, Google Docs, and LibreOffice review remains required. Google Docs is
not automated; these checks do not establish visual interoperability.

## Known limitations

- Simple text paragraphs and existing simple PAGE fields only; no rich header
  layouts, tables, floating objects, or general fields.
- Existing numbering formats preserved/inspected; new formats not authored.
- Odd/even editing requires an existing settings part with a closing tag.
- Tracked, nested, missing-final, or duplicate section properties fail safely.
  Boundaries with namespace declarations on paragraph ancestors are rejected.
- Section header/footer authoring supports transitional DOCX; strict DOCX and
  copying header/footer parts with their own relationships are rejected.
- Break insertion adds a blank boundary paragraph. Word may place continuous
  breaks on a new page when page geometry changes.
- No rendering, exact page counts, advanced long-document features, or Pass 2.
