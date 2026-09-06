export interface DiagnosticOutput {
  code: string
  severity: string
  message: string
}

export interface TextTargetInput {
  text: string
  occurrence?: number
}

export interface RuntimeCapabilitiesOutput {
  ok: boolean
  protocolVersion: number
  engineVersion: string
  formats: Array<{ format: string; capabilities: string[] }>
}

export interface FindTextOutput {
  ok: boolean
  query: string
  matchCount: number
  matches: Array<{
    occurrence: number
    text: string
    before: string
    after: string
    container: 'paragraph' | 'table_cell'
  }>
  diagnostics: DiagnosticOutput[]
}

export interface InspectDocxOutput {
  ok: boolean
  target: TextTargetInput
  container?: { relativePosition: number; text: string; container: 'paragraph' | 'table_cell' }
  nearby: Array<{ relativePosition: number; text: string; container: 'paragraph' | 'table_cell' }>
  diagnostics: DiagnosticOutput[]
}

export interface ExecuteDocxReplaceTextOutput {
  result: {
    ok: boolean
    status: string
    diagnostics: DiagnosticOutput[]
    changes: Array<{ kind: string; before: string; after: string }>
  }
  output?: Buffer
}

export function getDocxCapabilities(): RuntimeCapabilitiesOutput
export function findDocxText(input: Buffer, request: { text: string }): Promise<FindTextOutput>
export function inspectDocx(input: Buffer, request: {
  target: TextTargetInput
  before?: number
  after?: number
}): Promise<InspectDocxOutput>
export function executeDocxReplaceText(input: Buffer, operation: {
  target: TextTargetInput
  expectedCurrentText: string
  replacement: string
  baseRevision?: string
}): Promise<ExecuteDocxReplaceTextOutput>
