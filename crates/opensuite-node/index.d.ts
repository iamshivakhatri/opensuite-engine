import { Buffer } from 'node:buffer'

export interface Diagnostic { code: string; severity: string; message: string; reasonCode?: string; operation?: string; targetHandle?: string }
export interface OperationChange { kind: string; before: string; after: string }
export interface OperationResult { ok: boolean; status: string; diagnostics: Diagnostic[]; changes: OperationChange[] }
export interface ExecuteDocxResult { result: OperationResult; output?: Buffer }
export interface TextTarget { text: string; occurrence?: number }
export interface ParagraphPlacement { kind: 'start' | 'end' | 'before' | 'after'; handle?: string }

export interface RuntimeFormatCapabilities { format: string; capabilities: string[] }
export interface RuntimeCapabilities { ok: boolean; protocolVersion: number; engineVersion: string; formats: RuntimeFormatCapabilities[] }
export interface FindTextMatch { occurrence: number; text: string; before: string; after: string; container: string }
export interface FindTextResult { ok: boolean; query: string; matchCount: number; matches: FindTextMatch[]; diagnostics: Diagnostic[] }

export interface InspectionPage { total: number; offset: number; returned: number; hasMore: boolean }
export interface Affordance { capability: string; supported: boolean; reason?: string }
export interface Picture { handle: string; format: string; widthEmu: number; heightEmu: number; altText?: string; affordances: Affordance[] }
export interface BodyBlock { handle: string; kind: string; text?: string; styleName?: string; headingLevel?: number; tableHandle?: string; rowCount?: number; columnCount?: number; headerTexts?: string[]; picture?: Picture }
export interface Heading { occurrence: number; text: string; styleName: string; level?: number }
export interface ParagraphList { kind: 'bullet' | 'decimal' | 'unknown'; level: number; supported: boolean }
export interface Paragraph { occurrence: number; handle?: string; text: string; styleName?: string; list?: ParagraphList }
export interface TableRow { handle: string; cells: string[]; cellHandles: string[]; cellAffordances: Affordance[][] }
export interface TableColumn { occurrence: number; handle: string; text: string }
export interface Table { occurrence: number; handle: string; rowCount: number; isRectangular: boolean; affordances: Affordance[]; columns: TableColumn[]; rows: TableRow[] }
export interface TableRowWindow { tableHandle: string; rowCount: number; columnCount: number; headerTexts: string[]; rowOffset: number; rows: Array<{ index: number; cells: string[] }> }
export interface TextContextContainer { relativePosition: number; text: string; container: string }
export interface InspectDocxResult {
  ok: boolean; focus: string
  overview?: { bodyBlockCount: number; paragraphCount: number; tableCount: number; sectionCount: number }
  bodyBlocks?: { page: InspectionPage; items: BodyBlock[] }
  headings?: { page: InspectionPage; items: Heading[] }
  paragraphs?: { page: InspectionPage; items: Paragraph[] }
  tables?: { page: InspectionPage; items: Table[] }
  tableRows?: TableRowWindow
  context?: { target: TextTarget; container?: TextContextContainer; nearby: TextContextContainer[] }
  diagnostics: Diagnostic[]
}
export interface InspectDocxInput {
  focus?: { kind: 'overview' | 'body_blocks' | 'headings' | 'paragraphs' | 'tables' | 'table_rows' | 'context'; offset?: number; limit?: number; tableHandle?: string; text?: string; occurrence?: number; before?: number; after?: number }
  target?: TextTarget; before?: number; after?: number
}

