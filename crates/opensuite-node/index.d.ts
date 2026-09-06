export interface DiagnosticOutput { code: string; severity: string; message: string }
export interface TextTargetInput { text: string; occurrence?: number }
export interface RuntimeCapabilitiesOutput { ok: boolean; protocolVersion: number; engineVersion: string; formats: Array<{ format: string; capabilities: string[] }> }
export interface FindTextOutput {
  ok: boolean
  query: string
  matchCount: number
  matches: Array<{ occurrence: number; text: string; before: string; after: string; container: 'paragraph' | 'table_cell' }>
  diagnostics: DiagnosticOutput[]
}
export type InspectDocxFocus =
  | { kind: 'overview' }
  | { kind: 'headings'; offset?: number; limit?: number }
  | { kind: 'paragraphs'; offset?: number; limit?: number }
  | { kind: 'tables'; offset?: number; limit?: number }
  | { kind: 'context'; text: string; occurrence?: number; before?: number; after?: number }
export interface InspectionPage<T> { page: { total: number; offset: number; returned: number; hasMore: boolean }; items: T[] }
export interface InspectDocxOutput {
  ok: boolean
  focus: string
  overview?: { bodyBlockCount: number; paragraphCount: number; tableCount: number; sectionCount: number }
  headings?: InspectionPage<{ occurrence: number; text: string; styleName: string; level?: number }>
  paragraphs?: InspectionPage<{ occurrence: number; text: string; styleName?: string }>
  tables?: InspectionPage<{ occurrence: number; rowCount: number; isRectangular: boolean; rows: Array<{ cells: string[] }> }>
  context?: { target: TextTargetInput; container?: { relativePosition: number; text: string; container: 'paragraph' | 'table_cell' }; nearby: Array<{ relativePosition: number; text: string; container: 'paragraph' | 'table_cell' }> }
  diagnostics: DiagnosticOutput[]
}
export interface ExecuteDocxReplaceTextOutput { result: { ok: boolean; status: string; diagnostics: DiagnosticOutput[]; changes: Array<{ kind: string; before: string; after: string }> }; output?: Buffer }
export function getDocxCapabilities(): RuntimeCapabilitiesOutput
export function findDocxText(input: Buffer, request: { text: string }): Promise<FindTextOutput>
export function inspectDocx(input: Buffer, request: { focus: InspectDocxFocus } | { target: TextTargetInput; before?: number; after?: number }): Promise<InspectDocxOutput>
export function executeDocxReplaceText(input: Buffer, operation: { target: TextTargetInput; expectedCurrentText: string; replacement: string; baseRevision?: string }): Promise<ExecuteDocxReplaceTextOutput>
