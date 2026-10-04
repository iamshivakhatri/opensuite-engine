import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFileSync, writeFileSync } from 'node:fs'
import binding from '../index.js'
const png = readFileSync(new URL('../../../tests/fixtures/image-layout.png', import.meta.url))
const replacement = readFileSync(new URL('../../../tests/fixtures/image-layout-replacement.png', import.meta.url))
const inspect = async bytes => JSON.parse(await binding.inspectDocxLayout(bytes, { blockLimit: 100 }))
const floating = {
  horizontal: { reference: 'margin', alignment: 'end' },
  vertical: { reference: 'paragraph', offsetEmu: 0 },
  wrap: 'square', distance: { leftEmu: 114300, rightEmu: 114300, topEmu: 57150, bottomEmu: 57150 },
}
async function apply(bytes, method, operation) {
  const result = await binding[method](bytes, operation)
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics)); return result.output
}
function artifact(name, bytes, facts) {
  writeFileSync(`/private/tmp/opensuite-image-${name}.docx`, bytes)
  writeFileSync(`/private/tmp/opensuite-image-${name}.json`, JSON.stringify(facts, null, 2))
}

test('inline and floating image dogfood, replacement and three-section ownership', async () => {
  let inline = await apply(binding.createBlankDocx(), 'executeDocxInsertParagraphs', { texts: ['Image layout report', 'This image uses inline placement with a proportional resize.'], placement: { kind: 'end' } })
  inline = await apply(inline, 'executeDocxSetParagraphStyle', { target: { text: 'Image layout report' }, style: 'Heading 1' })
  inline = await apply(inline, 'executeDocxInsertPicture', { imageBytes: png, placement: { kind: 'end' }, widthEmu: 1828800, altText: 'Two-to-one image' })
  const before = (await inspect(inline)).images[0]
  assert.equal(before.kind, 'inline'); assert.equal(before.displayHeightEmu, 914400); assert.equal(before.aspectRatio, 2)
  inline = await apply(inline, 'executeDocxSetPictureSize', { handle: before.handle, heightEmu: 1371600 })
  const smallStart = performance.now(); const small = await inspect(inline); const smallMs = performance.now() - smallStart
  assert.equal(small.images[0].displayWidthEmu, 2743200); artifact('inline', inline, small)

  let wrapped = await apply(binding.createBlankDocx(), 'executeDocxInsertParagraph', { text: 'Floating image on the right', placement: { kind: 'end' } })
  wrapped = await apply(wrapped, 'executeDocxSetParagraphStyle', { target: { text: 'Floating image on the right' }, style: 'Heading 1' })
  const body = Array.from({ length: 7 }, (_, i) => `Paragraph ${i + 1}. This sample describes how a floating image remains attached to a paragraph while the surrounding text flows around its right-side placement. The declared size, spacing, and wrap mode can be inspected without estimating Word's final rendered coordinates.`)
  wrapped = await apply(wrapped, 'executeDocxInsertParagraphs', { texts: body, placement: { kind: 'end' } })
  wrapped = await apply(wrapped, 'executeDocxInsertPicture', { imageBytes: png, placement: { kind: 'after', handle: 'b0' }, widthEmu: 1828800, layout: floating, altText: 'Floating square-wrap illustration' })
  for (const wrap of ['topAndBottom', 'behindText', 'inFrontOfText']) {
    const sample = await apply(binding.createBlankDocx(), 'executeDocxInsertPicture', { imageBytes: png, placement: { kind: 'end' }, widthEmu: 914400, layout: { ...floating, wrap } })
    assert.equal((await inspect(sample)).images[0].anchor.wrap, wrap)
  }
  const original = (await inspect(wrapped)).images[0]
  assert.equal(original.anchor.wrap, 'square'); assert.equal(original.anchor.horizontal.alignment, 'right')
  assert.equal(original.anchor.vertical.reference, 'paragraph'); artifact('floating', wrapped, await inspect(wrapped))
  const start = performance.now()
  let updated = await apply(wrapped, 'executeDocxSetPictureLayout', { handle: original.handle, layout: { horizontal: { reference: 'page', alignment: 'center' }, wrap: 'topAndBottom' } })
  const mutationMs = performance.now() - start
  const centered = (await inspect(updated)).images[0]
  assert.equal(centered.anchor.horizontal.reference, 'page'); assert.equal(centered.anchor.horizontal.alignment, 'center'); assert.equal(centered.anchor.wrap, 'topAndBottom')
  artifact('centered', updated, await inspect(updated))
  const stale = await binding.executeDocxSetPictureLayout(updated, { handle: original.handle, layout: { wrap: 'behindText' } })
  assert.equal(stale.result.ok, false); assert.equal(stale.output, undefined)
  for (const operation of [
    { horizontal: { reference: 'page', alignment: 'center', offsetEmu: 0 } },
    { vertical: { reference: 'column', offsetEmu: 0 } },
    { wrap: 'tight' }, { distance: { leftEmu: -1 } },
  ]) {
    const invalid = await binding.executeDocxSetPictureLayout(updated, { handle: centered.handle, layout: operation })
    assert.equal(invalid.result.ok, false); assert.equal(invalid.output, undefined)
  }
  updated = await apply(updated, 'executeDocxReplacePicture', { handle: centered.handle, replacementBytes: replacement, contentType: 'image/png' })
  const replaced = (await inspect(updated)).images[0]
  assert.deepEqual(replaced.anchor, centered.anchor); assert.equal(replaced.displayWidthEmu, centered.displayWidthEmu); assert.equal(replaced.displayHeightEmu, centered.displayHeightEmu)
  assert.equal(replaced.intrinsicWidthPixels, 240); artifact('replacement', updated, await inspect(updated))

  let multi = await apply(binding.createBlankDocx(), 'executeDocxInsertParagraph', { text: 'Portrait first section', placement: { kind: 'end' } })
  multi = await apply(multi, 'executeDocxInsertPicture', { imageBytes: png, placement: { kind: 'end' }, widthEmu: 1828800 })
  for (const text of ['Landscape middle section', 'Portrait final section']) {
    multi = await apply(multi, 'executeDocxInsertSectionBreak', { placement: { kind: 'end' }, breakType: 'nextPage' })
    multi = await apply(multi, 'executeDocxInsertParagraph', { text, placement: { kind: 'end' } })
    multi = await apply(multi, 'executeDocxInsertPicture', { imageBytes: png, placement: { kind: 'end' }, widthEmu: 1828800, layout: floating })
  }
  const sections = JSON.parse(await binding.inspectDocxSections(multi)).sections
  multi = await apply(multi, 'executeDocxSetSectionProperties', { handle: sections[1].handle, pageSetup: { orientation: 'landscape' } })
  const facts = await inspect(multi)
  assert.deepEqual(facts.images.map(i => i.sectionIndex), [0, 1, 2]); assert.equal(facts.sections[1].orientation, 'landscape')
  artifact('multi-section', multi, facts)
  const wide = await apply(multi, 'executeDocxSetPictureSize', { handle: facts.images[0].handle, widthEmu: 9144000 })
  assert.ok((await inspect(wide)).diagnostics.some(d => d.code === 'IMAGE_WIDTH_EXCEEDS_PAGE'))
  let many = binding.createBlankDocx()
  for (let i = 0; i < 40; i++) many = await apply(many, 'executeDocxInsertPicture', { imageBytes: png, placement: { kind: 'end' }, widthEmu: 914400, ...(i % 2 ? { layout: floating } : {}) })
  const manyStart = performance.now(); const manyFacts = await inspect(many); const manyMs = performance.now() - manyStart
  assert.equal(manyFacts.imageCount, 40)
  const manyMutationStart = performance.now(); many = await apply(many, 'executeDocxSetPictureSize', { handle: manyFacts.images[30].handle, widthEmu: 1828800 }); const manyMutationMs = performance.now() - manyMutationStart
  assert.equal((await inspect(many)).images[30].displayHeightEmu, 914400)
  const timing = { smallMs, mutationMs, manyMs, manyMutationMs, imageCount: 40 }
  writeFileSync('/private/tmp/opensuite-image-timings.json', JSON.stringify(timing, null, 2)); console.log(timing)
})

test('imported LibreOffice anchor remains intact during proportional resize', async () => {
  const bytes = readFileSync(new URL('../../../tests/fixtures/libreoffice-floating-image.docx', import.meta.url))
  const original = (await inspect(bytes)).images[0]
  assert.equal(original.anchor.editable, true)
  const output = await apply(bytes, 'executeDocxSetPictureSize', { handle: original.handle, widthEmu: 2743200 })
  const facts = await inspect(output)
  assert.deepEqual(facts.images[0].anchor, original.anchor)
  assert.equal(facts.images[0].displayHeightEmu, 1371600)
  artifact('imported-update', output, facts)
})
