import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFileSync, writeFileSync } from 'node:fs'
import engine from '../index.js'
import { documentXml, editDocument, editParts, image, packageParts } from './picture-support.mjs'

const images = async (bytes, options) => JSON.parse(await engine.inspectDocxImages(bytes, options))
const sections = async bytes => JSON.parse(await engine.inspectDocxSections(bytes)).sections
async function apply(bytes, method, operation) {
  const result = await engine[method](bytes, operation)
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics)); return result.output
}
async function target(bytes, kind = 'header', variant = 'default', index = 0) {
  return { kind, variant, handle: (await sections(bytes))[index].handle }
}
async function insert(bytes, placement, options = {}) {
  return apply(bytes, 'executeDocxInsertPicture', { imageBytes: image(240, 120), placement, alignment: 'center', autoFit: true, ...options })
}
function changePart(bytes, name, edit) {
  return editParts(bytes, parts => ({ ...parts, [name]: Buffer.from(edit(parts[name].toString())) }))
}
function withoutInsertion(xml) { return xml.replace(/<w:p><w:pPr><w:jc w:val="center"\/><\/w:pPr><w:r><w:drawing>[^]*?<\/w:p>/g, '') }

test('header/footer insertion preserves formatted text, PAGE fields, relationships, and body content', async () => {
  let bytes = engine.createBlankDocx()
  bytes = await apply(bytes, 'executeDocxSetSectionHeaderFooter', { ...(await target(bytes, 'footer')), action: 'text', text: 'Existing footer text' })
  bytes = await apply(bytes, 'executeDocxSetSectionHeaderFooter', { ...(await target(bytes, 'footer')), action: 'pageNumber', alignment: 'right' })
  bytes = changePart(bytes, 'word/footer1.xml', xml => xml.replace('<w:r>', '<w:r><w:rPr><w:b/><w:color w:val="284060"/></w:rPr>'))
  const before = packageParts(bytes), body = documentXml(bytes)
  bytes = await insert(bytes, await target(bytes, 'footer'))
  const after = packageParts(bytes)
  assert.equal(documentXml(bytes), body)
  assert.equal(withoutInsertion(after['word/footer1.xml'].toString()), before['word/footer1.xml'].toString())
  for (const [name, payload] of Object.entries(before)) if (name !== 'word/footer1.xml' && name !== '[Content_Types].xml') assert.deepEqual(after[name], payload, name)
  const facts = await images(bytes)
  assert.equal(facts.total, 1); assert.equal(facts.images[0].partName, '/word/footer1.xml')
  assert.equal(facts.images[0].intrinsicWidthPx, 240)
  assert.equal(facts.images[0].displayWidthEmu, facts.images[0].displayHeightEmu * 2)
  assert.equal(JSON.parse(await engine.inspectDocxFields(bytes)).fields[0].instruction.trim(), 'PAGE')
  const read = await engine.readDocxEmbeddedImage(bytes, { handle: facts.images[0].handle })
  assert.equal(read.result.ok, true); assert.deepEqual(read.imageBytes, image(240, 120)); assert.deepEqual(read.result.changes, [])
  writeFileSync('/private/tmp/opensuite-e3-footer.docx', bytes)
  assert.deepEqual((await images(readFileSync('/private/tmp/opensuite-e3-footer.docx'))).images, facts.images)
})

