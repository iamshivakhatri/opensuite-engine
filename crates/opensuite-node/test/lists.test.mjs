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

test('groups disjoint bullet runs without relaxing decimal or source order', async () => {
  let bytes = (await executeDocxInsertParagraphs(createBlankDocx(), {
    texts: ['First', 'Second', 'Section', 'Third', 'Fourth'], placement: { kind: 'end' },
  })).output
  const targets = ['First', 'Second', 'Third', 'Fourth'].map((text) => ({ text }))
  const bullets = await executeDocxSetParagraphsList(bytes, { targets, kind: 'bullet' })
  assert.equal(bullets.result.ok, true)
  bytes = bullets.output
  const inspected = await inspectDocx(bytes, { focus: { kind: 'paragraphs' } })
  assert.deepEqual(inspected.paragraphs.items.map(({ text, list }) => [text, list?.kind ?? null]), [
    ['First', 'bullet'], ['Second', 'bullet'], ['Section', null], ['Third', 'bullet'], ['Fourth', 'bullet'],
  ])

  const decimal = await executeDocxSetParagraphsList(bytes, { targets, kind: 'decimal' })
  assert.equal(decimal.result.ok, false)
  const reordered = await executeDocxSetParagraphsList(bytes, { targets: [...targets].reverse(), kind: 'bullet' })
  assert.equal(reordered.result.ok, false)
})
