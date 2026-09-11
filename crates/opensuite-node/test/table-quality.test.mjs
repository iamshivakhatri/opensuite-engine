import assert from 'node:assert/strict'
import { test } from 'node:test'
import binding from '../index.js'

test('maps explicit widths and batch header shading', async () => {
  const { createBlankDocx, executeDocxCreateTable, executeDocxSetTableColumnWidths, executeDocxSetTableCellShading } = binding
  let bytes = (await executeDocxCreateTable(createBlankDocx(), { rows: [['Name', 'Notes'], ['A', 'Long value']], placement: { kind: 'end' } })).output
  const table = { headerCells: ['Name', 'Notes'] }
  bytes = (await executeDocxSetTableColumnWidths(bytes, { table, widthsTwips: [2400, 6960] })).output
  const shaded = await executeDocxSetTableCellShading(bytes, { table, updates: [
    { target: { handle: 't0:r0:c0' }, fill: 'E9EEF5' }, { target: { handle: 't0:r0:c1' }, fill: 'E9EEF5' },
  ] })
  assert.equal(shaded.result.ok, true)
})
