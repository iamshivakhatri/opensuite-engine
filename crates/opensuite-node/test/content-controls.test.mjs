import assert from 'node:assert/strict'
import { test } from 'node:test'
import binding from '../index.js'

const { executeDocxSetContentControlText, findDocxText } = binding
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

function contentControlsDocx() {
  const files = [
    ['[Content_Types].xml', '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>'],
    ['_rels/.rels', `<Relationships><Relationship Id="rId1" Type="${office}" Target="word/document.xml"/></Relationships>`],
    ['word/document.xml', `<w:document xmlns:w="${word}"><w:body><w:sdt><w:sdtPr><w:tag w:val="customer"/><w:text/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>First</w:t></w:r></w:p></w:sdtContent></w:sdt><w:sdt><w:sdtPr><w:tag w:val="customer"/><w:text/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Second</w:t></w:r></w:p></w:sdtContent></w:sdt><w:sdt><w:sdtPr><w:tag w:val="locked"/><w:text/><w:lock w:val="sdtContentLocked"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Locked</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>`],
  ]
  let offset = 0
  const local = []
  const central = []
  for (const [name, text] of files) {
    const filename = Buffer.from(name)
    const content = Buffer.from(text)
    const header = Buffer.alloc(30)
    header.writeUInt32LE(0x04034b50, 0)
    header.writeUInt16LE(20, 4)
    header.writeUInt32LE(crc32(content), 14)
    header.writeUInt32LE(content.length, 18)
    header.writeUInt32LE(content.length, 22)
    header.writeUInt16LE(filename.length, 26)
    const directory = Buffer.alloc(46)
    directory.writeUInt32LE(0x02014b50, 0)
    directory.writeUInt16LE(20, 4)
    directory.writeUInt16LE(20, 6)
    directory.writeUInt32LE(crc32(content), 16)
    directory.writeUInt32LE(content.length, 20)
    directory.writeUInt32LE(content.length, 24)
    directory.writeUInt16LE(filename.length, 28)
    directory.writeUInt32LE(offset, 42)
    local.push(header, filename, content)
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

test('updates one duplicate content control by occurrence', async () => {
  const input = contentControlsDocx()
  const ambiguous = await executeDocxSetContentControlText(input, {
    target: { tag: 'customer' }, expectedCurrentText: 'Second', replacement: 'Updated',
  })
  assert.equal(ambiguous.result.diagnostics[0].code, 'TARGET_AMBIGUOUS')

  const result = await executeDocxSetContentControlText(input, {
    target: { tag: 'customer', occurrence: 1 }, expectedCurrentText: 'Second', replacement: 'Updated',
  })
  assert.equal(result.result.ok, true)
  assert.equal((await findDocxText(result.output, { text: 'Updated' })).matchCount, 1)

  const locked = await executeDocxSetContentControlText(input, {
    target: { tag: 'locked' }, expectedCurrentText: 'Locked', replacement: 'Nope',
  })
  assert.equal(locked.result.diagnostics[0].code, 'UNSUPPORTED_OPERATION')
})
