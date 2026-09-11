import assert from 'node:assert/strict'
import { test } from 'node:test'
import binding from '../index.js'

const { createBlankDocx, executeDocxInsertParagraphs, executeDocxSetParagraphsList, inspectDocx } = binding

test('authors bullet and decimal lists, then clears them', async () => {
  let bytes = (await executeDocxInsertParagraphs(createBlankDocx(), {
    texts: ['One', 'Two', 'Three'], placement: { kind: 'end' },
  })).output

  for (const kind of ['bullet', 'decimal', 'none']) {
    const result = await executeDocxSetParagraphsList(bytes, {
      targets: [{ text: 'One' }, { text: 'Two' }, { text: 'Three' }], kind,
    })
    assert.equal(result.result.ok, true)
    bytes = result.output
  }
})

test('maps list levels and continuation through N-API', async () => {
  let bytes = (await executeDocxInsertParagraphs(createBlankDocx(), {
    texts: ['Top', 'Child', 'Grandchild'], placement: { kind: 'end' },
  })).output
  for (const [text, level] of [['Top', 0], ['Child', 1], ['Grandchild', 2]]) {
    const result = await executeDocxSetParagraphsList(bytes, {
      targets: [{ text }], kind: 'decimal', level, continueFromPrevious: level > 0,
    })
    assert.equal(result.result.ok, true)
    bytes = result.output
  }
  const inspected = await inspectDocx(bytes, { focus: { kind: 'paragraphs' } })
  assert.deepEqual(inspected.paragraphs.items.map(({ list }) => [list.kind, list.level, list.supported]), [
    ['decimal', 0, true], ['decimal', 1, true], ['decimal', 2, true],
  ])
})
