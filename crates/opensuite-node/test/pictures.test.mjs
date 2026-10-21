import assert from 'node:assert/strict'
import { test } from 'node:test'
import binding from '../index.js'
import { image, documentXml } from './picture-support.mjs'
import { spawnSync } from 'node:child_process'
import { writeFileSync } from 'node:fs'

const {
  createBlankDocx,
  executeDocxDeletePicture,
  executeDocxInsertPicture,
  executeDocxReplacePicture,
  executeDocxSetPictureSize,
  inspectDocx,
} = binding
const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL7WQAAAABJRU5ErkJggg==', 'base64')

test('inspects, inserts, resizes, and deletes a picture', async () => {
  let bytes = createBlankDocx()
  let result = await executeDocxInsertPicture(bytes, {
    imageBytes: png, placement: { kind: 'end' }, altText: 'OpenSuite logo',
  })
  assert.equal(result.result.ok, true)
  bytes = result.output

  let picture = (await inspectDocx(bytes, { focus: { kind: 'body_blocks', offset: 0, limit: 10 } })).bodyBlocks.items[0].picture
  assert.equal(picture.format, 'png')
  assert.equal(picture.altText, 'OpenSuite logo')
  assert.ok(picture.widthEmu > 0)
  assert.ok(picture.heightEmu > 0)
  assert.ok(picture.affordances.every((item) => item.supported))

  result = await executeDocxSetPictureSize(bytes, { handle: picture.handle, widthEmu: 19050 })
  assert.equal(result.result.ok, true)
  bytes = result.output
  picture = (await inspectDocx(bytes, { focus: { kind: 'body_blocks', offset: 0, limit: 10 } })).bodyBlocks.items[0].picture
  assert.equal(picture.widthEmu, 19050)

  result = await executeDocxReplacePicture(bytes, {
    handle: picture.handle, replacementBytes: Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1]), contentType: 'image/png',
  })
  assert.equal(result.result.ok, true)
  bytes = result.output
  picture = (await inspectDocx(bytes, { focus: { kind: 'body_blocks', offset: 0, limit: 10 } })).bodyBlocks.items[0].picture
  assert.equal(picture.altText, 'OpenSuite logo')
  assert.equal(picture.widthEmu, 19050)

  result = await executeDocxDeletePicture(bytes, { handle: picture.handle })
  assert.equal(result.result.ok, true)
  assert.equal((await inspectDocx(result.output, { focus: { kind: 'body_blocks', offset: 0, limit: 10 } })).bodyBlocks.items.length, 0)
})

test('returns a structured picture failure', async () => {
  const result = await executeDocxDeletePicture(createBlankDocx(), { handle: 'p99' })
  assert.equal(result.result.ok, false)
  assert.equal(result.result.diagnostics[0].code, 'TARGET_NOT_FOUND')
  assert.equal(result.output, undefined)
})

function fixture(bytes, mode) {
  const result = spawnSync('python3', ['-c', `
import io,sys,zipfile
source=zipfile.ZipFile(io.BytesIO(sys.stdin.buffer.read()))
output=io.BytesIO()
with zipfile.ZipFile(output,'w',zipfile.ZIP_DEFLATED) as target:
 for part in source.infolist():
  data=source.read(part.filename)
  if part.filename=='word/document.xml':
   xml=data.decode()
   if sys.argv[1]=='column': xml=xml.replace('</w:sectPr>','<w:cols w:num="2" w:space="720"/></w:sectPr>')
   else:
    section=xml[xml.index('<w:sectPr>'):xml.index('</w:sectPr>')+11]
    xml=xml.replace('<w:p>','<w:p><w:pPr>'+section+'</w:pPr>',1)
   data=xml.encode()
  target.writestr(part,data)
sys.stdout.buffer.write(output.getvalue())`, mode], { input: bytes })
  assert.equal(result.status, 0, result.stderr.toString())
  return result.stdout
}
async function layout(bytes) { return JSON.parse(await binding.inspectDocxLayout(bytes, {})) }
async function applyPicture(bytes, options) {
  const result = await executeDocxInsertPicture(bytes, { placement: { kind: 'end' }, ...options })
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics))
  return result.output
}

