// Generated from Rust N-API types. Do not edit.
import { Buffer } from 'node:buffer'

export interface AddCommentInput {
  target: TextTargetInput
  /** @schema {"minLength":1,"maxLength":32000} */
  text: string
  /** @schema {"minLength":1} */
  author: string
  initials?: string
  /**
   * Explicit ISO timestamp. The engine-client supplies the current time when omitted.
   * @schema {"description":"ISO timestamp, e.g. 2026-10-04T12:00:00Z","x-client-default":"currentUtc"}
   */
  date: string
}

export interface AffordanceOutput {
  capability: string
  supported: boolean
  reason?: string
}

export interface BodyBlockOutput {
  handle: string
  kind: string
  text?: string
  styleName?: string
  headingLevel?: number
  tableHandle?: string
  rowCount?: number
  columnCount?: number
  headerTexts?: Array<string>
  picture?: PictureOutput
}

export interface BodyBlockPageOutput {
  page: InspectionPageOutput
  items: Array<BodyBlockOutput>
}

export interface CommentInspectionInput {
  offset?: number
  /** @schema {"minimum":1,"maximum":100} */
  limit?: number
}

export interface ContentControlTargetInput {
  tag?: string
  alias?: string
  /** @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."} */
  occurrence?: number
}

/** Creates a valid empty DOCX without touching the filesystem. */
export declare function createBlankDocx(): Buffer

export interface CreateTableInput {
  rows: Array<Array<string>>
  /** @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."} */
  placement: ParagraphPlacementInput
  baseRevision?: string
}

export interface DeleteCommentInput {
  handle: string
}

export interface DeleteNoteInput {
  /** @schema {"minLength":1} */
  handle: string
}

export interface DeletePageBreakInput {
  handle: string
  baseRevision?: string
}

export interface DeleteParagraphInput {
  target: TextTargetInput
  baseRevision?: string
}

export interface DeletePictureInput {
  handle: string
  baseRevision?: string
}

export interface DeleteTableColumnInput {
  /** @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."} */
  table: TableTargetInput
  columnHeader?: string
  columnHandle?: string
  baseRevision?: string
}

export interface DeleteTableInput {
  /** @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."} */
  table: TableTargetInput
  baseRevision?: string
}

export interface DeleteTableRowInput {
  /** @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."} */
  table: TableTargetInput
  /** @schema {"description":"Prefer kind=label with text and occurrence when needed, or kind=index with expectedFirstCellText. Use a handle only when these cannot express the row. Older firstCellText + occurrence remains supported."} */
  row: DeleteTableRowTargetInput
  baseRevision?: string
}

export interface DeleteTableRowTargetInput {
  /** @schema {"enum":["header","label","index"]} */
  kind?: string
  /** @schema {"description":"First-cell row label for kind=label."} */
  text?: string
  /** @schema {"description":"Zero-based row position for kind=index."} */
  index?: number
  /** @schema {"description":"Required for kind=index; prevents a shifted row from being edited."} */
  expectedFirstCellText?: string
  firstCellText?: string
  /** @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."} */
  occurrence?: number
  handle?: string
}

export interface DeleteTrackedTextInput {
  target: TextTargetInput
  /** @schema {"minLength":1} */
  author: string
  /** @schema {"description":"UTC ISO timestamp","x-client-default":"currentUtc"} */
  date: string
}

export interface DiagnosticOutput {
  code: string
  severity: string
  message: string
  reasonCode?: string
  operation?: string
  targetHandle?: string
  targetType?: string
  targetDescription?: string
  updateIndex?: number
  candidateCount?: number
  candidateTargets?: Array<string>
  requiredSelectorKind?: string
  retryable?: boolean
  recoveryKind?: string
}

export interface DocxOperationOutput {
  id: string
  kind: string
  nodeExport: string
  description: string
  inputSchema?: string
  jsonInput: boolean
  jsonOutput: boolean
}

/** Accept one supported insertion or deletion using its fresh inspect_tracked_changes handle. Keeps inserted content or removes deleted content. For replacement, decide both records individually and re-inspect after each successful decision. Complex revisions are unsupported. */
export declare function executeDocxAcceptRevision(input: Buffer, op: RevisionDecisionInput): Promise<ExecuteDocxReplaceTextOutput>

/** Attach a standard Word comment to exact text in one ordinary body paragraph, including simple formatted runs. Author is explicit; the client supplies current UTC when date is omitted. Wrappers, fields, revisions, cross-paragraph ranges, and threaded comments are unsupported. */
export declare function executeDocxAddComment(input: Buffer, operation: AddCommentInput): Promise<ExecuteDocxReplaceTextOutput>

/**
 * Create a reusable real Word paragraph or character style. No default-style authoring. basedOn and next use IDs; apply paragraph styles with set_paragraph_style using the display name.
 * @schema {"required":["styleId","styleType","name"]}
 */
export declare function executeDocxCreateStyle(input: Buffer, operation: StyleInput): Promise<ExecuteDocxReplaceTextOutput>

/** Create a table at a paragraph placement. */
export declare function executeDocxCreateTable(input: Buffer, operation: CreateTableInput): Promise<ExecuteDocxReplaceTextOutput>

/** Delete a standard comment and its markers using a fresh inspect_comments handle, preserving the selected document text. Inspect again after every edit. */
export declare function executeDocxDeleteComment(input: Buffer, operation: DeleteCommentInput): Promise<ExecuteDocxReplaceTextOutput>

/** Delete an ordinary simple footnote/endnote and its body reference using a fresh inspect_notes handle. Remaining IDs are preserved. Protected references and complex/malformed notes refuse. Re-inspect after every mutation. */
export declare function executeDocxDeleteNote(input: Buffer, op: DeleteNoteInput): Promise<ExecuteDocxReplaceTextOutput>

