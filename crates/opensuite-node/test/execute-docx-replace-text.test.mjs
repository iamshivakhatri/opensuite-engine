import assert from 'node:assert/strict'
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import binding from '../index.js'

const { createBlankDocx, executeDocxCreateTable, executeDocxDeleteParagraph, executeDocxDeleteTable, executeDocxDeleteTableColumn, executeDocxDeleteTableRow, executeDocxInsertParagraph, executeDocxInsertParagraphs, executeDocxInsertTableRow, executeDocxInsertTableRows, executeDocxReplaceText, executeDocxSetParagraphFormatting, executeDocxSetParagraphStyle, executeDocxSetTableCellsText, executeDocxSetTextFormatting, findDocxText, getDocxCapabilities, inspectDocx } = binding
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
    ['[Content_Types].xml', '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>'],
    ['_rels/.rels', `<Relationships><Relationship Id="rId1" Type="${office}" Target="word/document.xml"/></Relationships>`],
    ['word/_rels/document.xml.rels', '<Relationships><Relationship Id="styles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>'],
    ['word/styles.xml', `<w:styles xmlns:w="${word}"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="Heading 1"/></w:style><w:style w:type="paragraph" w:styleId="Body"><w:name w:val="Body Text"/></w:style></w:styles>`],
    ['word/document.xml', `<w:document xmlns:w="${word}"><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Report heading</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Body"/></w:pPr><w:r><w:t>old text</w:t></w:r></w:p><w:p><w:r><w:t>Date:</w:t></w:r></w:p><w:p><w:r><w:t>Date:</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>table needle</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>second cell</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>uneven row</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Role</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Alice</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CEO</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>Bob</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>CTO</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p/></w:tc><w:tc><w:p/></w:tc></w:tr></w:tbl></w:body></w:document>`],
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

test('creates and authors a blank DOCX entirely as Buffers', async () => {
  const blank = createBlankDocx()
  assert.ok(Buffer.isBuffer(blank))
  const heading = await executeDocxInsertParagraph(blank, { text: 'Hello', placement: { kind: 'end' } })
  assert.equal(heading.result.ok, true)
  const second = await executeDocxInsertParagraph(heading.output, { text: 'World', placement: { kind: 'after', handle: 'b0' } })
  assert.equal(second.result.ok, true)
  const body = await inspectDocx(second.output, { focus: { kind: 'body_blocks', offset: 0, limit: 20 } })
  assert.deepEqual(body.bodyBlocks.items.map((block) => block.text), ['Hello', 'World'])
})

test('creates, edits, and deletes a table through native bindings', async () => {
  let result = await executeDocxCreateTable(createBlankDocx(), {
    rows: [['Task', 'Owner'], ['Prepare report', 'J. Smith']], placement: { kind: 'end' },
  })
  assert.equal(result.result.ok, true)
  let tables = await inspectDocx(result.output, { focus: { kind: 'tables', offset: 0, limit: 10 } })
  let table = tables.tables.items[0]
  result = await executeDocxDeleteTableColumn(result.output, { table: { handle: table.handle }, columnHandle: table.columns[1].handle })
  assert.equal(result.result.ok, true)
  tables = await inspectDocx(result.output, { focus: { kind: 'tables', offset: 0, limit: 10 } })
  table = tables.tables.items[0]
  result = await executeDocxDeleteTableRow(result.output, { table: { handle: table.handle }, row: { handle: table.rows[1].handle } })
  assert.equal(result.result.ok, true)
  result = await executeDocxDeleteTable(result.output, { table: { handle: 't0' } })
  assert.equal(result.result.ok, true)
  tables = await inspectDocx(result.output, { focus: { kind: 'tables', offset: 0, limit: 10 } })
  assert.equal(tables.tables.items.length, 0)
})

test('authors and edits a complete paragraph lifecycle through native bindings', async () => {
  let result = await executeDocxInsertParagraphs(createBlankDocx(), {
    texts: ['Title', 'Introduction', 'Body', 'Conclusion'], placement: { kind: 'end' },
  })
  assert.equal(result.result.ok, true)
  assert.ok(Buffer.isBuffer(result.output))
  result = await executeDocxSetParagraphStyle(result.output, { target: { text: 'Title' }, style: 'Heading 1' })
  assert.equal(result.result.ok, true)
  result = await executeDocxSetParagraphFormatting(result.output, { target: { text: 'Body' }, alignment: 'center', spacingAfterTwips: 120 })
  assert.equal(result.result.ok, true)
  result = await executeDocxSetTextFormatting(result.output, { target: { text: 'Body' }, bold: true })
  assert.equal(result.result.ok, true)
  result = await executeDocxDeleteParagraph(result.output, { target: { text: 'Conclusion' } })
  assert.equal(result.result.ok, true)
  const paragraphs = await inspectDocx(result.output, { focus: { kind: 'paragraphs', offset: 0, limit: 20 } })
  assert.deepEqual(paragraphs.paragraphs.items.map((item) => item.text), ['Title', 'Introduction', 'Body'])
  const headings = await inspectDocx(result.output, { focus: { kind: 'headings', offset: 0, limit: 20 } })
  assert.deepEqual(headings.headings.items.map((item) => [item.text, item.styleName]), [['Title', 'Heading 1']])
  assert.equal((await findDocxText(result.output, { text: 'Conclusion' })).matchCount, 0)
})

