import assert from 'node:assert/strict'
import { test } from 'node:test'
import binding from '../index.js'

const { createBlankDocx, executeDocxInsertParagraph, executeDocxSetTextFormatting } = binding

async function style(bytes, operation) {
  const result = await executeDocxSetTextFormatting(bytes, {
    target: { text: 'Styled text' },
    ...operation,
  })
  assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics))
  assert.ok(Buffer.isBuffer(result.output))
  return result.output
}

test('maps text styling through the native DOCX execution path', async () => {
  let bytes = (await executeDocxInsertParagraph(createBlankDocx(), {
    text: 'Styled text', placement: { kind: 'end' },
  })).output

  bytes = await style(bytes, { color: '0057B8' })
  bytes = await style(bytes, { clearColor: true })
  bytes = await style(bytes, { underline: true })
  bytes = await style(bytes, { clearUnderline: true })
  bytes = await style(bytes, { highlight: 'yellow' })
  bytes = await style(bytes, { strikethrough: true })
  bytes = await style(bytes, { verticalAlignment: 'superscript' })
  bytes = await style(bytes, { verticalAlignment: 'subscript' })
  bytes = await style(bytes, { verticalAlignment: 'baseline' })
  await style(bytes, { clearVerticalAlignment: true })
})

test('returns Rust validation failures through the Node boundary', async () => {
  const input = (await executeDocxInsertParagraph(createBlankDocx(), {
    text: 'Styled text', placement: { kind: 'end' },
  })).output
  const result = await executeDocxSetTextFormatting(input, {
    target: { text: 'Styled text' }, color: 'not-a-color',
  })

  assert.equal(result.result.ok, false)
  assert.equal(result.result.diagnostics[0].code, 'INVALID_OPERATION')
  assert.equal(result.output, undefined)
})
