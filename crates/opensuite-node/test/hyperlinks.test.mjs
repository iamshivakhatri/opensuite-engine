import assert from 'node:assert/strict'
import { test } from 'node:test'
import binding from '../index.js'

const { createBlankDocx, executeDocxInsertParagraph, executeDocxSetHyperlink, inspectDocx } = binding

test('applies, replaces, and clears a hyperlink', async () => {
  let bytes = (await executeDocxInsertParagraph(createBlankDocx(), {
    text: 'OpenSuite', placement: { kind: 'end' },
  })).output

  for (const url of ['https://example.com/one', 'https://example.com/two', undefined]) {
    const result = await executeDocxSetHyperlink(bytes, { target: { text: 'OpenSuite' }, url })
    assert.equal(result.result.ok, true)
    bytes = result.output
  }

  const paragraphs = await inspectDocx(bytes, { focus: { kind: 'paragraphs', offset: 0, limit: 10 } })
  assert.deepEqual(paragraphs.paragraphs.items.map((item) => item.text), ['OpenSuite'])
})
