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

test('native alignment applies to every cell paragraph and clears only requested properties', async () => {
  const { documentXml, editDocument, packageParts } = await import('./picture-support.mjs')
  let input = (await binding.executeDocxCreateTable(binding.createBlankDocx(), { rows: [['First', 'Other']], placement: { kind: 'end' } })).output
  input = editDocument(input, xml => xml.replace('<w:t>First</w:t></w:r></w:p>', '<w:t>First</w:t></w:r></w:p><w:p/><w:p><w:r><w:rPr><w:i/></w:rPr><w:t>Second</w:t></w:r></w:p>'))
  const update = { target: { handle: 't0:r0:c0' }, alignment: 'center', verticalAlignment: 'bottom', fill: 'ABCDEF' }
  const options = { table: { handle: 't0' }, updates: [update] }
  const result = await binding.executeDocxSetTableCellsFormatting(input, options)
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics))
  const xml = documentXml(result.output)
  assert.equal((xml.match(/<w:jc w:val="center"\/>/g) ?? []).length, 3)
  assert.equal((xml.match(/<w:vAlign w:val="bottom"\/>/g) ?? []).length, 1)
  assert.match(xml, /<w:i\/>/)
  assert.match(xml, /w:fill="ABCDEF"/)
  const beforeParts = packageParts(input), afterParts = packageParts(result.output)
  for (const name of Object.keys(beforeParts).filter(n => n !== 'word/document.xml')) assert.deepEqual(afterParts[name], beforeParts[name], name)
  const repeated = await binding.executeDocxSetTableCellsFormatting(result.output, options)
  assert.equal(documentXml(repeated.output), xml)
  const cleared = await binding.executeDocxSetTableCellsFormatting(result.output, { ...options, updates: [{ target: update.target, clear: ['alignment', 'verticalAlignment'] }] })
  assert.equal(cleared.result.ok, true)
  assert.doesNotMatch(documentXml(cleared.output), /<w:jc|<w:vAlign/)
  assert.match(documentXml(cleared.output), /w:fill="ABCDEF"/)
  const snapshot = await binding.inspectDocx(cleared.output, { focus: { kind: 'tables' } })
  assert.deepEqual(snapshot.tables.items[0].rows[0].cells, ['FirstSecond', 'Other'])
})

test('native cell alignment refuses unsafe structures and invalid inputs atomically', async () => {
  const { editDocument, editParts } = await import('./picture-support.mjs')
  const input = (await binding.executeDocxCreateTable(binding.createBlankDocx(), { rows: [['First', 'Other']], placement: { kind: 'end' } })).output
  const op = { table: { handle: 't0' }, updates: [{ target: { handle: 't0:r0:c0' }, alignment: 'right', verticalAlignment: 'center' }] }
  for (const edit of [
    xml => xml.replace('<w:tc>', '<w:tc><w:tcPr><w:vMerge/></w:tcPr>'),
    xml => xml.replace('<w:tc>', '<w:tc><w:tcPr><w:hMerge/></w:tcPr>'),
    xml => xml.replace('<w:tc>', '<w:tc><w:tbl/>'),
    xml => xml.replace('<w:tbl>', '<w:bookmarkStart w:id="4"/><w:tbl>').replace('</w:tbl>', '</w:tbl><w:bookmarkEnd w:id="4"/>'),
    xml => xml.replace('<w:p>', '<w:p><w:pPr><w:jc/><w:jc/></w:pPr>'),
  ]) {
    const result = await binding.executeDocxSetTableCellsFormatting(editDocument(input, edit), op)
    assert.equal(result.result.ok, false); assert.equal(result.output, undefined)
  }
  assert.throws(() => binding.executeDocxSetTableCellsFormatting(input, { ...op, updates: [{ ...op.updates[0], alignment: 'justify' }] }), /alignment/)
  assert.throws(() => binding.executeDocxSetTableCellsFormatting(input, { ...op, updates: [{ ...op.updates[0], verticalAlignment: 'middle' }] }), /alignment/)
  const protectedInput = editParts(input, parts => {
    const key = Object.keys(parts).find(n => n.endsWith('settings.xml'))
    assert.ok(key)
    parts[key] = Buffer.from(parts[key].toString().replace('</w:settings>', '<w:documentProtection w:enforcement="1"/></w:settings>'))
    return parts
  })
  const protectedResult = await binding.executeDocxSetTableCellsFormatting(protectedInput, op)
  assert.equal(protectedResult.result.ok, false); assert.equal(protectedResult.output, undefined)
})
