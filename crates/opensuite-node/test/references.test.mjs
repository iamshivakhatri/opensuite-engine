import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import engine from '../index.js'
import { documentXml, editDocument, packageParts } from './picture-support.mjs'

const inspect = async (bytes, options) => JSON.parse(await engine.inspectDocxReferences(bytes, options))
const applied = result => { assert.equal(result.result.ok, true, JSON.stringify(result.result.diagnostics)); return result.output }
const refused = result => { assert.equal(result.result.ok, false); assert.equal(result.output, undefined) }
const paragraph = text => `<w:p><w:r><w:t>${text}</w:t></w:r></w:p>`
const marks = `<w:p><w:bookmarkStart w:id='7' w:name='Destination'/><w:r><w:t>Destination heading</w:t></w:r><w:bookmarkEnd w:id='7'/></w:p>`
const fixture = body => editDocument(engine.createBlankDocx(), xml => xml.replace('<w:body>', `<w:body>${body}`))

test('reference inspection exposes multiple imported bookmarks, hyperlink targets and bounded paging', async () => {
  const bytes = readFileSync(new URL('../../../tests/fixtures/references.docx', import.meta.url))
  const before = packageParts(bytes)
  const result = await inspect(bytes)
  assert.equal(result.ok, true)
  assert.equal(result.total, 4)
  const links = result.references.filter(r => r.kind === 'hyperlink')
  assert.deepEqual(links.map(r => r.targetKind), ['external', 'bookmark'])
  assert.equal(links[1].target, 'RevenueSection')
  assert.equal(links[0].textTarget.text, 'OpenSuite')
  assert.equal(result.references.find(r => r.name === 'TableCell').container, 'tableCell')
  const page = await inspect(bytes, { offset: 1, limit: 1 })
  assert.equal(page.references.length, 1)
  assert.equal(page.references[0].index, 1)
  assert.equal(page.hasMore, true)
  assert.deepEqual(packageParts(bytes), before)
  const large = await inspect(fixture(paragraph('unused') + Array.from({length:120}, (_,i) => `<w:p><w:hyperlink w:anchor='Missing${i}'><w:r><w:t>${'x'.repeat(2100)}</w:t></w:r></w:hyperlink></w:p>`).join('')), { limit: 1000 })
  assert.equal(large.references.length, 100)
  assert.equal(large.total, 120)
  assert.equal(large.references[0].text.length, 2000)
  assert.equal(large.references[0].textTarget, null)
  assert.equal(large.references[0].truncated, true)
})

test('internal hyperlinks preserve selected formatting, references, relationships and unrelated package parts', async () => {
  const input = fixture(`<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Before link</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t> text after</w:t></w:r></w:p>${marks}`)
  const bookmark = (await inspect(input)).references.find(r => r.kind === 'bookmark')
  const output = applied(await engine.executeDocxSetHyperlink(input, {target:{text:'link text'}, bookmarkName:bookmark.bookmarkName}))
  assert.match(documentXml(output), /<w:hyperlink w:anchor="Destination"><w:r><w:rPr><w:b\/>/)
  assert.match(documentXml(output), /<w:rPr><w:i\/><\/w:rPr>/)
  assert.match(documentXml(output), /<w:bookmarkStart w:id='7' w:name='Destination'\/>/)
  const before = packageParts(input), after = packageParts(output)
  assert.deepEqual(Object.keys(after), Object.keys(before))
  for (const name of Object.keys(before).filter(name => name !== 'word/document.xml')) assert.deepEqual(after[name], before[name], name)
  const link = (await inspect(output)).references.find(r => r.kind === 'hyperlink')
  assert.equal(link.targetKind,'bookmark')
  assert.equal(link.target,'Destination')
  assert.equal(link.text,'link text')
  const external = applied(await engine.executeDocxSetHyperlink(output,{target:link.textTarget,url:'https://example.com/target'}))
  assert.equal((await inspect(external)).references.find(r => r.kind === 'hyperlink').targetKind,'external')
  const internal = applied(await engine.executeDocxSetHyperlink(external,{target:{text:'link text'},bookmarkName:'Destination'}))
  const cleared = applied(await engine.executeDocxSetHyperlink(internal,{target:{text:'link text'}}))
  assert.equal((await inspect(cleared)).references.filter(r => r.kind === 'hyperlink').length,0)
  assert.match(documentXml(cleared), /<w:b\/>/)
})