/** Delete a page break identified by an opaque body_blocks handle. */
export declare function executeDocxDeletePageBreak(input: Buffer, operation: DeletePageBreakInput): Promise<ExecuteDocxReplaceTextOutput>

/** Delete a paragraph identified by exact text target (occurrence zero-based when ambiguous). */
export declare function executeDocxDeleteParagraph(input: Buffer, operation: DeleteParagraphInput): Promise<ExecuteDocxReplaceTextOutput>

/** Delete a picture identified by an opaque body_blocks handle. */
export declare function executeDocxDeletePicture(input: Buffer, operation: DeletePictureInput): Promise<ExecuteDocxReplaceTextOutput>

/** Delete a table. */
export declare function executeDocxDeleteTable(input: Buffer, operation: DeleteTableInput): Promise<ExecuteDocxReplaceTextOutput>

/** Delete a table column by header or column handle. */
export declare function executeDocxDeleteTableColumn(input: Buffer, operation: DeleteTableColumnInput): Promise<ExecuteDocxReplaceTextOutput>

/** Delete a known table row using table + row. Prefer row {kind: 'label', text, occurrence?} or a checked index; a handle is only needed when these cannot express the row. */
export declare function executeDocxDeleteTableRow(input: Buffer, operation: DeleteTableRowInput): Promise<ExecuteDocxReplaceTextOutput>

/** Delete exact ordinary text as a real Word tracked deletion, retaining deleted text and run formatting for review. One body paragraph; simple multiple runs supported. Author explicit; the client supplies current UTC when date is omitted. Fields, wrappers, tables, and paragraphs containing revisions are unsupported. Re-inspect after editing. */
export declare function executeDocxDeleteTrackedText(input: Buffer, op: DeleteTrackedTextInput): Promise<ExecuteDocxReplaceTextOutput>

/** Insert a new paragraph of plain text and real PAGE/NUMPAGES fields. content items use kind text with text, or page/numPages without text. Example: text Page , page, text of , numPages. Body requires placement; footer appends to the single-section default footer and omits placement. Cached values are ? and marked dirty; Word calculates them. Imported fields remain unchanged. */
export declare function executeDocxInsertFields(input: Buffer, op: InsertFieldsInput): Promise<ExecuteDocxReplaceTextOutput>

/** Insert a real footnote or endnote immediately after exact ordinary text in one body paragraph. Plain one-paragraph note text. Independent comments/revisions are preserved; targets inside or crossing protected ranges, fields, wrappers, and tables refuse. Word controls visible numbering; IDs are not displayed note numbers. */
export declare function executeDocxInsertNote(input: Buffer, op: InsertNoteInput): Promise<ExecuteDocxReplaceTextOutput>

/** Insert a page break at a paragraph placement (start/end/before/after). */
export declare function executeDocxInsertPageBreak(input: Buffer, operation: InsertPageBreakInput): Promise<ExecuteDocxReplaceTextOutput>

/**
 * Runs DOCX paragraph insertion away from Node's event loop and returns a Promise.
 * Insert one paragraph. For a blank document, use placement {kind: "end"} or {kind: "start"} without a handle.
 */
export declare function executeDocxInsertParagraph(input: Buffer, operation: InsertParagraphInput): Promise<ExecuteDocxReplaceTextOutput>

/** Insert multiple paragraphs. For a blank document, use placement {kind: "end"} or {kind: "start"} without a handle. */
export declare function executeDocxInsertParagraphs(input: Buffer, operation: InsertParagraphsInput): Promise<ExecuteDocxReplaceTextOutput>

/** Insert a PNG or JPEG from supplied image bytes. */
export declare function executeDocxInsertPicture(input: Buffer, operation: InsertPictureInput): Promise<ExecuteDocxReplaceTextOutput>

/** Insert a real Word section boundary at a body placement. Re-inspect sections and body_blocks after edits. */
export declare function executeDocxInsertSectionBreak(input: Buffer, operation: InsertSectionBreakInput): Promise<ExecuteDocxReplaceTextOutput>

/** Insert a table column (header + one cell per existing row). */
export declare function executeDocxInsertTableColumn(input: Buffer, operation: InsertTableColumnInput): Promise<ExecuteDocxReplaceTextOutput>

/**
 * Runs DOCX table-row insertion away from Node's event loop and returns a Promise.
 * Insert one table row after an anchor; use insert_table_rows when several contiguous rows are known.
 */
export declare function executeDocxInsertTableRow(input: Buffer, operation: InsertTableRowInput): Promise<ExecuteDocxReplaceTextOutput>

/**
 * Runs DOCX multi-row insertion away from Node's event loop and returns a Promise.
 * Insert several contiguous, known rows in one call after a row anchor; prefer this over repeated insert_table_row calls.
 */
export declare function executeDocxInsertTableRows(input: Buffer, operation: InsertTableRowsInput): Promise<ExecuteDocxReplaceTextOutput>

/** Insert a real refreshable Word Table of Contents in a new body paragraph, with an optional title paragraph. Heading levels 1 through maxHeadingLevel (default 3, range 1–9). Marked dirty with a refresh placeholder. Inspect fields first to avoid an unintended duplicate TOC. No in-engine field evaluation. */
export declare function executeDocxInsertToc(input: Buffer, op: InsertTocInput): Promise<ExecuteDocxReplaceTextOutput>

/** Insert text as a real Word tracked insertion before/after an exact ordinary text anchor in one body paragraph. Default after. Inherits formatting at the insertion boundary. Author is explicit; the client supplies current UTC when date is omitted. Fields, wrappers, tables, and paragraphs containing revisions are unsupported. Re-inspect after editing. */
export declare function executeDocxInsertTrackedText(input: Buffer, op: InsertTrackedTextInput): Promise<ExecuteDocxReplaceTextOutput>

