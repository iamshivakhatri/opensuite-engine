export interface DiagnosticOutput {
  code: string
  severity: string
  message: string
  reasonCode?: string
  operation?: string
  targetHandle?: string
}

export interface OperationResultOutput {
  ok: boolean
  status: string
  diagnostics: DiagnosticOutput[]
  changes: Array<{ kind: string; before: string; after: string }>
}

export interface ExecuteDocxOutput {
  result: OperationResultOutput
  output?: Buffer
}

export interface InspectionPageOutput {
  total: number
  offset: number
  returned: number
  hasMore: boolean
}

export interface BodyBlockOutput {
  handle: string
  kind: 'paragraph' | 'table'
  text?: string
  tableHandle?: string
}

export interface InspectDocxInput {
  focus?: {
    kind: 'overview' | 'body_blocks' | 'headings' | 'paragraphs' | 'tables' | 'context'
    offset?: number
    limit?: number
    text?: string
    occurrence?: number
    before?: number
    after?: number
  }
}

export interface InspectDocxOutput {
  ok: boolean
  focus: string
  bodyBlocks?: { page: InspectionPageOutput; items: BodyBlockOutput[] }
  diagnostics: DiagnosticOutput[]
}

export interface InsertParagraphInput {
  text: string
  placement: { kind: 'start' | 'end' | 'before' | 'after'; handle?: string }
  baseRevision?: string
}

export function createBlankDocx(): Buffer
export function executeDocxInsertParagraph(input: Buffer, operation: InsertParagraphInput): Promise<ExecuteDocxOutput>
export function inspectDocx(input: Buffer, request: InspectDocxInput): Promise<InspectDocxOutput>
