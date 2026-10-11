// Compatibility names; input shapes come from generated Rust declarations.
import type { CommentInspectionInput, DeletePageBreakInput, DeletePictureInput, DeleteTableRowTargetInput, DiagnosticOutput, DocxOperationOutput, ExecuteDocxReplaceTextOutput, FindTextMatchOutput, FindTextOutput, FormatCapabilitiesOutput, InspectDocxOutput, LayoutInput, LineSpacingInput, NoteInspectionInput, OperationChangeOutput, OperationResultOutput, ParagraphPlacementInput, RuntimeCapabilitiesOutput, SemanticTableCellTargetInput, SetParagraphFormattingInput, SetTextFormattingInput, TableCellColumnInput, TableCellRowInput, TableCellShadingUpdateInput, TableCellTargetInput, TableRowTargetInput, TableRowWindowOutput, TableTargetInput, TextTargetInput } from "./index"

export type RuntimeCapabilities = RuntimeCapabilitiesOutput
export type RuntimeFormatCapabilities = FormatCapabilitiesOutput
export type DocxOperation = DocxOperationOutput
export type FindTextResult = FindTextOutput
export type FindTextMatch = FindTextMatchOutput
export type Diagnostic = DiagnosticOutput
export type TextTarget = TextTargetInput
export type InspectDocxResult = InspectDocxOutput
export type TableRowWindow = TableRowWindowOutput
export type ExecuteDocxResult = ExecuteDocxReplaceTextOutput
export type OperationResult = OperationResultOutput
export type OperationChange = OperationChangeOutput
export type ParagraphPlacement = ParagraphPlacementInput
export type TableTarget = TableTargetInput
export type TableRowTarget = TableRowTargetInput
export type TableCellShadingUpdate = TableCellShadingUpdateInput
export type SemanticTableCellTarget = SemanticTableCellTargetInput
export type PictureHandleInput = DeletePictureInput
export type PageBreakHandleInput = DeletePageBreakInput
export type TableCellRow = TableCellRowInput
export type TableCellColumn = TableCellColumnInput
export type DeleteTableRowTarget = DeleteTableRowTargetInput
export type TableCellTarget = TableCellTargetInput
export type LayoutOptions = LayoutInput
export type CommentInspectionOptions = CommentInspectionInput
export type NoteInspectionOptions = NoteInspectionInput
export type VerticalAlignment = NonNullable<SetTextFormattingInput["verticalAlignment"]>
export type TextFormattingProperty = NonNullable<SetTextFormattingInput["clear"]>[number]

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

export interface TextContextContainer { relativePosition: number; text: string; container: string }

export type ParagraphFormattingInput = Omit<SetParagraphFormattingInput, 'target' | 'baseRevision'>

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

export interface CommentSummary {
  id: string | null; handle: string | null; author: string | null; initials: string | null; date: string | null;
  text: string; anchoredText: string | null; paragraphIndex: number | null; endParagraphIndex: number | null;
  hasRange: boolean; hasReference: boolean; structure: 'range' | 'point' | 'orphaned' | 'malformed'; truncated: boolean;
}

export interface CommentInspection { ok: boolean; comments: CommentSummary[]; total: number; offset: number; hasMore: boolean; diagnostics: Array<{code: string; message: string}> }

export interface TrackedTextMetadata { author: string; date: string }

export interface NoteSummary { kind: 'footnote' | 'endnote'; id: number | null; handle: string | null; text: string; paragraphIndex: number | null; referenceIndex: number | null; referenceCount: number; structure: 'supported' | 'unsupported' | 'malformed'; truncated: boolean }

export interface NoteSettings { kind: 'footnote' | 'endnote'; partName: string; sectionIndex: number | null; numberFormat: string | null; restart: string | null; position: string | null; start: string | null }

export interface NoteInspection { ok: boolean; notes: NoteSummary[]; total: number; footnoteCount: number; endnoteCount: number; unsupportedCount: number; offset: number; hasMore: boolean; settings: NoteSettings[]; diagnostics: Array<{code: string; message: string}> }
