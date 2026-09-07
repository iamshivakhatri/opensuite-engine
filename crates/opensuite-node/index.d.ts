export interface TableTargetInput { headerCells?: string[]; occurrence?: number; handle?: string }
export interface SetTableFormattingInput {
  table: TableTargetInput
  alignment?: 'left' | 'center' | 'right' | 'clear'
  cellMarginTopTwips?: number
  cellMarginRightTwips?: number
  cellMarginBottomTwips?: number
  cellMarginLeftTwips?: number
  borders?: 'grid' | 'none' | 'clear'
  baseRevision?: string
}
export function executeDocxSetTableFormatting(input: Buffer, operation: SetTableFormattingInput): Promise<{ result: { ok: boolean }; output?: Buffer }>