export interface TableTarget { headerCells?: string[]; occurrence?: number; handle?: string }
export interface TableRowTarget { firstCellText?: string; occurrence?: number; handle?: string }
export interface TableCellTarget { rowLabel?: string; columnHeader?: string; occurrence?: number; handle?: string }
export type TableCellRow = { kind: 'header' } | { kind: 'label'; text: string; occurrence?: number } | { kind: 'index'; index: number; expectedFirstCellText: string }
export type DeleteTableRowTarget = TableCellRow | TableRowTarget
export type TableCellColumn = { kind: 'first' } | { kind: 'header'; text: string; occurrence?: number } | { kind: 'index'; index: number; expectedHeaderText: string }
export type SemanticTableCellTarget = TableCellTarget | { row: TableCellRow; column: TableCellColumn }
export interface ReplaceTextInput { target: TextTarget; expectedCurrentText: string; replacement: string; baseRevision?: string }
export interface InsertParagraphInput { text: string; placement: ParagraphPlacement; baseRevision?: string }
export interface InsertParagraphsInput { texts: string[]; placement: ParagraphPlacement; baseRevision?: string }
export interface SetParagraphStyleInput { target: TextTarget; /** Omit to clear the direct style. */ style?: string; baseRevision?: string }
export interface SetParagraphFormattingInput {
  target: TextTarget; alignment?: 'left' | 'center' | 'right' | 'clear'; spacingBeforeTwips?: number; spacingAfterTwips?: number
  leftIndentTwips?: number; clearLeftIndent?: boolean; baseRevision?: string
}
export type VerticalAlignment = 'baseline' | 'superscript' | 'subscript'
export interface SetTextFormattingInput { target: TextTarget; bold?: boolean; italic?: boolean; fontSizeHalfPoints?: number; fontFamily?: string; clearBold?: boolean; color?: string; clearColor?: boolean; underline?: boolean; clearUnderline?: boolean; highlight?: string; clearHighlight?: boolean; strikethrough?: boolean; clearStrikethrough?: boolean; verticalAlignment?: VerticalAlignment; clearVerticalAlignment?: boolean; baseRevision?: string }

export interface CreateTableInput { rows: string[][]; placement: ParagraphPlacement; baseRevision?: string }
export interface InsertTableRowInput { table: TableTarget; after: TableRowTarget; cells: string[]; baseRevision?: string }
export interface InsertTableRowsInput { table: TableTarget; after: TableRowTarget; rows: string[][]; baseRevision?: string }
export interface InsertTableColumnInput { table: TableTarget; afterColumnHeader?: string; afterColumnHandle?: string; header: string; cells: string[]; baseRevision?: string }
export interface SetTableCellsTextInput { table: TableTarget; updates: Array<{ target: SemanticTableCellTarget; expectedCurrentText: string; replacement: string }>; baseRevision?: string }
export interface TableCellTextFormattingInput { bold?: boolean; italic?: boolean; fontFamily?: string; fontSizeHalfPoints?: number; color?: string }
export interface SetTableCellsFormattingInput { table: TableTarget; updates: Array<{ target: SemanticTableCellTarget; fill?: string; textFormatting?: TableCellTextFormattingInput }>; baseRevision?: string }
export interface SetTableFormattingInput {
  table: TableTarget; alignment?: 'left' | 'center' | 'right' | 'clear'; cellMarginTopTwips?: number; cellMarginRightTwips?: number
  cellMarginBottomTwips?: number; cellMarginLeftTwips?: number; borders?: 'grid' | 'none' | 'clear'; baseRevision?: string
}
export interface SetTableColumnWidthsInput { table: TableTarget; widthsTwips: number[]; baseRevision?: string }
export interface TableCellShadingUpdate { target: SemanticTableCellTarget; /** Omit to clear. */ fill?: string }
export interface SetTableCellShadingInput { table: TableTarget; updates: TableCellShadingUpdate[]; baseRevision?: string }

