import assert from 'node:assert/strict'
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import binding from '../index.js'

const { executeDocxReplaceText, findDocxText, getDocxCapabilities, inspectDocx } = binding
const office = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument'
const word = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'

function crc32(bytes) {
  let value = 0xffffffff
  for (const byte of bytes) {
    value ^= byte
    for (let bit = 0; bit < 8; bit += 1) value = (value >>> 1) ^ (value & 1 ? 0xedb88320 : 0)
  }
  return (value ^ 0xffffffff) >>> 0
}

function docxFixture() {
  const files = [
    ['[Content_Types].xml', '<Types><Default Extension="xml" ContentType="application/xml"/></Types>'],
    ['_rels/.rels', `<Relationships><Relationship Id="rId1" Type="${office}" Target="word/document.xml"/></Relationships>`],
    ['word/document.xml', `<w:document xmlns:w="${word}"><w:body><w:p><w:r><w:t>old text</w:t></w:r></w:p><w:p><w:r><w:t>Date:</w:t></w:r></w:p><w:p><w:r><w:t>Date:</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>table needle</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>`],
  ]
  let offset = 0
  const local = []
  const central = []
  for (const [name, text] of files) {
    const filename = Buffer.from(name)
    const content = Buffer.from(text)
    const checksum = crc32(content)
    const header = Buffer.alloc(30)
    header.writeUInt32LE(0x04034b50, 0)
    header.writeUInt16LE(20, 4)
    header.writeUInt32LE(checksum, 14)
    header.writeUInt32LE(content.length, 18)
    header.writeUInt32LE(content.length, 22)
    header.writeUInt16LE(filename.length, 26)
    local.push(header, filename, content)
    const directory = Buffer.alloc(46)
    directory.writeUInt32LE(0x02014b50, 0)
    directory.writeUInt16LE(20, 4)
    directory.writeUInt16LE(20, 6)
    directory.writeUInt32LE(checksum, 16)
    directory.writeUInt32LE(content.length, 20)
    directory.writeUInt32LE(content.length, 24)
    directory.writeUInt16LE(filename.length, 28)
    directory.writeUInt32LE(offset, 42)
    central.push(directory, filename)
    offset += header.length + filename.length + content.length
  }
  const directory = Buffer.concat(central)
  const end = Buffer.alloc(22)
  end.writeUInt32LE(0x06054b50, 0)
  end.writeUInt16LE(files.length, 8)
  end.writeUInt16LE(files.length, 10)
  end.writeUInt32LE(directory.length, 12)
  end.writeUInt32LE(offset, 16)
  return Buffer.concat([...local, directory, end])
}

const inputPath = join(mkdtempSync(join(tmpdir(), 'opensuite-node-')), 'input.docx')
writeFileSync(inputPath, docxFixture())
const input = readFileSync(inputPath)
const operation = (text, replacement = 'OpenSuite replacement') => ({
  target: { text },
  expectedCurrentText: text,
  replacement,
})

test('returns verified Buffer output for a valid replacement', async () => {
  const result = await executeDocxReplaceText(input, operation('old text'))

  assert.equal(result.result.ok, true)
  assert.equal(result.result.status, 'applied')
  assert.ok(Buffer.isBuffer(result.output))
  assert.notDeepEqual(result.output, input)
  const second = await executeDocxReplaceText(result.output, operation('OpenSuite replacement', 'Second replacement'))
  assert.equal(second.result.ok, true)
  assert.ok(Buffer.isBuffer(second.output))
})

test('returns structured failures without output buffers', async () => {
  for (const value of [
    await executeDocxReplaceText(input, operation('not present')),
    await executeDocxReplaceText(input, operation('Date:')),
    await executeDocxReplaceText(Buffer.from('not a DOCX'), operation('anything')),
  ]) {
    assert.equal(value.result.ok, false)
    assert.equal(value.result.status, 'failed')
    assert.ok(value.result.diagnostics.length)
    assert.equal(value.output, undefined)
  }
})

test('reads and writes the same DOCX Buffer through the Rust engine', async () => {
  const capabilities = getDocxCapabilities()
  assert.equal(capabilities.ok, true)
  assert.equal(capabilities.formats[0].format, 'docx')
  assert.ok(capabilities.formats[0].capabilities.includes('find_text'))
  assert.ok(capabilities.formats[0].capabilities.includes('inspect_context'))
  assert.ok(capabilities.formats[0].capabilities.includes('replace_text'))

  const found = await findDocxText(input, { text: 'Date:' })
  assert.equal(found.ok, true)
  assert.equal(found.matchCount, 2)
  assert.deepEqual(found.matches.map((match) => match.occurrence), [0, 1])
  assert.deepEqual(Object.keys(found.matches[0]).sort(), ['after', 'before', 'container', 'occurrence', 'text'])

  const context = await inspectDocx(input, { target: { text: 'table needle' }, before: 1, after: 0 })
  assert.equal(context.ok, true)
  assert.equal(context.container.text, 'table needle')
  assert.equal(context.container.container, 'table_cell')
  assert.equal(context.nearby.length, 2)
  assert.deepEqual(Object.keys(context.container).sort(), ['container', 'relativePosition', 'text'])

  const changed = await executeDocxReplaceText(input, operation('old text', 'new text'))
  assert.equal(changed.result.ok, true)
  const foundOutput = await findDocxText(changed.output, { text: 'new text' })
  const contextOutput = await inspectDocx(changed.output, { target: { text: 'new text' } })
  assert.equal(foundOutput.matchCount, 1)
  assert.equal(contextOutput.container.text, 'new text')

  for (const result of [
    await findDocxText(Buffer.from('not a DOCX'), { text: 'anything' }),
    await inspectDocx(Buffer.from('not a DOCX'), { target: { text: 'anything' } }),
  ]) {
    assert.equal(result.ok, false)
    assert.equal(result.diagnostics[0].code, 'INVALID_ZIP')
    assert.equal(JSON.stringify(result).includes('NodeId'), false)
    assert.equal(JSON.stringify(result).includes('SourceSpan'), false)
  }
})