test('missing, duplicate, malformed and protected bookmark/link inputs refuse without output', async () => {
  const input = fixture(paragraph('Link') + marks)
  refused(await engine.executeDocxSetHyperlink(input,{target:{text:'Link'},bookmarkName:'Missing'}))
  refused(await engine.executeDocxSetHyperlink(input,{target:{text:'Link'},bookmarkName:'Destination',url:'https://example.com'}))
  const duplicate = editDocument(input, xml => xml.replace('</w:body>', `${marks.replaceAll("'7'", "'8'")}</w:body>`))
  assert.equal((await inspect(duplicate)).references[0].bookmarkName, null)
  refused(await engine.executeDocxSetHyperlink(duplicate,{target:{text:'Link'},bookmarkName:'Destination'}))
  for (const middle of ["<w:bookmarkStart w:id='8' w:name='Other'/><w:bookmarkEnd w:id='8'/>","<w:commentRangeStart w:id='1'/><w:commentRangeEnd w:id='1'/>","<w:ins w:id='1'/>","<w:r><w:fldChar w:fldCharType='begin'/><w:instrText>PAGE</w:instrText><w:fldChar w:fldCharType='end'/></w:r>"]) {
    const protectedBytes = fixture(`<w:p><w:r><w:t>One</w:t></w:r>${middle}<w:r><w:t>Two</w:t></w:r></w:p>${marks}`)
    refused(await engine.executeDocxSetHyperlink(protectedBytes,{target:{text:'OneTwo'},bookmarkName:'Destination'}))
  }
  const broken = fixture(paragraph('Link') + "<w:p><w:bookmarkStart w:id='1' w:name='Broken'/></w:p>")
  assert.equal((await inspect(broken)).diagnostics[0].code,'UNMATCHED_BOOKMARK_START')
  refused(await engine.executeDocxSetHyperlink(broken,{target:{text:'Link'},bookmarkName:'Broken'}))
})

test('reference reads and paragraph movement are discoverable with generated JSON inputs', () => {
  const operations = engine.getDocxCapabilities().formats[0].operations
  const read = operations.find(op => op.id === 'inspect_references')
  assert.equal(read.nodeExport,'inspectDocxReferences')
  assert.equal(read.jsonOutput,true)
  assert.equal(JSON.parse(read.inputSchema).properties.limit.maximum,100)
  const move = operations.find(op => op.id === 'move_paragraph')
  assert.equal(move.jsonInput,true)
  assert.deepEqual(JSON.parse(move.inputSchema).properties.position.enum,['before','after'])
  assert.equal(JSON.parse(operations.find(op => op.id === 'set_hyperlink').inputSchema).properties.bookmarkName.type,'string')
})

test('LibreOffice-imported bookmarks and links survive link authoring and formatted numbered paragraph movement', async () => {
  const input = readFileSync(new URL('../../../tests/fixtures/libreoffice-references.docx', import.meta.url))
  const imported = await inspect(input)
  assert.equal(imported.total,3)
  assert.deepEqual(imported.references.map(r => r.kind),['hyperlink','hyperlink','bookmark'])
  assert.equal(imported.references[2].bookmarkName,'Findings')
  let bytes = applied(await engine.executeDocxSetHyperlink(input,{target:{text:'details'},bookmarkName:'Findings'}))
  const page = (await engine.inspectDocx(bytes,{focus:{kind:'body_blocks'}})).bodyBlocks.items
  const target = page.find(p => p.text === 'Move this formatted numbered item.')
  const destination = page.find(p => p.text === 'First numbered item.')
  const originalParagraph = documentXml(bytes).match(/<w:p(?:\s[^>]*)?>.*?<\/w:p>/gs).find(p => p.includes('Move this'))
  bytes = applied(await engine.executeDocxMoveParagraph(bytes,{target:{handle:target.movementHandle},destination:{handle:destination.movementHandle},position:'before'}))
  assert.ok(documentXml(bytes).includes(originalParagraph))
  const result = await inspect(bytes)
  assert.equal(result.total,4)
  assert.equal(result.references.filter(r => r.targetKind === 'bookmark').length,2)
  const before = packageParts(input), after = packageParts(bytes)
  for (const name of Object.keys(before).filter(name => name !== 'word/document.xml')) assert.deepEqual(after[name],before[name],name)
})