/** Reject one supported insertion or deletion using its fresh inspect_tracked_changes handle. Removes inserted content or restores deleted content with its formatting. For replacement, decide both records individually and re-inspect after each successful decision. Complex revisions are unsupported. */
export declare function executeDocxRejectRevision(input: Buffer, op: RevisionDecisionInput): Promise<ExecuteDocxReplaceTextOutput>

/** Replace a picture with supplied image bytes, preserving its layout. */
export declare function executeDocxReplacePicture(input: Buffer, operation: ReplacePictureInput): Promise<ExecuteDocxReplaceTextOutput>

/**
 * Runs DOCX replacement away from Node's event loop and returns a Promise.
 * Replace one exact text span. Requires target text, zero-based occurrence when ambiguous, expectedCurrentText (must match current content), and replacement.
 */
export declare function executeDocxReplaceText(input: Buffer, operation: ReplaceTextInput): Promise<ExecuteDocxReplaceTextOutput>

export interface ExecuteDocxReplaceTextOutput {
  result: OperationResultOutput
  output?: Buffer
}

/** Replace exact ordinary text with real Word deletion + insertion revisions, preserving source formatting. Simple multiple runs require identical formatting. Author explicit; the client supplies current UTC when date is omitted. Fields, wrappers, tables, and paragraphs containing revisions are unsupported. Re-inspect after editing. */
export declare function executeDocxReplaceTextWithTrackedChange(input: Buffer, op: ReplaceTextWithTrackedChangeInput): Promise<ExecuteDocxReplaceTextOutput>

/** Set content-control text by tag and/or alias. */
export declare function executeDocxSetContentControlText(input: Buffer, operation: SetContentControlTextInput): Promise<ExecuteDocxReplaceTextOutput>

/** Set or clear header/footer text. */
export declare function executeDocxSetHeaderFooterText(input: Buffer, operation: SetHeaderFooterTextInput): Promise<ExecuteDocxReplaceTextOutput>

/** Add or clear an external hyperlink on a text target. */
export declare function executeDocxSetHyperlink(input: Buffer, operation: SetHyperlinkInput): Promise<ExecuteDocxReplaceTextOutput>

/** Explicitly enable or disable different odd/even headers and footers for the ENTIRE document. This affects every section; it preserves all variant parts. */
export declare function executeDocxSetOddEvenHeaders(input: Buffer, operation: SetOddEvenHeadersInput): Promise<ExecuteDocxReplaceTextOutput>

/** Set or remove page-number fields in header/footer. */
export declare function executeDocxSetPageNumber(input: Buffer, operation: SetPageNumberInput): Promise<ExecuteDocxReplaceTextOutput>

/** Update section page margins, paper size (letter/a4), or orientation when section properties exist. */
export declare function executeDocxSetPageSetup(input: Buffer, operation: SetPageSetupInput): Promise<ExecuteDocxReplaceTextOutput>

/** Set alignment, spacing, indentation, or keep rules on a direct body paragraph (occurrence zero-based). Twips: 20 = 1 pt. both = justified. */
export declare function executeDocxSetParagraphFormatting(input: Buffer, operation: SetParagraphFormattingInput): Promise<ExecuteDocxReplaceTextOutput>

/** Apply list formatting to one or more paragraph text targets. */
export declare function executeDocxSetParagraphsList(input: Buffer, operation: SetParagraphsListInput): Promise<ExecuteDocxReplaceTextOutput>

/** Set or clear a direct body paragraph style by display name (e.g. 'Heading 1'), not styleId. Does not format table cells. Target via exact text + zero-based occurrence. Omit style to clear. */
export declare function executeDocxSetParagraphStyle(input: Buffer, operation: SetParagraphStyleInput): Promise<ExecuteDocxReplaceTextOutput>

/** Patch a supported floating image using a fresh inspect_layout image handle. Omitted properties stay unchanged. Inline/floating conversion and tight/through wrapping are unsupported. EMU: 914,400 per inch. Re-inspect after editing. */
export declare function executeDocxSetPictureLayout(input: Buffer, operation: SetPictureLayoutInput): Promise<ExecuteDocxReplaceTextOutput>

/** Resize an image using a fresh inspect_layout handle. One dimension preserves aspect ratio; both set exact size. EMU: 914,400 per inch. */
export declare function executeDocxSetPictureSize(input: Buffer, operation: SetPictureSizeInput): Promise<ExecuteDocxReplaceTextOutput>

/** Edit one simple section header/footer variant. text creates independent content; empty text clears it. inherit links to previous; unlink preserves visible content in an independent part. PAGE fields use pageNumber. Other sections retain their content. Enable odd/even explicitly before editing even variants; use differentFirstPage to activate first variants. */
export declare function executeDocxSetSectionHeaderFooter(input: Buffer, operation: SetSectionHeaderFooterInput): Promise<ExecuteDocxReplaceTextOutput>

/** Edit one section using its latest inspected handle. Margins use twips (1440 = one inch). Restart numbering with pageNumberStart or continuePageNumbering=true. */
export declare function executeDocxSetSectionProperties(input: Buffer, operation: SetSectionPropertiesInput): Promise<ExecuteDocxReplaceTextOutput>

/** Format known whole table cells using table + row + column, including header and first-column cells, without find or inspect handles. Format all relevant cells in one call; set fill and direct text formatting together when needed. Supports fill (6-digit RGB), bold, italic, font family, font size in half-points, and text color. Handles remain available for cells that need them. */
export declare function executeDocxSetTableCellsFormatting(input: Buffer, operation: SetTableCellsFormattingInput): Promise<ExecuteDocxReplaceTextOutput>

