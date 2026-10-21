import assert from 'node:assert/strict'
import { test } from 'node:test'
import engine from '../index.js'
import { documentXml, editDocument, editParts, packageParts } from './picture-support.mjs'

const p = text => `<w:p><w:r><w:t>${text}</w:t></w:r></w:p>`
const fixture = body => editDocument(engine.createBlankDocx(), xml => xml.replace('<w:body>', `<w:body>${body}`))
const blocks = async bytes => (await engine.inspectDocx(bytes, {focus:{kind:'body_blocks'}})).bodyBlocks.items
const applied = result => { assert.equal(result.result.ok,true,JSON.stringify(result.result.diagnostics)); return result.output }
const refused = result => { assert.equal(result.result.ok,false); assert.equal(result.output,undefined) }
async function operation(bytes,from,to,position) {
  const items = await blocks(bytes)
  return {target:{handle:items[from].movementHandle},destination:{handle:items[to].movementHandle},position}
}
async function move(bytes,from,to,position) { return engine.executeDocxMoveParagraph(bytes,await operation(bytes,from,to,position)) }

test('movement verifies semantic order when an unrelated content control precedes the targets', async () => {
  const control = `<w:sdt><w:sdtPr><w:tag w:val="client"/><w:text/></w:sdtPr><w:sdtContent>${p('Client')}</w:sdtContent></w:sdt>`
  const input = fixture(control + p('First') + p('Second') + p('Third'))
  const page = await blocks(input)
  for (const position of ['before', 'after']) {
    const output = applied(await engine.executeDocxMoveParagraph(input, {
      target: { handle: page.find(b => b.text === 'Third').movementHandle },
      destination: { handle: page.find(b => b.text === 'First').movementHandle },
      position,
    }))
    assert.equal(documentXml(output), documentXml(input).replace(p('Third'), '').replace(
      p('First'), position === 'before' ? p('Third') + p('First') : p('First') + p('Third'),
    ))
    assert.ok(documentXml(output).includes(control))
    const before = packageParts(input), after = packageParts(output)
    for (const name of Object.keys(before).filter(name => name !== 'word/document.xml')) assert.deepEqual(after[name], before[name], name)
  }
})

test('paragraph moves preserve original styles, numbering, runs, empty paragraphs and unrelated parts', async () => {
  const styled = `<w:p w:rsidR='00112233'><w:pPr><w:pStyle w:val='Normal'/><w:numPr><w:ilvl w:val='0'/><w:numId w:val='1'/></w:numPr><w:keepNext/></w:pPr><w:r><w:rPr><w:b/><w:i/></w:rPr><w:t>Formatted</w:t><w:tab/><w:t>second</w:t></w:r></w:p>`
  const input = fixture(`${p('First')}\n${styled}\n${p('Third')}<w:p/>`)
  const output = applied(await move(input,1,2,'after'))
  assert.equal(documentXml(output),documentXml(input).replace(styled,'').replace(p('Third'),p('Third')+styled))
  const before = packageParts(input), after = packageParts(output)
  for (const name of Object.keys(before).filter(name => name !== 'word/document.xml')) assert.deepEqual(after[name],before[name],name)
  const back = applied(await move(output,2,0,'before'))
  assert.deepEqual((await blocks(back)).map(b => b.text),['Formattedsecond','First','Third',''])
  assert.match(documentXml(applied(await move(back,3,0,'before'))),/<w:body><w:p\/>/)
})

test('self/adjacent moves are deterministic no-ops; stale, wrong and malformed handles refuse', async () => {
  const input = fixture(p('First')+p('Second')+p('Third'))
  for (const [from,to,position] of [[0,0,'before'],[1,1,'after'],[0,1,'before'],[1,0,'after']]) assert.equal(documentXml(applied(await move(input,from,to,position))),documentXml(input))
  const stale = await operation(input,0,2,'after'), output = applied(await engine.executeDocxMoveParagraph(input,stale))
  refused(await engine.executeDocxMoveParagraph(output,stale))
  for (const handle of ['b0','bad','b999:bad']) refused(await engine.executeDocxMoveParagraph(input,{...stale,target:{handle}}))
  refused(await engine.executeDocxMoveParagraph(input,{...stale,position:'middle'}))
})

test('movement refuses section boundaries, cross-section moves and dependent structures at either endpoint', async () => {
  for (const content of [
    `<w:pPr><w:sectPr/></w:pPr><w:r><w:t>Unsafe</w:t></w:r>`,
    `<w:bookmarkStart w:id='1' w:name='Mark'/><w:r><w:t>Unsafe</w:t></w:r><w:bookmarkEnd w:id='1'/>`,
    `<w:commentRangeStart w:id='1'/><w:r><w:t>Unsafe</w:t></w:r><w:commentRangeEnd w:id='1'/>`,
    `<w:fldSimple w:instr='PAGE'><w:r><w:t>Unsafe</w:t></w:r></w:fldSimple>`,
    `<w:ins w:id='1'><w:r><w:t>Unsafe</w:t></w:r></w:ins>`,
    `<w:r><w:rPr><w:rPrChange/></w:rPr><w:t>Unsafe</w:t></w:r>`,
    `<w:r><w:t>Unsafe</w:t><w:footnoteReference w:id='1'/></w:r>`,
    `<w:hyperlink w:anchor='Mark'><w:r><w:t>Unsafe</w:t></w:r></w:hyperlink>`,
    `<w:r><w:drawing/></w:r>`,
    `<w:r><w:t>Unsafe</w:t></w:r><x:unknown xmlns:x='urn:keep'/>`,
  ]) {
    const input = fixture(`<w:p>${content}</w:p>${p('Safe')}`)
    for (const [from,to] of [[0,1],[1,0]]) refused(await move(input,from,to,'after'))
  }
  const cross = fixture(`${p('First section')}<w:p><w:pPr><w:sectPr/></w:pPr></w:p>${p('Second section')}`)
  refused(await move(cross,0,2,'after'))
  const enclosed = fixture(`<w:p><w:bookmarkStart w:id='1' w:name='Mark'/></w:p>${p('Inside')}<w:p><w:bookmarkEnd w:id='1'/></w:p>${p('Outside')}`)
  refused(await move(enclosed,1,3,'after'))
  const protectedDoc = editParts(fixture(p('One')+p('Two')), parts => {
    parts['word/settings.xml'] = Buffer.from('<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:documentProtection w:enforcement="1"/></w:settings>')
    parts['word/_rels/document.xml.rels'] = Buffer.from(parts['word/_rels/document.xml.rels'].toString().replace('</Relationships>','<Relationship Id="rIdProtect" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>'))
    return parts
  })
  refused(await move(protectedDoc,0,1,'after'))
})
