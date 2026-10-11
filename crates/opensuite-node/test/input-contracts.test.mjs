import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'
import engine from '../index.js'
import { inputContracts, rustIntegerFields } from '../scripts/generate-inputs.mjs'

test('compiled engine inputs match generated Rust declarations and restrictions', () => {
  const source = readFileSync(new URL('../index.d.ts', import.meta.url), 'utf8')
  const generated = inputContracts(source, rustIntegerFields(fileURLToPath(new URL('..', import.meta.url))))
  const stored = JSON.parse(readFileSync(new URL('../operation-inputs.json', import.meta.url), 'utf8'))
  assert.deepEqual(generated, stored)
  for (const operation of engine.getDocxCapabilities().formats[0].operations) {
    const definition = stored[operation.nodeExport]
    assert.equal(operation.description, definition.description)
    assert.deepEqual(JSON.parse(operation.inputSchema), definition.inputSchema)
    assert.equal(operation.jsonInput, definition.jsonInput)
    assert.equal(operation.jsonOutput, definition.jsonOutput)
  }
  const replace = generated.executeDocxReplaceText.inputSchema
  assert.deepEqual(replace.required, ['target', 'expectedCurrentText', 'replacement'])
  assert.equal(replace.properties.target.properties.occurrence.type, 'integer')
  assert.equal(replace.properties.target.properties.occurrence.minimum, 0)
  assert.equal(generated.executeDocxInsertPicture.jsonInput, false)
  assert.equal(generated.executeDocxReplacePicture.jsonInput, false)
  assert.equal(generated.executeDocxAddComment.inputSchema.properties.date['x-client-default'], 'currentUtc')
  assert.equal(generated.inspectDocxNotes.jsonOutput, true)
  assert.equal(generated.inspectDocx.inputSchema.properties.focus.properties.kind.enum.includes('table_rows'), true)
})

test('a new Rust input automatically gains a JSON schema; unsupported inputs stop generation', () => {
  const source = '/** Update a value. */\nexport declare function executeDocxNewOperation(bytes: Buffer, op: NewInput): Promise<unknown>\nexport interface NewInput { target: string; value?: boolean }'
  const definition = inputContracts(source).executeDocxNewOperation
  assert.deepEqual(definition.inputSchema.required, ['target'])
  assert.deepEqual(definition.inputSchema.properties, { target: { type: 'string' }, value: { type: 'boolean' } })
  assert.throws(() => inputContracts('export declare function bad(bytes: Buffer, op: Unknown): void'), /Unknown or recursive/)
})
