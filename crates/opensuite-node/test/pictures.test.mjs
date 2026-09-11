import assert from 'node:assert/strict'
import { test } from 'node:test'
import binding from '../index.js'

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
