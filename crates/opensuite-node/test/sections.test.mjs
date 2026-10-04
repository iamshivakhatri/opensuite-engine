import assert from 'node:assert/strict'
import { test } from 'node:test'
import { writeFileSync } from 'node:fs'
import binding from '../index.js'

const inspect = async (bytes) => {
  const result = JSON.parse(await binding.inspectDocxSections(bytes))
  assert.equal(result.ok, true, JSON.stringify(result.diagnostics))
  return result.sections
}
async function apply(bytes, method, operation) {
  const result = await binding[method](bytes, operation)
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics))
  assert.ok(result.output)
  return result.output
}
async function sectionEdit(bytes, index, method, operation) {
  const sections = await inspect(bytes)
  return apply(bytes, method, { handle: sections[index].handle, ...operation })
}

test('three-section native dogfood reopens with independent page setup and variants', async () => {
  let bytes = binding.createBlankDocx()
  bytes = await apply(bytes, 'executeDocxInsertParagraphs', { texts: ['OpenSuite E1: Three-section report', 'Section 1: portrait overview', 'This document demonstrates independent Word sections.'], placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxSetParagraphStyle', { target: { text: 'OpenSuite E1: Three-section report' }, style: 'Title' })
  bytes = await apply(bytes, 'executeDocxSetHeaderFooterText', { kind: 'header', text: 'OpenSuite shared report header' })
  bytes = await apply(bytes, 'executeDocxInsertSectionBreak', { placement: { kind: 'end' }, breakType: 'nextPage' })
  bytes = await apply(bytes, 'executeDocxInsertParagraph', { text: 'Section 2: landscape comparison', placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxCreateTable', { rows: [['Area', 'Owner', 'Current result', 'Next action'], ['Sections', 'Engine', 'Independent page setup', 'Manual review'], ['Headers', 'Engine', 'Variant and inheritance support', 'Manual review']], placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxInsertSectionBreak', { placement: { kind: 'end' }, breakType: 'nextPage' })
  bytes = await apply(bytes, 'executeDocxInsertParagraphs', { texts: ['Section 3: portrait conclusion', 'This section links its header to section 2 and owns its footer.'], placement: { kind: 'end' } })
  for (let index = 0; index < 3; index++) {
    bytes = await sectionEdit(bytes, index, 'executeDocxSetSectionProperties', { pageSetup: { paperSize: 'letter', orientation: index === 1 ? 'landscape' : 'portrait', topMarginTwips: 1440, bottomMarginTwips: 1440, leftMarginTwips: index === 1 ? 720 : 1440, rightMarginTwips: index === 1 ? 720 : 1440 }, ...(index === 0 ? { differentFirstPage: true } : {}) })
  }
  bytes = await sectionEdit(bytes, 0, 'executeDocxSetSectionHeaderFooter', { kind: 'header', variant: 'first', action: 'text', text: 'OpenSuite E1 cover' })
  bytes = await sectionEdit(bytes, 1, 'executeDocxSetSectionHeaderFooter', { kind: 'header', variant: 'default', action: 'text', text: 'Landscape comparison' })
  bytes = await sectionEdit(bytes, 2, 'executeDocxSetSectionHeaderFooter', { kind: 'header', variant: 'default', action: 'inherit' })
  bytes = await sectionEdit(bytes, 2, 'executeDocxSetSectionHeaderFooter', { kind: 'footer', variant: 'default', action: 'text', text: 'OpenSuite conclusion' })
  bytes = await apply(bytes, 'executeDocxSetOddEvenHeaders', { enabled: true })
  bytes = await sectionEdit(bytes, 1, 'executeDocxSetSectionHeaderFooter', { kind: 'header', variant: 'even', action: 'text', text: 'Landscape: even page' })
  bytes = await sectionEdit(bytes, 1, 'executeDocxSetSectionHeaderFooter', { kind: 'footer', variant: 'default', action: 'pageNumber', alignment: 'center' })
  bytes = await sectionEdit(bytes, 1, 'executeDocxSetSectionProperties', { pageNumberStart: 1 })
  bytes = await sectionEdit(bytes, 2, 'executeDocxSetSectionHeaderFooter', { kind: 'footer', variant: 'default', action: 'pageNumber', alignment: 'right' })
  bytes = await sectionEdit(bytes, 2, 'executeDocxSetSectionProperties', { continuePageNumbering: true })
  const sections = await inspect(bytes)
  assert.equal(sections.length, 3)
  assert.deepEqual(sections.map(s => s.orientation), ['portrait', 'landscape', 'portrait'])
  assert.deepEqual(sections.map(s => s.pageWidthTwips), [12240, 15840, 12240])
  assert.equal(sections[0].differentFirstPage, true)
  assert.equal(sections[1].headersFooters[0].text, 'Landscape comparison')
  assert.equal(sections[2].headersFooters[0].linkedToPrevious, true)
  assert.equal(sections[2].headersFooters[0].text, 'Landscape comparison')
  assert.equal(sections[2].headersFooters[3].text, 'OpenSuite conclusion')
  assert.equal(sections[1].pageNumberStart, 1)
  assert.equal(sections[2].pageNumberStart, null)
  const artifact = '/private/tmp/opensuite-e1-three-sections.docx'
  writeFileSync(artifact, bytes)
  writeFileSync('/private/tmp/opensuite-e1-three-sections.json', JSON.stringify(sections, null, 2))

  // Updating an existing multi-section document must preserve section 1 facts/content.
  const beforeBlocks = await binding.inspectDocx(bytes, { focus: { kind: 'body_blocks' } })
  const oldHandle = sections[2].handle
  bytes = await sectionEdit(bytes, 1, 'executeDocxSetSectionProperties', { pageSetup: { topMarginTwips: 900 } })
  bytes = await sectionEdit(bytes, 2, 'executeDocxSetSectionHeaderFooter', { kind: 'footer', variant: 'default', action: 'text', text: 'Updated conclusion' })
  const updated = await inspect(bytes)
  assert.deepEqual({ ...updated[0], handle: '' }, { ...sections[0], handle: '' })
  assert.deepEqual(await binding.inspectDocx(bytes, { focus: { kind: 'body_blocks' } }), beforeBlocks)
  const stale = await binding.executeDocxSetSectionProperties(bytes, { handle: oldHandle, differentFirstPage: true })
  assert.equal(stale.result.ok, false)
  assert.equal(stale.result.diagnostics[0].code, 'PRECONDITION_FAILED')
  assert.equal(stale.output, undefined)
})

test('section binding validates unsupported inputs without output', async () => {
  const bytes = binding.createBlankDocx()
  for (const type of ['invalid', 'nextColumn']) {
    const result = await binding.executeDocxInsertSectionBreak(bytes, { placement: { kind: 'end' }, breakType: type })
    assert.equal(result.result.ok, false)
    assert.equal(result.result.diagnostics[0].code, 'INVALID_OPERATION')
    assert.equal(result.output, undefined)
  }
  const invalid = JSON.parse(await binding.inspectDocxSections(Buffer.from('bad zip')))
  assert.equal(invalid.ok, false)
  assert.ok(invalid.diagnostics.length)
})

test('edits a python-docx multi-section fixture with shared imported headers', async () => {
  const { readFileSync } = await import('node:fs')
  let bytes = readFileSync(new URL('./fixtures/three-sections.docx', import.meta.url))
  const before = await inspect(bytes)
  assert.equal(before.length, 3)
  assert.equal(before[1].headersFooters[0].linkedToPrevious, true)
  const blocks = await binding.inspectDocx(bytes, { focus: { kind: 'body_blocks' } })
  bytes = await sectionEdit(bytes, 1, 'executeDocxSetSectionProperties', { pageSetup: { orientation: 'landscape' } })
  bytes = await sectionEdit(bytes, 2, 'executeDocxSetSectionHeaderFooter', { kind: 'footer', variant: 'default', action: 'text', text: 'Updated imported footer' })
  const after = await inspect(bytes)
  assert.deepEqual({ ...before[0], handle: '' }, { ...after[0], handle: '' })
  assert.equal(after[1].orientation, 'landscape')
  assert.equal(after[2].headersFooters[3].text, 'Updated imported footer')
  assert.deepEqual(await binding.inspectDocx(bytes, { focus: { kind: 'body_blocks' } }), blocks)
  writeFileSync('/private/tmp/opensuite-e1-imported-updated.docx', bytes)
})
