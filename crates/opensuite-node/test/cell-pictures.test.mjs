import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFileSync, writeFileSync } from 'node:fs'
import engine from '../index.js'
import { documentXml, editDocument, image, packageParts } from './picture-support.mjs'

const inspect = async bytes => JSON.parse(await engine.inspectDocxLayout(bytes, { blockLimit: 100 }))
const tables = async bytes => (await engine.inspectDocx(bytes, { focus: { kind: 'tables' } })).tables.items
async function apply(bytes, method, operation) {
  const result = await engine[method](bytes, operation)
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics))
  return result.output
}
async function placement(bytes, row, column, position) {
  return { kind: 'tableCell', handle: (await tables(bytes))[0].rows[row].pictureCellHandles[column], ...(position ? { position } : {}) }
}
async function seed() {
  let bytes = await apply(engine.createBlankDocx(), 'executeDocxInsertParagraphs', { texts: ['Native image report', 'Pictures, text, and table content survive together.'], placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxSetParagraphStyle', { target: { text: 'Native image report' }, style: 'Heading 1' })
  return apply(bytes, 'executeDocxCreateTable', { rows: [['Item', 'Picture'], ['Logo', 'Existing caption'], ['Screenshot', 'Existing notes']], placement: { kind: 'end' } })
}
function cells(bytes) { return [...documentXml(bytes).matchAll(/<w:tc>(.*?)<\/w:tc>/gs)].map(match => match[1]) }
function assertImageRelations(bytes) {
  const parts = packageParts(bytes)
  const ids = [...parts['word/document.xml'].toString().matchAll(/r:embed="([^"]+)"/g)].map(match => match[1])
  assert.equal(new Set(ids).size, ids.length)
  for (const id of ids) {
    const relationship = parts['word/_rels/document.xml.rels'].toString().match(new RegExp(`<Relationship\\b[^>]*Id="${id}"[^>]*/>`))?.[0]
    assert.ok(relationship, id)
    assert.match(relationship, /Type="[^"]*\/image"/)
    const target = relationship.match(/Target="([^"]+)"/)[1]
    assert.ok(parts[`word/${target}`], target)
  }
}

test('native cell insertion preserves paragraphs, fits widths, and reuses resize/replace/delete', async () => {
  let bytes = await seed()
  bytes = editDocument(bytes, xml => xml.replace('Existing caption</w:t></w:r></w:p>', 'Existing caption</w:t></w:r></w:p><w:p><w:r><w:t>Second caption paragraph</w:t></w:r></w:p>'))
  const original = documentXml(bytes)
  for (const [row, col, position, width, height, alignment, size] of [
    [1, 1, 'start', 32, 16, 'left'], [2, 1, undefined, 2000, 1000, 'center'], [2, 0, 'end', 80, 160, 'right', 457200],
  ]) bytes = await apply(bytes, 'executeDocxInsertPicture', { imageBytes: image(width, height), placement: await placement(bytes, row, col, position), alignment, ...(size ? { widthEmu: size } : { autoFit: true }) })
  const facts = await inspect(bytes)
  assert.equal(facts.imageCount, 3)
  assert.deepEqual(facts.images.map(i => [i.displayWidthEmu, i.displayHeightEmu]), [[304800, 152400], [457200, 914400], [2794000, 1397000]])
  const table = (await tables(Buffer.from(bytes)))[0]
  assert.deepEqual(table.rows.map(row => row.cells.map(text => text.trim())), [['Item', 'Picture'], ['Logo', 'Existing captionSecond caption paragraph'], ['Screenshot', 'Existing notes']])
  const content = cells(bytes)
  assert.ok(content[3].indexOf('<w:drawing>') < content[3].indexOf('Existing caption'))
  assert.ok(content[5].indexOf('<w:drawing>') > content[5].indexOf('Existing notes'))
  for (const [index, alignment] of [[3, 'left'], [4, 'right'], [5, 'center']]) assert.ok(content[index].includes(`<w:jc w:val="${alignment}"/>`))
  assert.equal(documentXml(bytes).replace(/<w:p><w:pPr><w:jc[^]*?<\/w:p>/g, ''), original)
  assertImageRelations(bytes)
  writeFileSync('/private/tmp/opensuite-e2-cells.docx', bytes)
  bytes = readFileSync('/private/tmp/opensuite-e2-cells.docx')
  assert.equal((await inspect(bytes)).imageCount, 3)
  const beforeReplacement = documentXml(bytes), beforeParts = packageParts(bytes)
  bytes = await apply(bytes, 'executeDocxReplacePicture', { handle: facts.images[2].handle, replacementBytes: image(1000, 500), contentType: 'image/png' })
  assert.equal(documentXml(bytes), beforeReplacement)
  const afterParts = packageParts(bytes)
  assert.equal(Object.keys(afterParts).filter(name => !afterParts[name].equals(beforeParts[name])).length, 1)
  const replaced = await inspect(bytes)
  assert.equal(replaced.images[2].displayWidthEmu, facts.images[2].displayWidthEmu)
  bytes = await apply(bytes, 'executeDocxSetPictureSize', { handle: replaced.images[2].handle, heightEmu: 635000 })
  const resized = await inspect(bytes)
  assert.deepEqual([resized.images[2].displayWidthEmu, resized.images[2].displayHeightEmu], [1270000, 635000])
  bytes = await apply(bytes, 'executeDocxDeletePicture', { handle: resized.images[0].handle })
  assert.equal((await inspect(bytes)).imageCount, 2)
  assert.ok(cells(bytes)[3].includes('Second caption paragraph'))
  assertImageRelations(bytes)
})

test('combined native image operations preserve an imported drawing and unrelated package content', async () => {
  let bytes = readFileSync(new URL('../../../tests/fixtures/libreoffice-floating-image.docx', import.meta.url))
  bytes = editDocument(bytes, xml => xml.replace('</pic:spPr>', '<a:extLst><a:ext uri="urn:e2"><e2:keep xmlns:e2="urn:opensuite:e2">untouched</e2:keep></a:ext></a:extLst></pic:spPr>'))
  assert.ok(documentXml(bytes).includes('urn:e2'))
  const importedDrawing = documentXml(bytes).match(/<w:drawing\b[^]*?<\/w:drawing>/)[0]
  const importedParts = packageParts(bytes)
  bytes = await apply(bytes, 'executeDocxInsertParagraph', { text: 'Combined native image coverage', placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxCreateTable', { rows: [['Item', 'Picture'], ['Screenshot', 'Keep this caption']], placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxInsertPicture', { imageBytes: image(2000, 1000), placement: await placement(bytes, 1, 1), widthEmu: 1828800, alignment: 'center' })
  bytes = await apply(bytes, 'executeDocxInsertPicture', { imageBytes: image(2000, 1000), placement: { kind: 'end' }, autoFit: true, alignment: 'center' })
  const references = [['page', 'page'], ['margin', 'margin'], ['column', 'paragraph'], ['page', 'paragraph']]
  for (const [index, wrap] of ['square', 'topAndBottom', 'behindText', 'inFrontOfText'].entries()) {
    const [horizontal, vertical] = references[index]
    bytes = await apply(bytes, 'executeDocxInsertParagraph', { text: `Native ${wrap} image. Declared positions require renderer review.`, placement: { kind: 'end' } })
    bytes = await apply(bytes, 'executeDocxInsertPicture', { imageBytes: image(240, 120), placement: { kind: 'end' }, widthEmu: 914400, layout: { horizontal: { reference: horizontal, alignment: 'end' }, vertical: { reference: vertical, offsetEmu: 0 }, wrap } })
    const facts = await inspect(bytes)
    assert.equal(facts.images.at(-1).anchor.wrap, wrap)
    assert.equal(facts.images.at(-1).anchor.horizontal.reference, horizontal)
    assert.equal(facts.images.at(-1).anchor.vertical.reference, vertical)
  }
  assert.ok(documentXml(bytes).includes(importedDrawing))
  let facts = await inspect(bytes)
  assert.equal(facts.imageCount, 7)
  bytes = await apply(bytes, 'executeDocxSetPictureLayout', { handle: facts.images.at(-1).handle, layout: { horizontal: { reference: 'margin', alignment: 'center' }, wrap: 'topAndBottom' } })
  facts = await inspect(bytes)
  const before = facts.images.at(-1)
  bytes = await apply(bytes, 'executeDocxReplacePicture', { handle: before.handle, replacementBytes: image(120, 240), contentType: 'image/png' })
  facts = await inspect(Buffer.from(bytes))
  assert.deepEqual(facts.images.at(-1).anchor, before.anchor)
  assert.equal(facts.images.at(-1).displayWidthEmu, 914400)
  assert.equal(facts.images.at(-1).displayHeightEmu, 457200)
  assert.ok(documentXml(bytes).includes(importedDrawing))
  const afterParts = packageParts(bytes)
  for (const [name, payload] of Object.entries(importedParts)) if (name !== 'word/document.xml' && name !== 'word/_rels/document.xml.rels') assert.deepEqual(afterParts[name], payload, name)
  assertImageRelations(bytes)
  writeFileSync('/private/tmp/opensuite-e2-combined.docx', bytes)
  assert.equal((await inspect(readFileSync('/private/tmp/opensuite-e2-combined.docx'))).imageCount, 7)
})

test('native rejects stale/invalid cell targets, floating cells, and unsupported structures', async () => {
  const bytes = await seed(), target = await placement(bytes, 1, 1)
  for (const options of [
    { placement: { kind: 'tableCell' } }, { placement: { ...target, position: 'middle' } },
    { placement: { kind: 'end', position: 'start' } }, { placement: { ...target, handle: 't0:r1:c1' } },
    { placement: { ...target, handle: target.handle.replace(':r1:', ':r99:') } },
    { placement: { ...target, handle: target.handle.replace(':c1:', ':c99:') } },
    { widthEmu: 100, autoFit: true },
    { layout: { horizontal: { reference: 'page', alignment: 'start' }, vertical: { reference: 'page', alignment: 'start' } } },
  ]) {
    const result = await engine.executeDocxInsertPicture(bytes, { imageBytes: image(32, 16), placement: target, ...options })
    assert.equal(result.result.ok, false); assert.equal(result.output, undefined)
  }
  const changed = await apply(bytes, 'executeDocxInsertTableRows', { table: { handle: 't0' }, after: { firstCellText: 'Screenshot' }, rows: [['Added', 'row']] })
  const stale = await engine.executeDocxInsertPicture(changed, { imageBytes: image(32, 16), placement: target })
  assert.equal(stale.result.ok, false); assert.equal(stale.output, undefined)
  for (const edit of [
    xml => xml.replace('<w:tc>', '<w:tc><w:tcPr><w:gridSpan w:val="2"/></w:tcPr>'),
    xml => xml.replace('<w:tc>', '<w:tc><w:tcPr><w:vMerge/></w:tcPr>'),
    xml => xml.replace('</w:tc>', '<w:tbl><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl></w:tc>'),
    xml => xml.replace('<w:tbl>', '<w:bookmarkStart w:id="99" w:name="Protected"/><w:tbl>').replace('</w:tbl>', '</w:tbl><w:bookmarkEnd w:id="99"/>'),
    xml => xml.replace(/w:w="4680"/g, 'w:w="0"'),
    xml => xml.replace('<w:tc>', '<w:tc><w:tcPr><w:tcW w:w="50" w:type="pct"/></w:tcPr>'),
  ]) {
    const input = editDocument(bytes, edit)
    const result = await engine.executeDocxInsertPicture(input, { imageBytes: image(32, 16), placement: await placement(input, 1, 1), autoFit: true })
    assert.equal(result.result.ok, false); assert.equal(result.output, undefined)
  }
})