/** Shade known table cells in one call using table + row + column, including header and first-column cells. Fill is 6-digit RGB without #; omit fill to clear. Use set_table_cells_formatting for fill and text together. */
export declare function executeDocxSetTableCellShading(input: Buffer, operation: SetTableCellShadingInput): Promise<ExecuteDocxReplaceTextOutput>

/**
 * Runs DOCX multi-cell text replacement away from Node's event loop and returns a Promise.
 * Update known cells in one table. Select each by row (header, label, or checked index) and column (first, header, or checked index); use occurrence for duplicate labels. All selectors use the original table state, so headers and row labels can change in this call. Each update needs expectedCurrentText and replacement.
 */
export declare function executeDocxSetTableCellsText(input: Buffer, operation: SetTableCellsTextInput): Promise<ExecuteDocxReplaceTextOutput>

/** Set table column widths in twips. */
export declare function executeDocxSetTableColumnWidths(input: Buffer, operation: SetTableColumnWidthsInput): Promise<ExecuteDocxReplaceTextOutput>

/** Set table alignment/margins/borders. */
export declare function executeDocxSetTableFormatting(input: Buffer, operation: SetTableFormattingInput): Promise<ExecuteDocxReplaceTextOutput>

/** Format exact body text or partial text spans, including supported simple table-cell runs. For known whole table cells, use set_table_cells_formatting. Occurrence is zero-based when ambiguous. */
export declare function executeDocxSetTextFormatting(input: Buffer, operation: SetTextFormattingInput): Promise<ExecuteDocxReplaceTextOutput>

/** Replace plain comment text using a fresh inspect_comments handle. Anchor, author, initials, and date stay unchanged. Inspect again after every edit. */
export declare function executeDocxUpdateComment(input: Buffer, operation: UpdateCommentInput): Promise<ExecuteDocxReplaceTextOutput>

/** Update a simple one-paragraph note using a fresh inspect_notes handle. Keeps its ID, reference, formatting, and other notes. Complex note bodies are preserved and cannot be edited. Re-inspect after every mutation. */
export declare function executeDocxUpdateNote(input: Buffer, op: UpdateNoteInput): Promise<ExecuteDocxReplaceTextOutput>

/** Patch supported properties of one Word style globally. Unspecified properties stay unchanged; clear removes a declaration and restores inheritance. Style type and default flag cannot change. */
export declare function executeDocxUpdateStyle(input: Buffer, operation: StyleInput): Promise<ExecuteDocxReplaceTextOutput>

/** @schema {"oneOf":[{"type":"object","properties":{"kind":{"const":"text"},"text":{"type":"string","maxLength":32000}},"required":["kind","text"],"additionalProperties":false},{"type":"object","properties":{"kind":{"type":"string","enum":["page","numPages"]}},"required":["kind"],"additionalProperties":false}]} */
export interface FieldContentInput {
  kind: string
  text?: string
}

export interface FieldInspectionInput {
  offset?: number
  /** @schema {"minimum":1,"maximum":100} */
  limit?: number
}

/** Find exact current DOCX text. Copy matches[].target into replace_text or text formatting; occurrence is zero-based among text matches. Never guess among duplicates. Refresh after content edits. */
export declare function findDocxText(input: Buffer, request: FindTextInput): Promise<FindTextOutput>

export interface FindTextInput {
  /** @schema {"description":"Exact text to find"} */
  text: string
}

export interface FindTextMatchOutput {
  target: TextTargetInput
  occurrence: number
  text: string
  before: string
  after: string
  container: string
}

export interface FindTextOutput {
  ok: boolean
  query: string
  matchCount: number
  matches: Array<FindTextMatchOutput>
  diagnostics: Array<DiagnosticOutput>
}

export interface FormatCapabilitiesOutput {
  format: string
  capabilities: Array<string>
  operations: Array<DocxOperationOutput>
}

/** Returns the static DOCX runtime manifest without requiring artifact bytes. */
export declare function getDocxCapabilities(): RuntimeCapabilitiesOutput

export interface HeadingOutput {
  paragraphTarget?: TextTargetInput
  textTarget?: TextTargetInput
  occurrence: number
  text: string
  styleName: string
  level?: number
}

export interface HeadingPageOutput {
  page: InspectionPageOutput
  items: Array<HeadingOutput>
}

/** @schema {"minProperties":1} */
export interface ImageDistanceInput {
  /** @schema {"minimum":0,"maximum":4294967295} */
  topEmu?: number
  /** @schema {"minimum":0,"maximum":4294967295} */
  bottomEmu?: number
  /** @schema {"minimum":0,"maximum":4294967295} */
  leftEmu?: number
  /** @schema {"minimum":0,"maximum":4294967295} */
  rightEmu?: number
}

/** @schema {"oneOf":[{"required":["alignment"]},{"required":["offsetEmu"]}]} */
export interface ImagePositionInput {
  /** @schema {"enum":["page","margin","column","paragraph"]} */
  reference: string
  /**
   * start/end mean left/right horizontally, top/bottom vertically.
   * @schema {"enum":["start","center","end"]}
   */
  alignment?: string
  /** @schema {"minimum":-2147483648,"maximum":2147483647} */
  offsetEmu?: number
}

export interface InsertFieldsInput {
  /** @schema {"minItems":1,"maxItems":20,"items":{"oneOf":[{"type":"object","properties":{"kind":{"const":"text"},"text":{"type":"string","maxLength":32000}},"required":["kind","text"],"additionalProperties":false},{"type":"object","properties":{"kind":{"type":"string","enum":["page","numPages"]}},"required":["kind"],"additionalProperties":false}]}} */
  content: Array<FieldContentInput>
  /**
   * Body (default) requires placement. Footer appends to a single-section default footer.
   * @schema {"enum":["body","footer"]}
   */
  location?: string
  /** @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."} */
  placement?: ParagraphPlacementInput
}

