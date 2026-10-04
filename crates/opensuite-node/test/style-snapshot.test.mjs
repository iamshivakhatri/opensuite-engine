import assert from 'node:assert/strict'
import { test } from 'node:test'
import binding from '../index.js'

test('returns a structured read-only DOCX style snapshot', async () => {
  const docxCapabilities = binding.getDocxCapabilities().formats.find((format) => format.format === 'docx').capabilities
  const input = binding.createBlankDocx()
  const before = Buffer.from(input)
  const snapshot = JSON.parse(await binding.inspectDocxStyleSnapshot(input))

  assert.equal(snapshot.ok, true)
  assert.equal(snapshot.schemaVersion, 1)
  assert.equal(snapshot.tableCount, 0)
  assert.ok(docxCapabilities.includes('style_snapshot'))
  assert.deepEqual(input, before)
})