export interface SetContentControlTextInput { target: { tag?: string; alias?: string; occurrence?: number }; expectedCurrentText: string; replacement: string; baseRevision?: string }
export interface SetParagraphsListInput { targets: TextTarget[]; kind: 'bullet' | 'decimal' | 'none'; level?: 0 | 1 | 2; /** Reuse the immediately preceding compatible list. */ continueFromPrevious?: boolean; baseRevision?: string }
export interface SetHyperlinkInput { target: TextTarget; /** Omit to clear the external hyperlink. */ url?: string; baseRevision?: string }
/** EMU: 914,400 per inch. Exactly one alignment or offset per axis. */
export interface ImagePositionInput { reference: 'page' | 'margin' | 'column' | 'paragraph'; alignment?: 'start' | 'center' | 'end'; offsetEmu?: number }
export interface PictureLayoutInput {
  horizontal?: ImagePositionInput; vertical?: ImagePositionInput;
  wrap?: 'square' | 'topAndBottom' | 'behindText' | 'inFrontOfText';
  distance?: { topEmu?: number; bottomEmu?: number; leftEmu?: number; rightEmu?: number };
}
export interface SetPictureLayoutInput { handle: string; layout: PictureLayoutInput; baseRevision?: string }
export interface InsertPictureInput { widthEmu?: number; heightEmu?: number; layout?: PictureLayoutInput; imageBytes: Buffer; placement: ParagraphPlacement; altText?: string; baseRevision?: string }
export interface PictureHandleInput { handle: string; baseRevision?: string }
export interface SetPictureSizeInput extends PictureHandleInput { /** One dimension preserves aspect ratio; both set exact size. */ widthEmu?: number; heightEmu?: number }
export interface ReplacePictureInput extends PictureHandleInput { replacementBytes: Buffer; contentType: string }
export interface InsertPageBreakInput { placement: ParagraphPlacement; baseRevision?: string }
export interface PageBreakHandleInput { handle: string; baseRevision?: string }
export interface SetPageSetupInput {
  topMarginTwips?: number; rightMarginTwips?: number; bottomMarginTwips?: number; leftMarginTwips?: number
  paperSize?: 'letter' | 'a4'; orientation?: 'portrait' | 'landscape'; baseRevision?: string
}
export interface SetHeaderFooterTextInput { kind: 'header' | 'footer'; /** Omit to clear the text. */ text?: string; baseRevision?: string }
export interface SetPageNumberInput { kind: 'header' | 'footer'; /** Omit to remove the page number. */ alignment?: 'left' | 'center' | 'right'; baseRevision?: string }

export function getDocxCapabilities(): RuntimeCapabilities
export function createBlankDocx(): Buffer
export function findDocxText(input: Buffer, request: { text: string }): Promise<FindTextResult>
export function inspectDocx(input: Buffer, request: InspectDocxInput): Promise<InspectDocxResult>
export function inspectDocxStyleSnapshot(input: Buffer): Promise<string>
export function executeDocxReplaceText(input: Buffer, operation: ReplaceTextInput): Promise<ExecuteDocxResult>
export function executeDocxInsertParagraph(input: Buffer, operation: InsertParagraphInput): Promise<ExecuteDocxResult>
export function executeDocxInsertParagraphs(input: Buffer, operation: InsertParagraphsInput): Promise<ExecuteDocxResult>
export function executeDocxDeleteParagraph(input: Buffer, operation: { target: TextTarget; baseRevision?: string }): Promise<ExecuteDocxResult>
export function executeDocxSetParagraphStyle(input: Buffer, operation: SetParagraphStyleInput): Promise<ExecuteDocxResult>
export function executeDocxSetParagraphFormatting(input: Buffer, operation: SetParagraphFormattingInput): Promise<ExecuteDocxResult>
export function executeDocxSetTextFormatting(input: Buffer, operation: SetTextFormattingInput): Promise<ExecuteDocxResult>
export function executeDocxCreateTable(input: Buffer, operation: CreateTableInput): Promise<ExecuteDocxResult>
export function executeDocxDeleteTable(input: Buffer, operation: { table: TableTarget; baseRevision?: string }): Promise<ExecuteDocxResult>
export function executeDocxInsertTableRow(input: Buffer, operation: InsertTableRowInput): Promise<ExecuteDocxResult>
export function executeDocxInsertTableRows(input: Buffer, operation: InsertTableRowsInput): Promise<ExecuteDocxResult>
export function executeDocxInsertTableColumn(input: Buffer, operation: InsertTableColumnInput): Promise<ExecuteDocxResult>
export function executeDocxDeleteTableRow(input: Buffer, operation: { table: TableTarget; row: DeleteTableRowTarget; baseRevision?: string }): Promise<ExecuteDocxResult>
export function executeDocxDeleteTableColumn(input: Buffer, operation: { table: TableTarget; columnHeader?: string; columnHandle?: string; baseRevision?: string }): Promise<ExecuteDocxResult>
export function executeDocxSetTableCellsText(input: Buffer, operation: SetTableCellsTextInput): Promise<ExecuteDocxResult>
export function executeDocxSetTableCellsFormatting(input: Buffer, operation: SetTableCellsFormattingInput): Promise<ExecuteDocxResult>
export function executeDocxSetTableFormatting(input: Buffer, operation: SetTableFormattingInput): Promise<ExecuteDocxResult>
export function executeDocxSetTableColumnWidths(input: Buffer, operation: SetTableColumnWidthsInput): Promise<ExecuteDocxResult>
export function executeDocxSetTableCellShading(input: Buffer, operation: SetTableCellShadingInput): Promise<ExecuteDocxResult>
export function executeDocxSetContentControlText(input: Buffer, operation: SetContentControlTextInput): Promise<ExecuteDocxResult>
export function executeDocxSetParagraphsList(input: Buffer, operation: SetParagraphsListInput): Promise<ExecuteDocxResult>
export function executeDocxSetHyperlink(input: Buffer, operation: SetHyperlinkInput): Promise<ExecuteDocxResult>
export function executeDocxInsertPicture(input: Buffer, operation: InsertPictureInput): Promise<ExecuteDocxResult>
export function executeDocxDeletePicture(input: Buffer, operation: PictureHandleInput): Promise<ExecuteDocxResult>
export function executeDocxSetPictureSize(input: Buffer, operation: SetPictureSizeInput): Promise<ExecuteDocxResult>
export function executeDocxReplacePicture(input: Buffer, operation: ReplacePictureInput): Promise<ExecuteDocxResult>
export function executeDocxInsertPageBreak(input: Buffer, operation: InsertPageBreakInput): Promise<ExecuteDocxResult>
export function executeDocxDeletePageBreak(input: Buffer, operation: PageBreakHandleInput): Promise<ExecuteDocxResult>
export function executeDocxSetPageSetup(input: Buffer, operation: SetPageSetupInput): Promise<ExecuteDocxResult>
export function executeDocxSetHeaderFooterText(input: Buffer, operation: SetHeaderFooterTextInput): Promise<ExecuteDocxResult>
export function executeDocxSetPageNumber(input: Buffer, operation: SetPageNumberInput): Promise<ExecuteDocxResult>