export interface InsertNoteInput {
  /** @schema {"enum":["footnote","endnote"]} */
  kind: string
  target: TextTargetInput
  /** @schema {"minLength":1,"maxLength":32000} */
  text: string
}

export interface InsertPageBreakInput {
  /** @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."} */
  placement: ParagraphPlacementInput
  baseRevision?: string
}

export interface InsertParagraphInput {
  text: string
  /** @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."} */
  placement: ParagraphPlacementInput
  baseRevision?: string
}

export interface InsertParagraphsInput {
  texts: Array<string>
  /** @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."} */
  placement: ParagraphPlacementInput
  baseRevision?: string
}

export interface InsertPictureInput {
  widthEmu?: number
  heightEmu?: number
  layout?: PictureLayoutInput
  imageBytes: Buffer
  placement: ParagraphPlacementInput
  altText?: string
  baseRevision?: string
}

export interface InsertSectionBreakInput {
  /** @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."} */
  placement: ParagraphPlacementInput
  /** @schema {"enum":["nextPage","continuous","oddPage","evenPage"]} */
  breakType: string
}

export interface InsertTableColumnInput {
  /** @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."} */
  table: TableTargetInput
  afterColumnHeader?: string
  afterColumnHandle?: string
  header: string
  cells: Array<string>
  baseRevision?: string
}

export interface InsertTableRowInput {
  /** @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."} */
  table: TableTargetInput
  /** @schema {"description":"Row selector. Use firstCellText/occurrence when unambiguous; inspect only when a current row handle is needed."} */
  after: TableRowTargetInput
  cells: Array<string>
  baseRevision?: string
}

export interface InsertTableRowsInput {
  /** @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."} */
  table: TableTargetInput
  /** @schema {"description":"Row selector. Use firstCellText/occurrence when unambiguous; inspect only when a current row handle is needed."} */
  after: TableRowTargetInput
  rows: Array<Array<string>>
  baseRevision?: string
}

export interface InsertTocInput {
  /** @schema {"description":"Use start/end without a handle at document boundaries, including initial content in a blank document (which has no handles). For before/after, use only a handle from the latest relevant document.inspect; never invent one. Formatting preserves handles within one model turn; handles expire before the next model turn or after other edits."} */
  placement: ParagraphPlacementInput
  /** @schema {"minimum":1,"maximum":9} */
  maxHeadingLevel?: number
  /** @schema {"maxLength":32000} */
  title?: string
}

export interface InsertTrackedTextInput {
  target: TextTargetInput
  /** @schema {"minLength":1,"maxLength":32000} */
  text: string
  /** @schema {"enum":["before","after"]} */
  position?: string
  /** @schema {"minLength":1} */
  author: string
  /** @schema {"description":"UTC ISO timestamp","x-client-default":"currentUtc"} */
  date: string
}

export interface InspectContextOutput {
  target: TextTargetInput
  container?: TextContextContainerOutput
  nearby: Array<TextContextContainerOutput>
}

/** Inspect current DOCX elements. Headings/paragraphs expose textTarget for replace_text and text formatting; paragraph formatting uses headings.paragraphTarget or paragraphs text + targetOccurrence. Heading occurrence and paragraph index are list positions. Copy table/cell handles or use zero-based row/column indices with expected labels. Refresh selectors after content edits. */
export declare function inspectDocx(input: Buffer, request: InspectDocxInput): Promise<InspectDocxOutput>

/**
 * Inspect standard comments, author/date, bounded text, attached text, source-order paragraph locations, fresh handles, and malformed/orphan diagnostics. Default 20; maximum 100 per page. Re-inspect after editing. Resolved state and replies are unavailable.
 * @jsonOutput
 */
export declare function inspectDocxComments(input: Buffer, options?: CommentInspectionInput | undefined | null): Promise<string>

/**
 * Inspect Word fields including TOC, PAGE, NUMPAGES, DATE, and unknown instructions. Returns bounded instruction and cached text separately, representation, part/paragraph location, dirty/locked flags when present, common TOC heading levels, and structural diagnostics. Main document and header/footer parts. Default 20, maximum 100 records; text capped at 2000 characters. Dirty means refresh requested; absent flags do not prove results are current. Does not calculate or edit fields.
 * @jsonOutput
 */
export declare function inspectDocxFields(input: Buffer, options?: FieldInspectionInput | undefined | null): Promise<string>

export interface InspectDocxFocusInput {
  /** @schema {"enum":["overview","headings","paragraphs","tables","body_blocks","table_rows","context"]} */
  kind: string
  offset?: number
  /** @schema {"minimum":1,"maximum":100} */
  limit?: number
  tableHandle?: string
  text?: string
  occurrence?: number
  before?: number
  after?: number
}

export interface InspectDocxInput {
  focus?: InspectDocxFocusInput
  /** Compatibility input for the original context-only binding. */
  target?: TextTargetInput
  before?: number
  after?: number
}

/**
 * Inspect structural section geometry, explicit pagination controls, table/image dimensions, floating-image positions/wrap, fresh image handles, and width warnings. No automatic page positions. Default returns summaries; blockLimit (0–100) and blockOffset request details; sectionIndex selects a zero-based section.
 * @jsonOutput
 */
export declare function inspectDocxLayout(input: Buffer, options?: LayoutInput | undefined | null): Promise<string>

/**
 * Inspect ordinary footnotes and endnotes, fresh handles, note IDs, text, zero-based body paragraph and source reference indexes, reference counts, structural status, and imported numbering settings. Separators are excluded and protected. IDs differ from visible note numbers. Default 20, maximum 100 records per page; note text capped at 2000 characters. Read-only.
 * @jsonOutput
 */
export declare function inspectDocxNotes(input: Buffer, options?: NoteInspectionInput | undefined | null): Promise<string>

