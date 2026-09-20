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
export interface SetTableCellsTextInput { table: TableTarget; updates: Array<{ target: TableCellTarget; expectedCurrentText: string; replacement: string }>; baseRevision?: string }
export interface SetTableFormattingInput {
  table: TableTarget; alignment?: 'left' | 'center' | 'right' | 'clear'; cellMarginTopTwips?: number; cellMarginRightTwips?: number
  cellMarginBottomTwips?: number; cellMarginLeftTwips?: number; borders?: 'grid' | 'none' | 'clear'; baseRevision?: string
}
export interface SetTableColumnWidthsInput { table: TableTarget; widthsTwips: number[]; baseRevision?: string }
export interface TableCellShadingUpdate { target: TableCellTarget; /** Omit to clear. */ fill?: string }
export interface SetTableCellShadingInput { table: TableTarget; updates: TableCellShadingUpdate[]; baseRevision?: string }

export interface SetContentControlTextInput { target: { tag?: string; alias?: string; occurrence?: number }; expectedCurrentText: string; replacement: string; baseRevision?: string }
export interface SetParagraphsListInput { targets: TextTarget[]; kind: 'bullet' | 'decimal' | 'none'; level?: 0 | 1 | 2; /** Reuse the immediately preceding compatible list. */ continueFromPrevious?: boolean; baseRevision?: string }
export interface SetHyperlinkInput { target: TextTarget; /** Omit to clear the external hyperlink. */ url?: string; baseRevision?: string }
export interface InsertPictureInput { imageBytes: Buffer; placement: ParagraphPlacement; altText?: string; baseRevision?: string }
export interface PictureHandleInput { handle: string; baseRevision?: string }
export interface SetPictureSizeInput extends PictureHandleInput { /** Supply exactly one dimension. */ widthEmu?: number; heightEmu?: number }
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
export function executeDocxDeleteTableRow(input: Buffer, operation: { table: TableTarget; row: TableRowTarget; baseRevision?: string }): Promise<ExecuteDocxResult>
export function executeDocxDeleteTableColumn(input: Buffer, operation: { table: TableTarget; columnHeader?: string; columnHandle?: string; baseRevision?: string }): Promise<ExecuteDocxResult>
export function executeDocxSetTableCellsText(input: Buffer, operation: SetTableCellsTextInput): Promise<ExecuteDocxResult>
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