test('native automatic fitting, alignment, section geometry, and render fixtures', async () => {
  for (const [name, width, height, expected, alignment] of [
    ['wide', 2000, 1000, [5943600, 2971800], 'center'],
    ['tall', 1000, 4000, [2006600, 8026400], 'right'],
    ['logo', 32, 16, [304800, 152400], 'left'],
  ]) {
    const output = await applyPicture(createBlankDocx(), { imageBytes: image(width, height), autoFit: true, alignment })
    const facts = await layout(output)
    assert.deepEqual([facts.images[0].displayWidthEmu, facts.images[0].displayHeightEmu], expected)
    assert.equal(facts.sectionCount, 1)
    const xml = documentXml(output)
    assert.ok(xml.includes(`<w:p><w:pPr><w:jc w:val="${alignment}"/></w:pPr><w:r><w:drawing>`))
    assert.equal((xml.match(/<w:p>/g) ?? []).length, 1)
    writeFileSync(`/private/tmp/opensuite-a1-${name}.docx`, output)
  }
  for (const [name, setup, expected] of [
    ['landscape', { orientation: 'landscape' }, 8229600],
    ['margins', { leftMarginTwips: 2880, rightMarginTwips: 2880 }, 4114800],
  ]) {
    let bytes = createBlankDocx()
    const section = JSON.parse(await binding.inspectDocxSections(bytes)).sections[0]
    const changed = await binding.executeDocxSetSectionProperties(bytes, { handle: section.handle, pageSetup: setup })
    assert.equal(changed.result.ok, true)
    bytes = await applyPicture(changed.output, { imageBytes: image(2000, 1000), autoFit: true, alignment: 'center' })
    assert.equal((await layout(bytes)).images[0].displayWidthEmu, expected)
    writeFileSync(`/private/tmp/opensuite-a1-${name}.docx`, bytes)
  }
  const columns = await applyPicture(fixture(createBlankDocx(), 'column'), { imageBytes: image(2000, 1000), autoFit: true, alignment: 'center' })
  assert.equal((await layout(columns)).images[0].displayWidthEmu, 2743200)
  writeFileSync('/private/tmp/opensuite-a1-columns.docx', columns)
})

test('native deletion keeps section properties and stable relationships after reopen', async () => {
  let bytes = await applyPicture(createBlankDocx(), { imageBytes: png })
  bytes = fixture(bytes, 'section')
  const before = documentXml(bytes)
  const facts = await layout(bytes)
  const result = await executeDocxDeletePicture(bytes, { handle: facts.images[0].handle })
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics))
  const after = await layout(result.output)
  assert.equal(after.imageCount, 0); assert.equal(after.sectionCount, 2)
  assert.equal(documentXml(result.output), before.replace(/<w:drawing>.*?<\/w:drawing>/s, ''))
  const next = await applyPicture(result.output, { imageBytes: png, autoFit: true })
  assert.equal((await layout(next)).imageCount, 1)
  assert.equal((await layout(next)).sectionCount, 2)
})

test('native rejects conflicting fit options and invalid alignment without output', async () => {
  const floating = { horizontal: { reference: 'page', alignment: 'center' }, vertical: { reference: 'page', alignment: 'center' } }
  for (const options of [
    { autoFit: true, widthEmu: 100 }, { autoFit: true, heightEmu: 100 },
    { autoFit: true, layout: floating }, { alignment: 'center', layout: floating },
    { alignment: 'justify' }, { autoFit: true, imageBytes: Buffer.from('bad image') },
  ]) {
    const result = await executeDocxInsertPicture(createBlankDocx(), { imageBytes: png, placement: { kind: 'end' }, ...options })
    assert.equal(result.result.ok, false); assert.equal(result.output, undefined)
  }
  const legacy = await applyPicture(createBlankDocx(), { imageBytes: png, autoFit: false, widthEmu: 123, heightEmu: 456 })
  assert.equal((await layout(legacy)).images[0].displayWidthEmu, 123)
  assert.equal((await layout(legacy)).images[0].displayHeightEmu, 456)
})