export interface InspectDocxOutput {
  ok: boolean
  focus: string
  overview?: OverviewOutput
  bodyBlocks?: BodyBlockPageOutput
  headings?: HeadingPageOutput
  paragraphs?: ParagraphPageOutput
  tables?: TablePageOutput
  tableRows?: TableRowWindowOutput
  context?: InspectContextOutput
  diagnostics: Array<DiagnosticOutput>
}

/**
 * Inspect ordered Word sections with fresh handles, page setup, numbering, and owned/inherited header/footer variants. Re-inspect after every section edit.
 * @jsonOutput
 */
export declare function inspectDocxSections(input: Buffer): Promise<string>

/**
 * Inspects the bounded DOCX formatting system away from Node's event loop.
 * @jsonOutput
 */
export declare function inspectDocxStyleSnapshot(input: Buffer): Promise<string>

/**
 * Inspect existing Word tracked insertions/deletions: IDs, author/date, bounded text, source-order paragraph locations, totals, and complex-revision diagnostics. Main document including tables only. Default 20, maximum 100 per page; text/metadata capped at 2000 characters. Current document text includes insertions and excludes deletions. Does not edit or save a version.
 * @jsonOutput
 */
export declare function inspectDocxTrackedChanges(input: Buffer, options?: RevisionInspectionInput | undefined | null): Promise<string>

export interface InspectionPageOutput {
  total: number
  offset: number
  returned: number
  hasMore: boolean
}

export interface LayoutInput {
  blockOffset?: number
  /** @schema {"maximum":100} */
  blockLimit?: number
  sectionIndex?: number
}

export interface LineSpacingInput {
  /**
   * Auto uses 240 units per line; exact/atLeast use twips (20 per point).
   * @schema {"minimum":1,"description":"Auto: 240 per line, so 276 = 1.15. Exact/atLeast: twips, 20 = 1 pt."}
   */
  value: number
  /** @schema {"enum":["auto","exact","atLeast"]} */
  rule?: string
}

export interface NoteInspectionInput {
  offset?: number
  /** @schema {"minimum":1,"maximum":100} */
  limit?: number
}

export interface OperationChangeOutput {
  kind: string
  before: string
  after: string
}

export interface OperationResultOutput {
  ok: boolean
  status: string
  diagnostics: Array<DiagnosticOutput>
  changes: Array<OperationChangeOutput>
}

export interface OverviewOutput {
  bodyBlockCount: number
  paragraphCount: number
  tableCount: number
  sectionCount: number
}

export interface ParagraphListOutput {
  kind: string
  level: number
  supported: boolean
}

export interface ParagraphOutput {
  textTarget?: TextTargetInput
  index: number
  handle?: string
  targetOccurrence?: number
  text: string
  styleName?: string
  list?: ParagraphListOutput
}

export interface ParagraphPageOutput {
  page: InspectionPageOutput
  items: Array<ParagraphOutput>
}

export interface ParagraphPlacementInput {
  /** @schema {"enum":["start","end","before","after"],"description":"start/end need no handle; before/after need an inspected body_blocks handle."} */
  kind: string
  /** @schema {"description":"Omit for start/end. Required for before/after; copy from the latest relevant document.inspect."} */
  handle?: string
}

/** @schema {"minProperties":1} */
export interface PictureLayoutInput {
  /** @schema {"oneOf":[{"required":["alignment"]},{"required":["offsetEmu"]}]} */
  horizontal?: ImagePositionInput
  /** @schema {"oneOf":[{"required":["alignment"]},{"required":["offsetEmu"]}]} */
  vertical?: ImagePositionInput
  /** @schema {"enum":["square","topAndBottom","behindText","inFrontOfText"]} */
  wrap?: string
  /** @schema {"minProperties":1} */
  distance?: ImageDistanceInput
}

export interface PictureOutput {
  handle: string
  format: string
  widthEmu: number
  heightEmu: number
  altText?: string
  affordances: Array<AffordanceOutput>
}

export interface ReplacePictureInput {
  handle: string
  replacementBytes: Buffer
  contentType: string
  baseRevision?: string
}

export interface ReplaceTextInput {
  target: TextTargetInput
  expectedCurrentText: string
  replacement: string
  baseRevision?: string
}

export interface ReplaceTextWithTrackedChangeInput {
  target: TextTargetInput
  /** @schema {"minLength":1,"maxLength":32000} */
  replacement: string
  /** @schema {"minLength":1} */
  author: string
  /** @schema {"description":"UTC ISO timestamp","x-client-default":"currentUtc"} */
  date: string
}

export interface RevisionDecisionInput {
  /** @schema {"minLength":1} */
  handle: string
}

export interface RevisionInspectionInput {
  offset?: number
  /** @schema {"minimum":1,"maximum":100} */
  limit?: number
}

export interface RuntimeCapabilitiesOutput {
  ok: boolean
  protocolVersion: number
  engineVersion: string
  formats: Array<FormatCapabilitiesOutput>
}

export interface SemanticTableCellTargetInput {
  row?: TableCellRowInput
  column?: TableCellColumnInput
  rowLabel?: string
  columnHeader?: string
  /** @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."} */
  occurrence?: number
  handle?: string
}

export interface SetContentControlTextInput {
  target: ContentControlTargetInput
  expectedCurrentText: string
  replacement: string
  baseRevision?: string
}

export interface SetHeaderFooterTextInput {
  /** @schema {"enum":["header","footer"]} */
  kind: string
  /** @schema {"description":"Omit to clear the text."} */
  text?: string
  baseRevision?: string
}

export interface SetHyperlinkInput {
  target: TextTargetInput
  /** @schema {"description":"External URL. Omit to clear the hyperlink."} */
  url?: string
  baseRevision?: string
}

