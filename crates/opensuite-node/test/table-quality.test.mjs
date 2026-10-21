import assert from 'node:assert/strict'
import { test } from 'node:test'
import binding from '../index.js'
import { documentXml, editDocument, image, packageParts } from './picture-support.mjs'

test('changed column totals keep absolute table width consistent for automatic cell pictures', async () => {
  const created = await binding.executeDocxCreateTable(binding.createBlankDocx(), {
    rows: [['Item', 'Value'], ['Revenue', '125']], placement: { kind: 'end' },
  })
  assert.equal(created.result.ok, true)
  const input = editDocument(created.output, xml => xml.replace(
    '<w:tblW w:w="9360" w:type="dxa"/>',
    "<w:tblW data-keep='9360' w:w='9360' w:type='dxa'/>",
  ))
  const result = await binding.executeDocxSetTableColumnWidths(input, {
    table: { headerCells: ['Item', 'Value'] }, widthsTwips: [3000, 3000],
  })
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics))
  assert.ok(documentXml(result.output).includes("<w:tblW data-keep='9360' w:w='6000' w:type='dxa'/>"))
  const before = packageParts(input), after = packageParts(result.output)
  for (const name of Object.keys(before).filter(name => name !== 'word/document.xml')) assert.deepEqual(after[name], before[name], name)
  const inspected = await binding.inspectDocx(result.output, { focus: { kind: 'tables' } })
  const inserted = await binding.executeDocxInsertPicture(result.output, {
    imageBytes: image(2000, 1000), autoFit: true,
    placement: { kind: 'tableCell', handle: inspected.tables.items[0].rows[1].pictureCellHandles[0] },
  })
  assert.equal(inserted.result.ok, true, JSON.stringify(inserted.result.diagnostics))
  const images = JSON.parse(await binding.inspectDocxImages(inserted.output))
  assert.equal(images.total, 1)
  assert.equal(images.images[0].displayWidthEmu, (3000 - 280) * 635)
  assert.equal(images.images[0].displayWidthEmu, images.images[0].displayHeightEmu * 2)
})

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
