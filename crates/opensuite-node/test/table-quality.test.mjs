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

test('shades and bolds a table header, then sizes its columns', async () => {
  const { createBlankDocx, executeDocxCreateTable, executeDocxSetTableCellShading, executeDocxSetTextFormatting, executeDocxSetTableColumnWidths, inspectDocx } = binding
  let bytes = (await executeDocxCreateTable(createBlankDocx(), { rows: [['Option', 'Scope'], ['A', 'Pilot']], placement: { kind: 'end' } })).output
  const inspected = await inspectDocx(bytes, { focus: { kind: 'tables' } })
  const table = inspected.tables.items[0]
  const updates = table.rows[0].cellHandles.map(handle => ({ target: { handle }, fill: '17365D' }))
  let result = await executeDocxSetTableCellShading(bytes, { table: { handle: table.handle }, updates })
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics))
  bytes = result.output
  for (const text of ['Option', 'Scope']) {
    result = await executeDocxSetTextFormatting(bytes, { target: { text }, bold: true })
    assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics))
    bytes = result.output
  }
  result = await executeDocxSetTableColumnWidths(bytes, { table: { headerCells: ['Option', 'Scope'] }, widthsTwips: [3000, 3000] })
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics))
})
