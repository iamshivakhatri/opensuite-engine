import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFileSync, writeFileSync } from 'node:fs'
import binding from '../index.js'

async function apply(bytes, method, operation) {
  const r = await binding[method](bytes, operation)
  assert.equal(r.result.ok, true, JSON.stringify(r.result.diagnostics)); return r.output
}
const inspect = async (bytes, options) => JSON.parse(await binding.inspectDocxLayout(bytes, options))

test('layout dogfood reports geometry, three sections and wide content without changing bytes', async () => {
  let simple = binding.createBlankDocx()
  simple = await apply(simple, 'executeDocxInsertParagraphs', { texts: ['Layout report', 'Overview', 'Body text.'], placement: { kind: 'end' } })
  simple = await apply(simple, 'executeDocxSetParagraphStyle', { target: { text: 'Overview' }, style: 'Heading 1' })
  simple = await apply(simple, 'executeDocxCreateTable', { rows: [['Area', 'Result'], ['Layout', 'Structural inputs']], placement: { kind: 'end' } })
  const original = Buffer.from(simple)
  const start = performance.now(); const small = await inspect(simple, { blockLimit: 20 });
  const smallMs = performance.now() - start
  assert.equal(small.ok, true); assert.deepEqual(simple, original)
  assert.equal(small.sections[0].usableWidthTwips, 9360)
  assert.equal(small.renderedPageCount, null)
  assert.ok(small.paragraphPatterns.some(p => p.formatting.keepWithNext))
  writeFileSync('/private/tmp/opensuite-layout-simple.docx', simple)
  writeFileSync('/private/tmp/opensuite-layout-simple.json', JSON.stringify(small, null, 2))
  let multi = simple
  for (const text of ['Landscape comparison', 'Portrait conclusion']) {
    multi = await apply(multi, 'executeDocxInsertSectionBreak', { placement: { kind: 'end' }, breakType: 'nextPage' })
    multi = await apply(multi, 'executeDocxInsertParagraph', { text, placement: { kind: 'end' } })
  }
  const sections = JSON.parse(await binding.inspectDocxSections(multi)).sections
  multi = await apply(multi, 'executeDocxSetSectionProperties', { handle: sections[1].handle, pageSetup: { orientation: 'landscape', leftMarginTwips: 720, rightMarginTwips: 720 } })
  const current = JSON.parse(await binding.inspectDocxSections(multi)).sections
  multi = await apply(multi, 'executeDocxSetSectionHeaderFooter', { handle: current[1].handle, kind: 'header', variant: 'default', action: 'text', text: 'Landscape header' })
  const layout = await inspect(multi, { blockLimit: 100 })
  assert.equal(layout.sectionCount, 3)
  assert.equal(layout.sections[1].orientation, 'landscape')
  assert.equal(layout.sections[1].usableWidthTwips, 14400)
  assert.deepEqual(layout.sections.map(s => s.handle), JSON.parse(await binding.inspectDocxSections(multi)).sections.map(s => s.handle))
  assert.equal(layout.blocks.find(b => b.handle === 'b5').sectionIndex, 1)
  writeFileSync('/private/tmp/opensuite-layout-three-sections.docx', multi)
  writeFileSync('/private/tmp/opensuite-layout-three-sections.json', JSON.stringify(layout, null, 2))
  let wide = await apply(simple, 'executeDocxSetTableColumnWidths', { table: { headerCells: ['Area', 'Result'] }, widthsTwips: [7000, 7000] })
  const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL7WQAAAABJRU5ErkJggg==', 'base64')
  wide = await apply(wide, 'executeDocxInsertPicture', { imageBytes: png, placement: { kind: 'end' }, altText: 'Oversized layout image' })
  const picture = (await binding.inspectDocx(wide, { focus: { kind: 'body_blocks', limit: 100 } })).bodyBlocks.items.find(b => b.picture).picture
  wide = await apply(wide, 'executeDocxSetPictureSize', { handle: picture.handle, widthEmu: 8 * 914400 })
  const warnings = await inspect(wide)
  assert.ok(warnings.diagnostics.some(d => d.code === 'TABLE_WIDTH_EXCEEDS_PAGE'))
  assert.ok(warnings.diagnostics.some(d => d.code === 'IMAGE_WIDTH_EXCEEDS_PAGE'))
  assert.equal(warnings.images[0].intrinsicWidthPixels, 1)
  writeFileSync('/private/tmp/opensuite-layout-wide.docx', wide)
  writeFileSync('/private/tmp/opensuite-layout-wide.json', JSON.stringify(warnings, null, 2))
  const imported = readFileSync(new URL('../../../tests/fixtures/google-docs-table.docx', import.meta.url))
  const copy = Buffer.from(imported); assert.equal((await inspect(imported)).ok, true); assert.deepEqual(imported, copy)
  let large = binding.createBlankDocx()
  for (let index = 0; index < 10; index++) large = await apply(large, 'executeDocxInsertParagraphs', { texts: Array.from({ length: 100 }, (_, i) => `Body ${index * 100 + i}`), placement: { kind: 'end' } })
  const largeStart = performance.now(); const summary = await inspect(large); const largeMs = performance.now() - largeStart
  assert.equal(summary.paragraphCount, 1000); assert.equal(summary.blocks.length, 0)
  assert.equal(summary.paragraphPatterns.length, 1)
  assert.equal((await inspect(large, { blockLimit: 20, blockOffset: 900 })).blocks.length, 20)
  assert.equal((await inspect(large, { blockLimit: 101 })).ok, false)
  assert.equal((await inspect(large, { sectionIndex: 99 })).ok, false)
  console.log(JSON.stringify({ smallMs, largeMs, largeParagraphs: 1000 }))
})
