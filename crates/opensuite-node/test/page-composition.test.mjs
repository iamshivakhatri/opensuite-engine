import assert from 'node:assert/strict'
import { test } from 'node:test'
import binding from '../index.js'

const {
  createBlankDocx,
  executeDocxDeletePageBreak,
  executeDocxInsertPageBreak,
  executeDocxSetHeaderFooterText,
  executeDocxSetPageNumber,
  executeDocxSetPageSetup,
  inspectDocx,
} = binding

test('composes page layout through native bindings', async () => {
  let bytes = createBlankDocx()

  let result = await executeDocxInsertPageBreak(bytes, { placement: { kind: 'end' } })
  assert.equal(result.result.ok, true)
  bytes = result.output
  const blocks = await inspectDocx(bytes, { focus: { kind: 'body_blocks', offset: 0, limit: 10 } })
  assert.equal(blocks.bodyBlocks.items[0].kind, 'page_break')

  result = await executeDocxDeletePageBreak(bytes, { handle: blocks.bodyBlocks.items[0].handle })
  assert.equal(result.result.ok, true)
  bytes = result.output
  assert.equal((await inspectDocx(bytes, { focus: { kind: 'body_blocks', offset: 0, limit: 10 } })).bodyBlocks.items.length, 0)

  result = await executeDocxSetPageSetup(bytes, {
    paperSize: 'a4', orientation: 'landscape', topMarginTwips: 720, leftMarginTwips: 720,
  })
  assert.equal(result.result.ok, true)
  bytes = result.output

  result = await executeDocxSetHeaderFooterText(bytes, { kind: 'header', text: 'OpenSuite report' })
  assert.equal(result.result.ok, true)
  bytes = result.output
  result = await executeDocxSetHeaderFooterText(bytes, { kind: 'footer', text: 'Confidential' })
  assert.equal(result.result.ok, true)
  bytes = result.output

  for (const alignment of ['center', 'right', undefined]) {
    result = await executeDocxSetPageNumber(bytes, { kind: 'footer', alignment })
    assert.equal(result.result.ok, true)
    bytes = result.output
  }
})

test('returns a structured page-break failure', async () => {
  const result = await executeDocxDeletePageBreak(createBlankDocx(), { handle: 'b99' })
  assert.equal(result.result.ok, false)
  assert.equal(result.result.diagnostics[0].code, 'TARGET_NOT_FOUND')
  assert.equal(result.output, undefined)
})
