import assert from 'node:assert/strict'
import { test } from 'node:test'
import engine from '../index.js'
import { documentXml, editDocument, packageParts } from './picture-support.mjs'

async function tables(bytes) {
  const result = await engine.inspectDocx(bytes, { focus: { kind: 'tables' } })
  return result.tables.items
}
async function fixture(count = 2) {
  let bytes = engine.createBlankDocx()
  for (let i = 0; i < count; i++) {
    const result = await engine.executeDocxCreateTable(bytes, {
      rows: [[`Header ${i}`, 'Value'], ['First', '1'], ['Second', '2'], ['Third', '3']],
      placement: { kind: 'end' },
    })
    assert.equal(result.result.ok, true)
    bytes = result.output
  }
  return bytes
}
const target = table => ({ handle: table.layoutHandle })
const applied = result => { assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics)); return result.output }
const refused = result => { assert.equal(result.result.ok, false); assert.equal(result.output, undefined) }
async function gap(bytes, before = 0, after = 1, spacingPoints = 8) {
  const selected = await tables(bytes)
  return engine.executeDocxSetTableSpacing(bytes, { before: target(selected[before]), after: target(selected[after]), spacingPoints })
}
async function layout(bytes, options) {
  const [table] = await tables(bytes)
  return engine.executeDocxSetTableRowLayout(bytes, { table: target(table), ...options })
}

test('native exact 8 pt separators preserve tables, repeat safely, and keep the third boundary adjacent', async () => {
  const input = await fixture(3)
  const beforeTables = documentXml(input).match(/<w:tbl>.*?<\/w:tbl>/gs)
  const output = applied(await gap(input))
  assert.match(documentXml(output), /w:before="0" w:after="0".*?w:line="160" w:lineRule="exact"/)
  assert.deepEqual(documentXml(output).match(/<w:tbl>.*?<\/w:tbl>/gs), beforeTables)
  assert.equal(documentXml(output).match(/w:lineRule="exact"/g).length, 1)
  assert.match(documentXml(output), /<\/w:tbl><w:tbl>/)
  const repeat = applied(await gap(output))
  assert.equal(documentXml(repeat), documentXml(output))
  const both = applied(await gap(repeat, 1, 2))
  assert.equal(documentXml(both).match(/w:lineRule="exact"/g).length, 2)
  const changed = applied(await gap(both, 0, 1, 8.5))
  assert.match(documentXml(changed), /w:line="170"/)
  const inputParts = packageParts(input), outputParts = packageParts(changed)
  for (const name of Object.keys(inputParts).filter(name => name !== 'word/document.xml')) assert.deepEqual(outputParts[name], inputParts[name], name)
})

test('native spacing refuses stale, reversed, nonadjacent, invalid and occupied boundaries', async () => {
  const input = await fixture(3), selected = await tables(input)
  const stale = { before: target(selected[0]), after: target(selected[1]), spacingPoints: 8 }
  const output = applied(await gap(input))
  refused(await engine.executeDocxSetTableSpacing(output, stale))
  for (const [a,b] of [[1,0],[0,2],[0,0]]) refused(await gap(input,a,b))
  for (const points of [0,-8,0.01,1584.05,NaN,Infinity]) refused(await gap(input,0,1,points))
  const occupied = editDocument(input, xml => xml.replace('</w:tbl><w:tbl>', '</w:tbl><w:p/><w:tbl>'))
  refused(await gap(occupied))
  refused(await engine.executeDocxSetTableSpacing(input, { ...stale, before: { handle: 't0' } }))
})

test('native header count, explicit clearing, and row splitting preserve unrelated XML', async () => {
  const input = editDocument(await fixture(), xml => xml.replaceAll('<w:tr>', "<w:tr><w:trPr data-keep='row'><w:trHeight w:val='300' w:hRule='atLeast'/></w:trPr>"))
  const output = applied(await layout(input, { repeatingHeaderRows: 2, row: { handle: 't0:r2' }, preventSplit: true }))
  assert.equal(documentXml(output).match(/<w:tblHeader w:val="1"\/>/g).length,2)
  assert.equal(documentXml(output).match(/<w:cantSplit w:val="1"\/>/g).length,1)
  assert.equal(documentXml(output).match(/data-keep='row'/g).length,8)
  assert.match(documentXml(output), /<\/w:tbl><w:tbl>/)
  const repeat = applied(await layout(output, { repeatingHeaderRows: 2, row: { handle: 't0:r2' }, preventSplit: true }))
  assert.equal(documentXml(repeat), documentXml(output))
  const clear = applied(await layout(output, { repeatingHeaderRows: 0, row: { firstCellText: 'Second' }, preventSplit: false }))
  assert.equal(documentXml(clear).match(/<w:tblHeader w:val="0"\/>/g).length,4)
  assert.equal(documentXml(clear).match(/<w:cantSplit w:val="0"\/>/g).length,1)
  const after = await tables(clear)
  assert.deepEqual(after.map(t => t.rows.map(r => r.cells)), (await tables(input)).map(t => t.rows.map(r => r.cells)))
})

test('native row layout refuses invalid header selection, mismatched row and protected structures', async () => {
  const input = await fixture(), selected = await tables(input)
  for (const options of [{}, { repeatingHeaderRows: 5 }, { row: { handle: 't0:r2' } }, { preventSplit: true }, { row: { handle: 't1:r1' }, preventSplit: true }, { row: { handle: 't0:r1', firstCellText: 'wrong' }, preventSplit: true }]) refused(await layout(input,options))
  const changed = applied(await layout(input,{repeatingHeaderRows:1}))
  refused(await engine.executeDocxSetTableRowLayout(changed,{table:target(selected[0]),repeatingHeaderRows:1}))
  // The count-only contract cannot express a non-leading header selection.
  const schema = JSON.parse(engine.getDocxCapabilities().formats[0].operations.find(op => op.id === 'set_table_row_layout').inputSchema)
  assert.equal(schema.properties.repeatingHeaderRows.type,'integer')
  assert.equal(schema.properties.headerRowIndices, undefined)
  for (const edit of [
    xml => xml.replace('<w:tc>', '<w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr>'),
    xml => xml.replace('<w:tr>', '<w:tr><w:trPr><w:tblHeader/><w:tblHeader/></w:trPr>'),
    xml => xml.replace('<w:tr>', '<w:tr><w:trPr><w:trPrChange/></w:trPr>'),
    xml => xml.replace('<w:tbl>', '<w:bookmarkStart w:id="4"/><w:tbl>').replace('</w:tbl>', '</w:tbl><w:bookmarkEnd w:id="4"/>'),
  ]) refused(await layout(editDocument(input,edit), {repeatingHeaderRows:1}))
})

test('multi-page content survives native save and reopen with header and splitting flags', async () => {
  const rows = [['Report heading','Detail'], ...Array.from({length:80},(_,i) => [`Row ${i+1}`, `Line one ${i+1}\nLine two ${i+1}\nLine three ${i+1}`])]
  const input = applied(await engine.executeDocxCreateTable(engine.createBlankDocx(),{ rows, placement:{kind:'end'} }))
  const output = applied(await layout(input,{ repeatingHeaderRows:1, row:{handle:'t0:r30'}, preventSplit:true }))
  const [table] = await tables(output)
  assert.equal(table.rowCount,81)
  assert.deepEqual(table.rows.map(r => r.cells),(await tables(input))[0].rows.map(r => r.cells))
  assert.match(documentXml(output), /<w:tblHeader w:val="1"\/>/)
  assert.match(documentXml(output), /<w:cantSplit w:val="1"\/>/)
})
