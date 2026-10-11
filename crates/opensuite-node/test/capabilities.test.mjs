import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import engine from '../index.js'

test('the engine inventory covers every native export and declaration', () => {
  const docx = engine.getDocxCapabilities().formats.find((format) => format.format === 'docx')
  assert.ok(docx.operations.length > 0)
  assert.equal(new Set(docx.capabilities).size, docx.capabilities.length)
  const names = new Set()
  for (const operation of docx.operations) {
    assert.ok(docx.capabilities.includes(operation.id), operation.id)
    assert.ok(['read', 'mutate', 'create'].includes(operation.kind), operation.id)
    assert.equal(typeof engine[operation.nodeExport], 'function', operation.nodeExport)
    names.add(operation.nodeExport)
  }
  const actual = Object.keys(engine).filter((key) => typeof engine[key] === 'function' && key !== 'getDocxCapabilities')
  assert.deepEqual([...names].sort(), actual.sort(), 'native exports must be recorded in the engine inventory')
  const types = readFileSync(new URL('../index.d.ts', import.meta.url), 'utf8')
  const declared = [...types.matchAll(/^export function (\w+)/gm)].map((match) => match[1])
  assert.deepEqual(declared.sort(), [...actual, 'getDocxCapabilities'].sort(), 'declarations must match the compiled engine')
  assert.ok(docx.capabilities.includes('semantic_table_cell_targets'))
  assert.ok(docx.capabilities.includes('semantic_table_row_deletion'))
  assert.ok(!docx.capabilities.includes('insert_paragraph_after'), 'Rust-only operation must not be advertised to Node')
  assert.ok(!docx.capabilities.includes('set_table_cell_text'), 'Rust-only operation must not be advertised to Node')
})