/** Real Word sections, ordered by their boundaries. Handles expire after edits. */
export interface SectionHeaderFooterInspection {
  kind: 'header' | 'footer'
  variant: 'default' | 'first' | 'even'
  linkedToPrevious: boolean
  relationshipId: string | null
  partName: string | null
  text: string | null
  pageNumberAlignment: 'left' | 'center' | 'right' | null
  supported: boolean
}
export interface SectionInspection {
  handle: string
  index: number
  breakType: string
  pageWidthTwips: number | null
  pageHeightTwips: number | null
  orientation: string | null
  marginsTwips: Record<string, number>
  columnCount: number | null
  differentFirstPage: boolean
  oddEvenHeaders: boolean
  pageNumberStart: number | null
  pageNumberFormat: string | null
  headersFooters: SectionHeaderFooterInspection[]
}
export interface SetSectionPropertiesInput {
  handle: string
  pageSetup?: SetPageSetupInput
  differentFirstPage?: boolean
  breakType?: 'nextPage' | 'continuous' | 'oddPage' | 'evenPage'
  pageNumberStart?: number
  continuePageNumbering?: boolean
}
export interface SetSectionHeaderFooterInput {
  handle: string
  kind: 'header' | 'footer'
  variant: 'default' | 'first' | 'even'
  action: 'text' | 'pageNumber' | 'inherit' | 'unlink'
  text?: string
  alignment?: 'left' | 'center' | 'right'
}
/** Returns JSON { ok, sections, diagnostics }. */
export function inspectDocxSections(input: Buffer): Promise<string>
export function executeDocxInsertSectionBreak(input: Buffer, operation: { placement: ParagraphPlacement; breakType: 'nextPage' | 'continuous' | 'oddPage' | 'evenPage' }): Promise<ExecuteDocxResult>
export function executeDocxSetSectionProperties(input: Buffer, operation: SetSectionPropertiesInput): Promise<ExecuteDocxResult>
export function executeDocxSetSectionHeaderFooter(input: Buffer, operation: SetSectionHeaderFooterInput): Promise<ExecuteDocxResult>
/** Explicit document-wide setting; even variants never enable it implicitly. */
export function executeDocxSetOddEvenHeaders(input: Buffer, operation: { enabled: boolean }): Promise<ExecuteDocxResult>