test('places paragraph batches around body blocks and preserves text exactly', async () => {
  let bytes = await executeDocxInsertParagraphs(createBlankDocx(), { texts: ['end', '', '  ünicode  '], placement: { kind: 'end' } }).then((value) => value.output)
  bytes = (await executeDocxInsertParagraphs(bytes, { texts: ['start'], placement: { kind: 'start' } })).output
  bytes = (await executeDocxInsertParagraphs(bytes, { texts: ['before'], placement: { kind: 'before', handle: 'b1' } })).output
  bytes = (await executeDocxInsertParagraphs(bytes, { texts: ['after'], placement: { kind: 'after', handle: 'b2' } })).output
  const body = await inspectDocx(bytes, { focus: { kind: 'body_blocks', offset: 0, limit: 20 } })
  assert.deepEqual(body.bodyBlocks.items.map((item) => item.text), ['start', 'before', 'end', 'after', '', '  ünicode  '])
  const empty = await executeDocxInsertParagraphs(bytes, { texts: [], placement: { kind: 'end' } })
  assert.equal(empty.result.ok, false)
  const stale = await executeDocxInsertParagraphs(bytes, { texts: ['nope'], placement: { kind: 'before', handle: 'b99' } })
  assert.equal(stale.result.diagnostics[0].code, 'TARGET_NOT_FOUND')
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
  assert.ok(capabilities.formats[0].capabilities.includes('insert_table_row'))
  assert.ok(capabilities.formats[0].capabilities.includes('insert_table_rows'))
  assert.ok(capabilities.formats[0].capabilities.includes('set_table_cells_text'))

  const found = await findDocxText(input, { text: 'Date:' })
  assert.equal(found.ok, true)
  assert.equal(found.matchCount, 2)
  assert.deepEqual(found.matches.map((match) => match.occurrence), [0, 1])
  assert.deepEqual(Object.keys(found.matches[0]).sort(), ['after', 'before', 'container', 'occurrence', 'text'])

  const overview = await inspectDocx(input, { focus: { kind: 'overview' } })
  assert.equal(overview.ok, true)
  assert.equal(overview.overview.paragraphCount, 4)
  assert.equal(overview.overview.tableCount, 2)

  const headings = await inspectDocx(input, { focus: { kind: 'headings', offset: 0, limit: 1 } })
  assert.equal(headings.headings.page.total, 1)
  assert.equal(headings.headings.items[0].text, 'Report heading')
  assert.equal(headings.headings.items[0].styleName, 'Heading 1')
  assert.equal(headings.headings.items[0].level, 1)

  const paragraphs = await inspectDocx(input, { focus: { kind: 'paragraphs', offset: 1, limit: 2 } })
  assert.equal(paragraphs.paragraphs.page.total, 4)
  assert.equal(paragraphs.paragraphs.items[0].text, 'old text')
  assert.equal(paragraphs.paragraphs.items[0].styleName, 'Body Text')

  const tables = await inspectDocx(input, { focus: { kind: 'tables', offset: 0, limit: 1 } })
  assert.equal(tables.tables.items[0].rows[0].cells[0], 'table needle')
  assert.equal(tables.tables.items[0].isRectangular, false)

  const namedTable = await inspectDocx(input, { focus: { kind: 'tables', offset: 1, limit: 1 } })
  const blankRow = namedTable.tables.items[0].rows.at(-1)
  assert.equal(blankRow.cells[0], '')
  const handleUpdated = await executeDocxSetTableCellsText(input, {
    table: { handle: namedTable.tables.items[0].handle },
    updates: [
      { target: { handle: blankRow.cellHandles[0] }, expectedCurrentText: '', replacement: 'Guest Panelist' },
      { target: { handle: blankRow.cellHandles[1] }, expectedCurrentText: '', replacement: 'Invited meetings' },
    ],
  })
  assert.equal(handleUpdated.result.ok, true)
  const afterHandleUpdate = await inspectDocx(handleUpdated.output, { focus: { kind: 'tables', offset: 1, limit: 1 } })
  assert.deepEqual(afterHandleUpdate.tables.items[0].rows.at(-1).cells, ['Guest Panelist', 'Invited meetings'])

  const updated = await executeDocxSetTableCellsText(input, {
    table: { headerCells: ['Name', 'Role'] },
    updates: [
      { target: { rowLabel: 'Alice', columnHeader: 'Role' }, expectedCurrentText: 'CEO', replacement: 'Founder & CEO' },
      { target: { rowLabel: 'Bob', columnHeader: 'Role' }, expectedCurrentText: 'CTO', replacement: 'CTO & VP Engineering' },
    ],
  })
  assert.equal(updated.result.ok, true)
  const tablesAfterUpdate = await inspectDocx(updated.output, { focus: { kind: 'tables', offset: 1, limit: 1 } })
  assert.equal(tablesAfterUpdate.tables.items[0].rows[1].cells[1], 'Founder & CEO')

  const inserted = await executeDocxInsertTableRows(updated.output, {
    table: { headerCells: ['Name', 'Role'] },
    after: { firstCellText: 'Bob' },
    rows: [['Charlie', 'CFO'], ['David', 'COO']],
  })
  assert.equal(inserted.result.ok, true)
  writeFileSync('/private/tmp/opensuite-insert-table-row-output.docx', inserted.output)
  const tablesAfterInsert = await inspectDocx(inserted.output, { focus: { kind: 'tables', offset: 1, limit: 1 } })
  assert.deepEqual(tablesAfterInsert.tables.items[0].rows[3].cells, ['Charlie', 'CFO'])
  assert.deepEqual(tablesAfterInsert.tables.items[0].rows[4].cells, ['David', 'COO'])

  const googleDocsTable = readFileSync(join(process.cwd(), '../../tests/fixtures/google-docs-table.docx'))
  const googleDocsInspection = await inspectDocx(googleDocsTable, { focus: { kind: 'tables', offset: 0, limit: 1 } })
  const googleDocsRows = googleDocsInspection.tables.items[0].rows
  assert.equal(googleDocsInspection.tables.items[0].affordances.every((item) => item.supported), true)
  assert.equal(googleDocsRows[0].cellAffordances[0].every((item) => item.supported), true)
  assert.deepEqual(googleDocsRows[0].cellAffordances[1].map((item) => item.reason), ['MULTIPLE_PARAGRAPHS', 'MULTIPLE_PARAGRAPHS'])
  assert.equal(googleDocsRows[1].cellAffordances[0].every((item) => item.supported), true)
  assert.deepEqual(googleDocsRows[1].cellAffordances[1].map((item) => item.reason), ['MULTIPLE_PARAGRAPHS', 'MULTIPLE_PARAGRAPHS'])
  const googleDocsFailure = await executeDocxSetTableCellsText(googleDocsTable, {
    table: { handle: 't0' },
    updates: [{ target: { handle: 't0:r1:c1' }, expectedCurrentText: '2026', replacement: '2027' }],
  })
  assert.equal(googleDocsFailure.result.diagnostics[0].code, 'UNSUPPORTED_OPERATION')
  assert.equal(googleDocsFailure.result.diagnostics[0].reasonCode, 'MULTIPLE_PARAGRAPHS')
  assert.equal(googleDocsFailure.result.diagnostics[0].operation, 'set_table_cells_text')
  assert.equal(googleDocsFailure.result.diagnostics[0].targetHandle, 't0:r1:c1')

  const context = await inspectDocx(input, { focus: { kind: 'context', text: 'table needle', before: 1, after: 0 } })
  assert.equal(context.ok, true)
  assert.equal(context.context.container.text, 'table needle')
  assert.equal(context.context.container.container, 'table_cell')
  assert.equal(context.context.nearby.length, 2)
  assert.deepEqual(Object.keys(context.context.container).sort(), ['container', 'relativePosition', 'text'])

  const changed = await executeDocxReplaceText(input, operation('old text', 'new text'))
  assert.equal(changed.result.ok, true)
  const foundOutput = await findDocxText(changed.output, { text: 'new text' })
  const contextOutput = await inspectDocx(changed.output, { focus: { kind: 'context', text: 'new text' } })
  assert.equal(foundOutput.matchCount, 1)
  assert.equal(contextOutput.context.container.text, 'new text')

  for (const result of [
    await findDocxText(Buffer.from('not a DOCX'), { text: 'anything' }),
    await inspectDocx(Buffer.from('not a DOCX'), { focus: { kind: 'overview' } }),
  ]) {
    assert.equal(result.ok, false)
    assert.equal(result.diagnostics[0].code, 'INVALID_ZIP')
    assert.equal(JSON.stringify(result).includes('NodeId'), false)
    assert.equal(JSON.stringify(result).includes('SourceSpan'), false)
  }

  const invalidBounds = await inspectDocx(input, { focus: { kind: 'tables', offset: 0, limit: 101 } })
  assert.equal(invalidBounds.ok, false)
  assert.equal(invalidBounds.diagnostics[0].code, 'INVALID_INSPECTION_BOUNDS')
})
