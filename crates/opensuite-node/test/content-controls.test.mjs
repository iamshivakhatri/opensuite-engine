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

test('inspection supplies exact selectors for duplicates and reports unsupported controls', async () => {
  const input = contentControlsDocx()
  const before = Buffer.from(input)
  const snapshot = JSON.parse(await binding.inspectDocxContentControls(input))
  assert.equal(snapshot.ok, true)
  assert.equal(snapshot.total, 3)
  assert.deepEqual(input, before)
  assert.equal(snapshot.controls[1].targetMatchCount, 2)
  assert.equal(snapshot.controls[1].textMutationSupported, true)
  assert.equal(snapshot.controls[1].target.occurrence, 1)
  assert.equal(snapshot.controls[1].paragraphIndex, 1)
  assert.equal(snapshot.controls[2].textMutationSupported, false)
  assert.ok(snapshot.controls[2].reasons.some(r => r.includes('unlocked')))
  const selected = snapshot.controls[1]
  const edited = await binding.executeDocxSetContentControlText(input, { target: selected.target, expectedCurrentText: selected.text, replacement: 'Selected by inspection' })
  assert.equal(edited.result.ok, true)
  const reopened = JSON.parse(await binding.inspectDocxContentControls(edited.output, { offset: 1, limit: 1 }))
  assert.equal(reopened.controls[0].text, 'Selected by inspection')
  assert.equal(reopened.hasMore, true)
})

test('inspection handles aliases, limits, unsupported inline controls and read-only failures', async () => {
  const { editDocument } = await import('./picture-support.mjs')
  const control = (properties, content) => `<w:sdt><w:sdtPr>${properties}</w:sdtPr><w:sdtContent>${content}</w:sdtContent></w:sdt>`
  const paragraph = text => `<w:p><w:r><w:t>${text}</w:t></w:r></w:p>`
  const controls = [
    control('<w:alias w:val="Client name"/><w:text/>', paragraph('Alias only')),
    control('<w:tag w:val="date"/><w:date/>', paragraph('2026-10-20')),
    control('<w:tag w:val="long"/><w:text/>', paragraph('界'.repeat(2001))),
    `<w:p>${control('<w:tag w:val="inline"/><w:text/>', '<w:r><w:t>Inline</w:t></w:r>')}</w:p>`,
  ].join('')
  const input = editDocument(binding.createBlankDocx(), xml => xml.replace('<w:body>', `<w:body>${controls}`))
  const snapshot = JSON.parse(await binding.inspectDocxContentControls(input, { limit: 100 }))
  assert.equal(snapshot.total, 4)
  assert.equal(snapshot.controls[0].target.alias, 'Client name')
  assert.equal(snapshot.controls[0].textMutationSupported, true)
  assert.equal(snapshot.controls[1].kind, 'date')
  assert.equal(snapshot.controls[1].textMutationSupported, false)
  assert.equal([...snapshot.controls[2].text].length, 2000)
  assert.equal(snapshot.controls[2].truncated, true)
  assert.equal(snapshot.controls[2].textMutationSupported, false)
  assert.equal(snapshot.controls[3].container, 'inline')
  assert.equal(snapshot.controls[3].textMutationSupported, false)
  assert.equal(JSON.parse(await binding.inspectDocxContentControls(Buffer.from('bad'))).ok, false)
})