export interface SetOddEvenHeadersInput {
  enabled: boolean
}

export interface SetPageNumberInput {
  /** @schema {"enum":["header","footer"]} */
  kind: string
  /** @schema {"enum":["left","center","right"],"description":"Omit to remove the page number."} */
  alignment?: string
  baseRevision?: string
}

export interface SetPageSetupInput {
  topMarginTwips?: number
  rightMarginTwips?: number
  bottomMarginTwips?: number
  leftMarginTwips?: number
  /** @schema {"enum":["letter","a4"]} */
  paperSize?: string
  /** @schema {"enum":["portrait","landscape"]} */
  orientation?: string
  baseRevision?: string
}

export interface SetParagraphFormattingInput {
  target: TextTargetInput
  /** @schema {"enum":["left","center","right","both","distribute","clear"]} */
  alignment?: string
  spacingBeforeTwips?: number
  spacingAfterTwips?: number
  leftIndentTwips?: number
  clearLeftIndent?: boolean
  lineSpacing?: LineSpacingInput
  rightIndentTwips?: number
  firstLineIndentTwips?: number
  hangingIndentTwips?: number
  /** @schema {"description":"Keep this paragraph with the following paragraph."} */
  keepWithNext?: boolean
  /** @schema {"description":"Keep all lines of this paragraph on one page."} */
  keepLines?: boolean
  /** @schema {"description":"Remove direct properties and restore inheritance. Omitted fields stay unchanged."} */
  clear?: Array<string>
  baseRevision?: string
}

export interface SetParagraphsListInput {
  targets: Array<TextTargetInput>
  /** @schema {"enum":["bullet","decimal","none"]} */
  kind: string
  /** @schema {"enum":[0,1,2]} */
  level?: number
  continueFromPrevious?: boolean
  baseRevision?: string
}

export interface SetParagraphStyleInput {
  target: TextTargetInput
  /** @schema {"description":"Style display name such as 'Heading 1'. Omit to clear."} */
  style?: string
  baseRevision?: string
}

export interface SetPictureLayoutInput {
  handle: string
  /** @schema {"minProperties":1} */
  layout: PictureLayoutInput
  baseRevision?: string
}

export interface SetPictureSizeInput {
  handle: string
  /** @schema {"minimum":1,"maximum":4294967295} */
  widthEmu?: number
  /** @schema {"minimum":1,"maximum":4294967295} */
  heightEmu?: number
  baseRevision?: string
}

export interface SetSectionHeaderFooterInput {
  handle: string
  /** @schema {"enum":["header","footer"]} */
  kind: string
  /** @schema {"enum":["default","first","even"]} */
  variant: string
  /** @schema {"enum":["text","pageNumber","inherit","unlink"]} */
  action: string
  text?: string
  /** @schema {"enum":["left","center","right"]} */
  alignment?: string
}

export interface SetSectionPropertiesInput {
  handle: string
  pageSetup?: SetPageSetupInput
  differentFirstPage?: boolean
  /** @schema {"enum":["nextPage","continuous","oddPage","evenPage"]} */
  breakType?: string
  /** @schema {"minimum":0} */
  pageNumberStart?: number
  continuePageNumbering?: boolean
}

export interface SetTableCellsFormattingInput {
  /** @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."} */
  table: TableTargetInput
  /** @schema {"minItems":1,"maxItems":100} */
  updates: Array<TableCellFormattingUpdateInput>
  baseRevision?: string
}

export interface SetTableCellShadingInput {
  /** @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."} */
  table: TableTargetInput
  updates: Array<TableCellShadingUpdateInput>
  baseRevision?: string
}

export interface SetTableCellsTextInput {
  /** @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."} */
  table: TableTargetInput
  updates: Array<TableCellTextUpdateInput>
  baseRevision?: string
}

export interface SetTableColumnWidthsInput {
  /** @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."} */
  table: TableTargetInput
  widthsTwips: Array<number>
  baseRevision?: string
}

export interface SetTableFormattingInput {
  /** @schema {"description":"Use exact current headerCells and occurrence when known. Inspect for a missing selector or needed handle. Handles survive read-only turns; content/structure edits clear them immediately, while formatting edits may preserve them only within the edited turn. Occurrence is zero-based."} */
  table: TableTargetInput
  /** @schema {"enum":["left","center","right","clear"]} */
  alignment?: string
  cellMarginTopTwips?: number
  cellMarginRightTwips?: number
  cellMarginBottomTwips?: number
  cellMarginLeftTwips?: number
  /** @schema {"enum":["grid","none","clear"]} */
  borders?: string
  baseRevision?: string
}

export interface SetTextFormattingInput {
  target: TextTargetInput
  bold?: boolean
  italic?: boolean
  /** @schema {"minimum":1,"maximum":65535} */
  fontSizeHalfPoints?: number
  fontFamily?: string
  clearBold?: boolean
  color?: string
  clearColor?: boolean
  underline?: boolean
  clearUnderline?: boolean
  highlight?: string
  clearHighlight?: boolean
  strikethrough?: boolean
  clearStrikethrough?: boolean
  /** @schema {"enum":["baseline","superscript","subscript"]} */
  verticalAlignment?: string
  clearVerticalAlignment?: boolean
  /** @schema {"description":"Remove direct properties and restore inheritance. Omitted fields stay unchanged."} */
  clear?: Array<string>
  baseRevision?: string
}