test('default, first, even, and missing parts use only explicitly selected section variants', async () => {
  let bytes = engine.createBlankDocx()
  bytes = await apply(bytes, 'executeDocxSetSectionProperties', { handle: (await sections(bytes))[0].handle, differentFirstPage: true, pageSetup: { topMarginTwips: 2160, bottomMarginTwips: 2160 } })
  bytes = await apply(bytes, 'executeDocxSetOddEvenHeaders', { enabled: true })
  for (const variant of ['default', 'first', 'even']) bytes = await insert(bytes, await target(bytes, 'header', variant), { altText: `${variant} logo` })
  for (const variant of ['default', 'first', 'even']) {
    bytes = await apply(bytes, 'executeDocxSetSectionHeaderFooter', { ...(await target(bytes, 'footer', variant)), action: 'pageNumber', alignment: 'right' })
    bytes = await insert(bytes, await target(bytes, 'footer', variant), { altText: `${variant} footer logo` })
  }
  const facts = await images(bytes)
  assert.equal(facts.total, 6)
  const state = await sections(bytes)
  assert.equal(state[0].differentFirstPage, true); assert.equal(state[0].oddEvenHeaders, true)
  const parts = packageParts(bytes)
  for (const record of facts.images) {
    assert.ok(record.handle); assert.equal(record.format, 'png'); assert.deepEqual((await engine.readDocxEmbeddedImage(bytes, { handle: record.handle })).imageBytes, image(240, 120))
    const file = record.partName.slice(1), rel = file.replace(/([^/]+)$/, '_rels/$1.rels')
    assert.match(parts[rel].toString(), /Type="[^\"]*\/image" Target="media\/image\d+.png"/)
    assert.equal(new Set([...parts[rel].toString().matchAll(/ Id="([^"]+)"/g)].map(m => m[1])).size, 1)
  }
  assert.equal(new Set(facts.images.map(i => i.mediaPartName)).size, 6)
  bytes = await apply(bytes, 'executeDocxInsertParagraph', { text: 'First page: first header. Body and footer stay visible.', placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxInsertPageBreak', { placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxInsertParagraph', { text: 'Second page: even header and page footer.', placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxInsertPageBreak', { placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxInsertParagraph', { text: 'Third page: default header and page footer.', placement: { kind: 'end' } })
  writeFileSync('/private/tmp/opensuite-e3-variants.docx', bytes)
})

test('separate section parts remain separate; shared/inherited parts refuse without output', async () => {
  let bytes = engine.createBlankDocx()
  bytes = await apply(bytes, 'executeDocxSetSectionHeaderFooter', { ...(await target(bytes)), action: 'text', text: 'First section header' })
  bytes = await apply(bytes, 'executeDocxInsertSectionBreak', { placement: { kind: 'end' }, breakType: 'nextPage' })
  for (const index of [0,1]) {
    const refused = await engine.executeDocxInsertPicture(bytes, { imageBytes: image(32,16), placement: await target(bytes, 'header', 'default', index) })
    assert.equal(refused.result.ok, false); assert.equal(refused.output, undefined)
  }
  bytes = await apply(bytes, 'executeDocxSetSectionHeaderFooter', { ...(await target(bytes, 'header', 'default', 1)), action: 'text', text: 'Second section header' })
  const before = await sections(bytes)
  bytes = await insert(bytes, await target(bytes, 'header', 'default', 0))
  bytes = await insert(bytes, await target(bytes, 'header', 'default', 1))
  const after = await sections(bytes)
  assert.equal(after.length, 2)
  for (const index of [0,1]) assert.deepEqual(after[index].headersFooters.map(v => [v.kind,v.variant,v.partName,v.linkedToPrevious]), before[index].headersFooters.map(v => [v.kind,v.variant,v.partName,v.linkedToPrevious]))
  const main = documentXml(bytes), facts = await images(bytes)
  assert.equal(facts.total, 2); assert.notEqual(facts.images[0].partName, facts.images[1].partName)
  const owned = after[1].headersFooters.find(v => v.kind === 'header' && v.variant === 'default')
  const shared = editDocument(bytes, xml => xml.replace(`r:id="${owned.relationshipId}"`, `r:id="${after[0].headersFooters.find(v => v.kind === 'header' && v.variant === 'default').relationshipId}"`))
  const refused = await engine.executeDocxInsertPicture(shared, { imageBytes: image(32,16), placement: await target(shared) })
  assert.equal(refused.result.ok, false); assert.equal(refused.output, undefined)
  assert.equal(documentXml(bytes), main)
  writeFileSync('/private/tmp/opensuite-e3-sections.docx', bytes)
  // Missing first-section footer creation keeps the following section's empty footer independent.
  let missing = await apply(engine.createBlankDocx(), 'executeDocxInsertSectionBreak', { placement: { kind: 'end' }, breakType: 'nextPage' })
  missing = await insert(missing, await target(missing, 'footer'))
  const missingState = await sections(missing)
  assert.notEqual(missingState[0].headersFooters.find(v => v.kind === 'footer' && v.variant === 'default').partName, missingState[1].headersFooters.find(v => v.kind === 'footer' && v.variant === 'default').partName)
})

test('image retrieval supports body, table cells, headers, floating imports, paging and shared media', async () => {
  let bytes = await insert(engine.createBlankDocx(), { kind: 'end' }, { altText: 'Body image' })
  bytes = await apply(bytes, 'executeDocxCreateTable', { rows: [['Item','Picture'],['Logo','Caption']], placement: { kind: 'end' } })
  const table = (await engine.inspectDocx(bytes, { focus: { kind: 'tables' } })).tables.items[0]
  bytes = await insert(bytes, { kind: 'tableCell', handle: table.rows[1].pictureCellHandles[1] })
  bytes = await insert(bytes, await target(bytes))
  const original = Buffer.from(bytes), before = packageParts(bytes), facts = await images(bytes)
  assert.equal(facts.total, 3)
  assert.deepEqual((await images(bytes)).images, facts.images)
  const page = await images(bytes, { offset: 1, limit: 1 })
  assert.equal(page.images.length, 1); assert.equal(page.hasMore, true); assert.equal(page.images[0].handle, facts.images[1].handle)
  for (const record of facts.images) assert.deepEqual((await engine.readDocxEmbeddedImage(bytes, { handle: record.handle })).imageBytes, image(240,120))
  assert.deepEqual(bytes, original); assert.deepEqual(packageParts(bytes), before)
  const shared = changePart(bytes, 'word/_rels/document.xml.rels', xml => xml.replace('media/image2.png', 'media/image1.png'))
  const sharedFacts = await images(shared)
  assert.equal(sharedFacts.images[0].mediaPartName, sharedFacts.images[1].mediaPartName)
  assert.notEqual(sharedFacts.images[0].handle, sharedFacts.images[1].handle)
  for (const record of sharedFacts.images) assert.equal((await engine.readDocxEmbeddedImage(shared, { handle: record.handle })).result.ok, true)
  // A bad, unrelated media entry is never decoded during selection of another image.
  const unrelated = editParts(bytes, parts => ({ ...parts, 'word/media/image2.png': Buffer.from('invalid unrelated image') }))
  assert.deepEqual((await engine.readDocxEmbeddedImage(unrelated, { handle: facts.images[0].handle })).imageBytes, image(240,120))
  const imported = readFileSync(new URL('../../../tests/fixtures/libreoffice-floating-image.docx', import.meta.url))
  const importedFacts = await images(imported)
  assert.equal(importedFacts.images[0].kind, 'floating')
  const importedRead = await engine.readDocxEmbeddedImage(imported, { handle: importedFacts.images[0].handle })
  assert.deepEqual(importedRead.imageBytes, packageParts(imported)[importedFacts.images[0].mediaPartName.slice(1)])
  assert.ok(importedFacts.images[0].anchor)
})

test('retrieval rejects stale identities, linked/malformed/unsupported/oversized media and retains bytes', async () => {
  const bytes = await insert(engine.createBlankDocx(), { kind: 'end' })
  const fact = (await images(bytes)).images[0], original = Buffer.from(bytes)
  for (const handle of ['', 'p0', fact.handle.replace(/:[^:]+$/, ':invalid'), fact.handle.replace(':0:', ':999:')]) {
    const result = await engine.readDocxEmbeddedImage(bytes, { handle }); assert.equal(result.result.ok, false); assert.equal(result.imageBytes, undefined)
  }
  const changedXml = editDocument(bytes, xml => xml.replace('Picture 1', 'Changed picture'))
  const changedRels = changePart(bytes, 'word/_rels/document.xml.rels', xml => xml.replace('<Relationships ', '<Relationships changed="yes" '))
  const changedMedia = editParts(bytes, parts => ({ ...parts, 'word/media/image1.png': image(120,60) }))
  for (const changed of [changedXml, changedRels, changedMedia]) {
    const result = await engine.readDocxEmbeddedImage(changed, { handle: fact.handle }); assert.equal(result.result.ok, false); assert.equal(result.imageBytes, undefined)
  }
  for (const [changed, code] of [
    [editDocument(bytes, xml => xml.replace('r:embed=', 'r:link=')), 'LINKED_IMAGE'],
    [editDocument(bytes, xml => xml.replace('r:embed=', 'r:link="linked" r:embed=')), 'LINKED_IMAGE'],
    [changePart(bytes, 'word/_rels/document.xml.rels', xml => xml.replace('</Relationships>', xml.match(/<Relationship\b[^>]*\/image"[^>]*\/>/)[0] + '</Relationships>')), 'AMBIGUOUS_IMAGE_RELATIONSHIP'],
    [changePart(editDocument(bytes, xml => xml.replace('r:embed=', 'r:link=')), 'word/_rels/document.xml.rels', xml => xml.replace('Target="media/image1.png"', 'Target="https://example.invalid/image.png" TargetMode="External"')), 'LINKED_IMAGE'],
    [changePart(bytes, 'word/_rels/document.xml.rels', xml => xml.replace('Target="media/image1.png"', 'Target="https://example.invalid/image.png" TargetMode="External"')), 'EXTERNAL_EMBEDDED_IMAGE'],
    [editDocument(bytes, xml => xml.replace('r:embed="rId', 'r:embed="missing-rId')), 'MISSING_IMAGE_RELATIONSHIP'],
    [changePart(bytes, 'word/_rels/document.xml.rels', xml => xml.replace('/relationships/image', '/relationships/hyperlink')), 'WRONG_IMAGE_RELATIONSHIP_TYPE'],
    [changePart(bytes, 'word/_rels/document.xml.rels', xml => xml.replace('media/image1.png', 'media/missing.png')), 'MISSING_TARGET_PART'],
    [changePart(bytes, '[Content_Types].xml', xml => xml.replace('image/png', 'image/gif')), 'UNSUPPORTED_IMAGE_FORMAT'],
    [editParts(bytes, parts => ({ ...parts, 'word/media/image1.png': Buffer.alloc(20*1024*1024+1) })), 'IMAGE_SIZE_LIMIT'],
    [editParts(bytes, parts => { const large = image(1,1); large.writeUInt32BE(100000,16); large.writeUInt32BE(100000,20); return { ...parts, 'word/media/image1.png': large } }), 'IMAGE_SIZE_LIMIT'],
  ]) {
    const facts = await images(changed); assert.equal(facts.total, 1); assert.equal(facts.images[0].handle, null); assert.ok(facts.images[0].diagnostics.includes(code), JSON.stringify(facts))
    const result = await engine.readDocxEmbeddedImage(changed, { handle: fact.handle }); assert.equal(result.result.ok, false); assert.equal(result.imageBytes, undefined)
  }
  assert.deepEqual(bytes, original)
  const capability = engine.getDocxCapabilities().formats[0].operations.find(v => v.id === 'read_embedded_image')
  assert.equal(capability.backendOnly, true); assert.equal(capability.jsonInput, false); assert.equal(capability.jsonOutput, false)
})

test('header insertion refuses inactive variants, stale handles, floats, rich roots and unsafe geometry', async () => {
  const bytes = engine.createBlankDocx(), placement = await target(bytes)
  for (const options of [
    { placement: { ...placement, variant: undefined } }, { placement: { ...placement, variant: 'first' } }, { placement: { ...placement, variant: 'even' } },
    { placement: { ...placement, position: 'end' } }, { placement: { ...placement, handle: 's0:bad' } },
    { placement: { kind: 'end', variant: 'default' } }, { widthEmu: 100, autoFit: true },
    { layout: { horizontal: { reference: 'page', alignment: 'start' }, vertical: { reference: 'page', alignment: 'start' } } },
  ]) {
    const result = await engine.executeDocxInsertPicture(bytes, { imageBytes: image(32,16), placement, ...options }); assert.equal(result.result.ok, false); assert.equal(result.output, undefined)
  }
  const seeded = await insert(bytes, placement)
  assert.equal((await engine.executeDocxInsertPicture(seeded, { imageBytes: image(32,16), placement })).result.ok, false)
  for (const edit of [xml => xml.replace('</w:hdr>', '<w:tbl/></w:hdr>'), xml => xml.replace('</w:hdr>', '<w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r></w:p></w:hdr>')]) {
    const changed = changePart(seeded, 'word/header1.xml', edit)
    const result = await engine.executeDocxInsertPicture(changed, { imageBytes: image(32,16), placement: await target(changed) }); assert.equal(result.result.ok, false); assert.equal(result.output, undefined)
  }
  for (const name of ['word/_rels/document.xml.rels', 'word/_rels/header1.xml.rels']) {
    const changed = changePart(seeded, name, xml => xml.replace('</Relationships>', xml.match(/<Relationship\b[^>]*\/>/)[0] + '</Relationships>'))
    const result = await engine.executeDocxInsertPicture(changed, { imageBytes: image(32,16), placement: await target(changed) })
    assert.equal(result.result.ok, false); assert.equal(result.output, undefined)
  }
  const impossible = editDocument(bytes, xml => xml.replace('w:header="720"', 'w:header="2000"'))
  assert.equal((await engine.executeDocxInsertPicture(impossible, { imageBytes: image(32,16), placement: await target(impossible) })).result.ok, false)
})

test('header image insertion preserves imported floating drawings and unfamiliar DrawingML', async () => {
  let bytes = readFileSync(new URL('../../../tests/fixtures/libreoffice-floating-image.docx', import.meta.url))
  bytes = editDocument(bytes, xml => xml.replace('</pic:spPr>', '<a:extLst><a:ext uri="urn:e3"><e3:keep xmlns:e3="urn:opensuite:e3">preserve</e3:keep></a:ext></a:extLst></pic:spPr>'))
  const before = packageParts(bytes), drawing = documentXml(bytes).match(/<w:drawing\b[^]*?<\/w:drawing>/)[0]
  bytes = await insert(bytes, await target(bytes))
  assert.ok(documentXml(bytes).includes(drawing))
  const after = packageParts(bytes)
  for (const [name, payload] of Object.entries(before)) if (!['word/document.xml', 'word/_rels/document.xml.rels', '[Content_Types].xml'].includes(name)) assert.deepEqual(after[name], payload, name)
  assert.equal((await images(bytes)).total, 2)
  writeFileSync('/private/tmp/opensuite-e3-imported.docx', bytes)
})

test('image relationships use the actual owning header part directory', async () => {
  let bytes = engine.createBlankDocx()
  bytes = await apply(bytes, 'executeDocxSetSectionHeaderFooter', { ...(await target(bytes)), action: 'text', text: 'Custom part location' })
  bytes = editParts(bytes, parts => {
    parts['custom/header.xml'] = parts['word/header1.xml']; delete parts['word/header1.xml']
    parts['[Content_Types].xml'] = Buffer.from(parts['[Content_Types].xml'].toString().replace('/word/header1.xml', '/custom/header.xml'))
    parts['word/_rels/document.xml.rels'] = Buffer.from(parts['word/_rels/document.xml.rels'].toString().replace('Target="header1.xml"', 'Target="../custom/header.xml"'))
    return parts
  })
  bytes = await insert(bytes, await target(bytes))
  const record = (await images(bytes)).images[0]
  assert.equal(record.partName, '/custom/header.xml')
  assert.match(packageParts(bytes)['custom/_rels/header.xml.rels'].toString(), /Target="\/word\/media\/image1.png"/)
  assert.deepEqual((await engine.readDocxEmbeddedImage(bytes, { handle: record.handle })).imageBytes, image(240,120))
})