/** Stable Word style ID; no default-style authoring. Colors use six hex digits without #. */
export interface StyleInput {
  styleId: string; styleType: 'paragraph' | 'character'; name?: string; basedOn?: string; next?: string;
  bold?: boolean;
  italic?: boolean;
  fontSizeHalfPoints?: number;
  fontFamily?: string;
  color?: string;
  underline?: boolean;
  alignment?: 'left' | 'center' | 'right' | 'both' | 'distribute';
  spacingBeforeTwips?: number;
  spacingAfterTwips?: number;
  leftIndentTwips?: number;
  rightIndentTwips?: number;
  firstLineIndentTwips?: number;
  hangingIndentTwips?: number;
  keepWithNext?: boolean;
  keepLines?: boolean;
  clear?: Array<'basedOn' | 'next' | 'bold' | 'italic' | 'fontSizeHalfPoints' | 'fontFamily' | 'color' | 'underline' | 'alignment' | 'spacingBeforeTwips' | 'spacingAfterTwips' | 'leftIndentTwips' | 'rightIndentTwips' | 'firstLineIndentTwips' | 'hangingIndentTwips' | 'keepWithNext' | 'keepLines'>;
  baseRevision?: string;
}
export function executeDocxCreateStyle(input: Buffer, operation: StyleInput & { name: string }): Promise<ExecuteDocxResult>
export function executeDocxUpdateStyle(input: Buffer, operation: StyleInput): Promise<ExecuteDocxResult>

export interface LayoutOptions { blockOffset?: number; blockLimit?: number; sectionIndex?: number }
/** JSON structural LayoutSnapshot. No rendered pagination or mutation. */
export function inspectDocxLayout(input: Buffer, options?: LayoutOptions): Promise<string>

export function executeDocxSetPictureLayout(input: Buffer, operation: SetPictureLayoutInput): Promise<ExecuteDocxResult>

/** Standard comments only; no replies or resolved state. */
export interface CommentInspectionOptions { offset?: number; limit?: number }
export interface CommentSummary {
  id: string | null; handle: string | null; author: string | null; initials: string | null; date: string | null;
  text: string; anchoredText: string | null; paragraphIndex: number | null; endParagraphIndex: number | null;
  hasRange: boolean; hasReference: boolean; structure: 'range' | 'point' | 'orphaned' | 'malformed'; truncated: boolean;
}
export interface CommentInspection { ok: boolean; comments: CommentSummary[]; total: number; offset: number; hasMore: boolean; diagnostics: Array<{code: string; message: string}> }
export interface AddCommentInput { target: TextTarget; text: string; author: string; initials?: string; date: string }
export function inspectDocxComments(input: Buffer, options?: CommentInspectionOptions): Promise<string>
export function executeDocxAddComment(input: Buffer, operation: AddCommentInput): Promise<ExecuteDocxResult>
export function executeDocxUpdateComment(input: Buffer, operation: {handle: string; text: string}): Promise<ExecuteDocxResult>
export function executeDocxDeleteComment(input: Buffer, operation: {handle: string}): Promise<ExecuteDocxResult>

/** Read-only, bounded main-document insertion/deletion inspection. JSON RevisionInspection. */
export function inspectDocxTrackedChanges(input: Buffer, options?: { offset?: number; limit?: number }): Promise<string>

export interface TrackedTextMetadata { author: string; date: string }
export interface InsertTrackedTextInput extends TrackedTextMetadata { target: TextTarget; text: string; position?: 'before' | 'after' }
export interface DeleteTrackedTextInput extends TrackedTextMetadata { target: TextTarget }
export interface ReplaceTextWithTrackedChangeInput extends TrackedTextMetadata { target: TextTarget; replacement: string }
export function executeDocxInsertTrackedText(input: Buffer, operation: InsertTrackedTextInput): Promise<ExecuteDocxResult>
export function executeDocxDeleteTrackedText(input: Buffer, operation: DeleteTrackedTextInput): Promise<ExecuteDocxResult>
export function executeDocxReplaceTextWithTrackedChange(input: Buffer, operation: ReplaceTextWithTrackedChangeInput): Promise<ExecuteDocxResult>

export interface RevisionDecisionInput { handle: string }
export function executeDocxAcceptRevision(input: Buffer, operation: RevisionDecisionInput): Promise<ExecuteDocxResult>
export function executeDocxRejectRevision(input: Buffer, operation: RevisionDecisionInput): Promise<ExecuteDocxResult>