/** Omitted fields are unchanged; clear names remove declarations and restore inheritance. */
export interface StyleInput {
  /** @schema {"description":"Stable Word style ID, for example OpenSuiteReportHeading"} */
  styleId: string
  /** @schema {"enum":["paragraph","character"]} */
  styleType: string
  /** @schema {"description":"Unique display name; apply paragraph styles using this name"} */
  name?: string
  /** @schema {"description":"Existing parent style ID, for example Heading1"} */
  basedOn?: string
  /** @schema {"description":"Existing paragraph style ID"} */
  next?: string
  /** @schema {"description":"Remove declarations and restore inheritance. Omitted fields stay unchanged. Twips: 20 = 1 pt."} */
  clear?: Array<string>
  baseRevision?: string
  bold?: boolean
  italic?: boolean
  /** @schema {"minimum":1,"maximum":65535,"description":"32 = 16 pt"} */
  fontSizeHalfPoints?: number
  fontFamily?: string
  /** @schema {"pattern":"^([0-9a-fA-F]{6}|auto)$","description":"RGB without #, for example 124733"} */
  color?: string
  underline?: boolean
  highlight?: string
  strikethrough?: boolean
  /** @schema {"enum":["baseline","superscript","subscript"]} */
  verticalAlignment?: string
  /** @schema {"enum":["left","center","right","both","distribute"]} */
  alignment?: string
  lineSpacing?: LineSpacingInput
  spacingBeforeTwips?: number
  spacingAfterTwips?: number
  leftIndentTwips?: number
  rightIndentTwips?: number
  firstLineIndentTwips?: number
  hangingIndentTwips?: number
  /** @schema {"description":"Keep this paragraph with the following paragraph."} */
  keepWithNext?: boolean
  /** @schema {"description":"Keep all lines of this paragraph on one page."} */
  keepLines?: boolean
}

export interface TableCellColumnInput {
  /** @schema {"enum":["first","header","index"]} */
  kind: string
  /** @schema {"description":"First-row column header for kind=header, including column zero. Duplicate labels require an occurrence or an index with expectedHeaderText."} */
  text?: string
  /** @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."} */
  occurrence?: number
  /** @schema {"description":"Zero-based column position for kind=index."} */
  index?: number
  /** @schema {"description":"Required for kind=index; prevents a shifted column from being edited."} */
  expectedHeaderText?: string
}

export interface TableCellFormattingUpdateInput {
  /** @schema {"description":"Prefer row + column. Use a handle only when these selectors cannot express the cell. Legacy rowLabel + columnHeader remains supported for data cells."} */
  target: SemanticTableCellTargetInput
  fill?: string
  textFormatting?: TableCellTextFormattingInput
}

export interface TableCellRowInput {
  /** @schema {"enum":["header","label","index"]} */
  kind: string
  /** @schema {"description":"First-cell row label for kind=label."} */
  text?: string
  /** @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."} */
  occurrence?: number
  /** @schema {"description":"Zero-based row position for kind=index."} */
  index?: number
  /** @schema {"description":"Required for kind=index; prevents a shifted row from being edited."} */
  expectedFirstCellText?: string
}

export interface TableCellShadingUpdateInput {
  /** @schema {"description":"Prefer row + column. Use a handle only when these selectors cannot express the cell. Legacy rowLabel + columnHeader remains supported for data cells."} */
  target: SemanticTableCellTargetInput
  fill?: string
}

export interface TableCellTargetInput {
  rowLabel?: string
  columnHeader?: string
  occurrence?: number
  handle?: string
}

export interface TableCellTextFormattingInput {
  bold?: boolean
  italic?: boolean
  fontFamily?: string
  fontSizeHalfPoints?: number
  color?: string
}

export interface TableCellTextUpdateInput {
  /** @schema {"description":"Prefer row + column. Use a handle only when these selectors cannot express the cell. Legacy rowLabel + columnHeader remains supported for data cells."} */
  target: SemanticTableCellTargetInput
  expectedCurrentText: string
  replacement: string
}

export interface TableColumnOutput {
  occurrence: number
  handle: string
  text: string
}

export interface TableOutput {
  occurrence: number
  handle: string
  rowCount: number
  isRectangular: boolean
  affordances: Array<AffordanceOutput>
  columns: Array<TableColumnOutput>
  rows: Array<TableRowOutput>
}

export interface TablePageOutput {
  page: InspectionPageOutput
  items: Array<TableOutput>
}

export interface TableRowOutput {
  handle: string
  cells: Array<string>
  cellHandles: Array<string>
  cellAffordances: Array<Array<AffordanceOutput>>
}

export interface TableRowTargetInput {
  firstCellText?: string
  /** @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."} */
  occurrence?: number
  handle?: string
}

export interface TableRowWindowItemOutput {
  index: number
  cells: Array<string>
}

export interface TableRowWindowOutput {
  tableHandle: string
  rowCount: number
  columnCount: number
  headerTexts: Array<string>
  rowOffset: number
  rows: Array<TableRowWindowItemOutput>
}

export interface TableTargetInput {
  headerCells?: Array<string>
  /** @schema {"description":"Zero-based occurrence among matching targets. Same convention as find.matches[].occurrence and inspect paragraphs.targetOccurrence / tables.occurrence. Omit when the match is unique."} */
  occurrence?: number
  handle?: string
}

export interface TextContextContainerOutput {
  relativePosition: number
  text: string
  container: string
}

export interface TextTargetInput {
  /** @schema {"description":"Exact text to match"} */
  text: string
  /** @schema {"description":"Zero-based occurrence among matching targets. Omit for unique matches. For text edits copy find.matches[].target or inspect textTarget. Paragraph formatting uses inspect paragraphs.targetOccurrence; tables use their own occurrence. These positions are version-local."} */
  occurrence?: number
}

export interface UpdateCommentInput {
  handle: string
  /** @schema {"minLength":1,"maxLength":32000} */
  text: string
}

export interface UpdateNoteInput {
  /** @schema {"minLength":1} */
  handle: string
  /** @schema {"minLength":1,"maxLength":32000} */
  text: string
}

export * from './compatibility-types'
