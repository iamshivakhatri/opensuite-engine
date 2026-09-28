import assert from 'node:assert/strict'
import { test } from 'node:test'
import binding from '../index.js'

test('formats three inspected header cells in one native mutation', async () => {
  const { createBlankDocx, executeDocxCreateTable, executeDocxSetTableCellsFormatting, inspectDocx } = binding
  const created = await executeDocxCreateTable(createBlankDocx(), {
    rows: [['Status', 'Owner', 'Actual'], ['Open', 'Alice', '10']],
    placement: { kind: 'end' },
  })
  const before = await inspectDocx(created.output, { focus: { kind: 'tables' } })
  const table = before.tables.items[0]
  const updates = table.rows[0].cellHandles.map(handle => ({
    target: { handle }, fill: '17365D',
    textFormatting: { bold: true, italic: true, fontFamily: 'Aptos', fontSizeHalfPoints: 24, color: 'FFFFFF' },
  }))
  const result = await executeDocxSetTableCellsFormatting(created.output, {
    table: { handle: table.handle }, updates,
  })
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics))
  assert.notDeepEqual(result.output, created.output)
  const after = await inspectDocx(result.output, { focus: { kind: 'tables' } })
  assert.deepEqual(after.tables.items[0].rows.map(row => row.cells), table.rows.map(row => row.cells))

  const wrongTable = await executeDocxSetTableCellsFormatting(created.output, {
    table: { handle: table.handle },
    updates: [...updates, { target: { handle: 't1:r0:c0' }, fill: '17365D' }],
  })
  assert.equal(wrongTable.result.ok, false)
  assert.equal(wrongTable.output, undefined)
})
