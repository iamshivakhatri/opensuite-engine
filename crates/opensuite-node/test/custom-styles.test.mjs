import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFileSync, writeFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import binding from '../index.js'

async function apply(bytes, method, operation) {
  const response = await binding[method](bytes, operation)
  assert.equal(response.result.ok, true, JSON.stringify(response.result.diagnostics))
  assert.ok(response.output)
  return response.output
}

test('custom Word styles dogfood persists, applies, updates and resolves inheritance', async () => {
  let bytes = binding.createBlankDocx()
  bytes = await apply(bytes, 'executeDocxInsertParagraphs', { texts: ['OpenSuite custom Word styles', 'Overview', 'Ordinary body text.', 'Results', 'Reusable callout text.'], placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxSetParagraphStyle', { target: { text: 'OpenSuite custom Word styles' }, style: 'Title' })
  bytes = await apply(bytes, 'executeDocxCreateStyle', { styleId: 'OpenSuiteReportHeading', styleType: 'paragraph', name: 'OpenSuiteReportHeading', basedOn: 'Heading1', next: 'Normal', fontFamily: 'Arial', fontSizeHalfPoints: 32, bold: true, color: '124733', spacingBeforeTwips: 240, spacingAfterTwips: 120, keepWithNext: true })
  let snapshot = JSON.parse(await binding.inspectDocxStyleSnapshot(bytes))
  assert.equal(snapshot.styles.find(s => s.styleId === 'OpenSuiteReportHeading').paragraphUsageCount, 0)
  for (const text of ['Overview', 'Results']) bytes = await apply(bytes, 'executeDocxSetParagraphStyle', { target: { text }, style: 'OpenSuiteReportHeading' })
  bytes = await apply(bytes, 'executeDocxCreateStyle', { styleId: 'OpenSuiteCallout', styleType: 'paragraph', name: 'OpenSuite Callout', basedOn: 'Normal', italic: true, leftIndentTwips: 240 })
  bytes = await apply(bytes, 'executeDocxSetParagraphStyle', { target: { text: 'Reusable callout text.' }, style: 'OpenSuite Callout' })
  bytes = await apply(bytes, 'executeDocxUpdateStyle', { styleId: 'OpenSuiteReportHeading', styleType: 'paragraph', color: '235744', spacingAfterTwips: 160, lineSpacing: { value: 276, rule: 'auto' }, clear: ['bold'] })
  bytes = await apply(bytes, 'executeDocxCreateStyle', { styleId: 'OpenSuiteAccent', styleType: 'character', name: 'OpenSuite Accent', color: '124733' })
  bytes = await apply(bytes, 'executeDocxUpdateStyle', { styleId: 'OpenSuiteAccent', styleType: 'character', italic: true })
  const artifact = '/private/tmp/opensuite-e1-custom-styles.docx'
  writeFileSync(artifact, bytes)
  snapshot = JSON.parse(await binding.inspectDocxStyleSnapshot(readFileSync(artifact)))
  const heading = snapshot.styles.find(s => s.styleId === 'OpenSuiteReportHeading')
  assert.equal(heading.basedOnStyleId, 'Heading1')
  assert.equal(heading.nextStyleId, 'Normal')
  assert.equal(heading.paragraphUsageCount, 2)
  assert.equal(heading.declaredRunFormatting.bold, undefined)
  assert.equal(heading.effectiveRunFormatting.bold, true)
  assert.equal(heading.effectiveRunFormatting.fontFamily, 'Arial')
  assert.equal(heading.effectiveRunFormatting.color, '235744')
  assert.equal(heading.effectiveParagraphFormatting.spacingAfterTwips, 160)
  assert.deepEqual(heading.effectiveParagraphFormatting.lineSpacing, { value: 276, rule: 'auto' })
  writeFileSync('/private/tmp/opensuite-e1-custom-styles.json', JSON.stringify(snapshot, null, 2))
  const duplicate = await binding.executeDocxCreateStyle(bytes, { styleId: heading.styleId, styleType: 'paragraph', name: 'Different' })
  assert.equal(duplicate.result.ok, false)
  assert.equal(duplicate.output, undefined)
  assert.throws(() => binding.executeDocxUpdateStyle(bytes, { styleId: heading.styleId, styleType: 'invalid' }))
  assert.throws(() => binding.executeDocxUpdateStyle(bytes, { styleId: heading.styleId, styleType: 'paragraph', clear: ['unsupported'] }))
})

test('imported styles and every untouched package payload remain byte-identical', async () => {
  for (const filename of ['google-docs-table.docx', 'imported-style-report.docx']) {
  const fixture = new URL('../../../tests/fixtures/' + filename, import.meta.url)
  const original = readFileSync(fixture)
  const inspected = await binding.inspectDocx(original, { focus: { kind: 'paragraphs' } })
  const paragraph = inspected.paragraphs.items.find(p => p.text.trim())
  const text = paragraph?.text ?? 'Imported document style check'
  let bytes = await apply(original, 'executeDocxCreateStyle', { styleId: 'ImportedCustom', styleType: 'paragraph', name: 'Imported Custom', basedOn: 'Normal', color: '124733' })
  if (!paragraph) bytes = await apply(bytes, 'executeDocxInsertParagraph', { text, placement: { kind: 'end' } })
  bytes = await apply(bytes, 'executeDocxSetParagraphStyle', { target: { text }, style: 'Imported Custom' })
  const output = '/private/tmp/opensuite-e1-imported-' + filename
  writeFileSync(output, bytes)
  const check = spawnSync('python3', ['-c', `
import sys,zipfile
from xml.etree import ElementTree as E
with zipfile.ZipFile(sys.argv[1]) as a,zipfile.ZipFile(sys.argv[2]) as b:
 assert a.namelist()==b.namelist()
 for name in a.namelist():
  if name not in ('word/styles.xml','word/document.xml'): assert a.read(name)==b.read(name),name
 old=a.read('word/styles.xml');new=b.read('word/styles.xml')
 ns={'w':'http://schemas.openxmlformats.org/wordprocessingml/2006/main'}
 root=E.fromstring(new); custom=root.find("w:style[@w:styleId='ImportedCustom']",ns)
 assert custom is not None
 start=new.index(b'<w:style w:type="paragraph" w:customStyle="1" w:styleId="ImportedCustom"')
 end=new.index(b'</w:style>',start)+len(b'</w:style>')
 assert new[:start]+new[end:]==old,'unrelated stylesheet bytes changed'
`, fixture.pathname, output], { encoding: 'utf8' })
  assert.equal(check.status, 0, check.stderr)
  const snapshot = JSON.parse(await binding.inspectDocxStyleSnapshot(readFileSync(output)))
  assert.equal(snapshot.styles.find(s => s.styleId === 'ImportedCustom').paragraphUsageCount, 1)
  }
})

test('existing character styles apply to partial text and clear without altering other formatting or styles', async () => {
  const { documentXml, editDocument, packageParts } = await import('./picture-support.mjs')
  let input = await apply(binding.createBlankDocx(), 'executeDocxInsertParagraph', { text: 'Before selected text after', placement: { kind: 'end' } })
  input = await apply(input, 'executeDocxCreateStyle', { styleId: 'Accent', styleType: 'character', name: 'Accent display name', italic: true, color: '124733' })
  input = editDocument(input, xml => xml.replace('<w:t>Before selected text after</w:t>', '<w:rPr><w:b/><w:lang w:val="en-US"/></w:rPr><w:t>Before selected text after</w:t>'))
  const output = await apply(input, 'executeDocxSetTextFormatting', { target: { text: 'selected text' }, characterStyleId: 'Accent' })
  assert.equal((documentXml(output).match(/<w:rStyle w:val="Accent"\/>/g) ?? []).length, 1)
  assert.match(documentXml(output), /<w:rStyle w:val="Accent"\/><w:b\/><w:lang w:val="en-US"\/>/)
  const before = packageParts(input), after = packageParts(output)
  for (const name of Object.keys(before).filter(n => n !== 'word/document.xml')) assert.deepEqual(after[name], before[name], name)
  const cleared = await apply(output, 'executeDocxSetTextFormatting', { target: { text: 'selected text' }, clear: ['characterStyleId'] })
  assert.doesNotMatch(documentXml(cleared), /<w:rStyle/)
  assert.match(documentXml(cleared), /<w:b\/><w:lang w:val="en-US"\/>/)
  assert.equal((await binding.findDocxText(cleared, { text: 'Before selected text after' })).matchCount, 1)
})

test('native character styles reject invalid types, missing IDs and protected targets without output', async () => {
  const { editDocument, editParts } = await import('./picture-support.mjs')
  let input = await apply(binding.createBlankDocx(), 'executeDocxInsertParagraph', { text: 'Target text', placement: { kind: 'end' } })
  input = await apply(input, 'executeDocxCreateStyle', { styleId: 'Accent', styleType: 'character', name: 'Accent', italic: true })
  for (const [style, code] of [['Normal', 'STYLE_TYPE_MISMATCH'], ['Missing', 'MISSING_STYLE']]) {
    const result = await binding.executeDocxSetTextFormatting(input, { target: { text: 'Target text' }, characterStyleId: style })
    assert.equal(result.result.diagnostics[0].code, code); assert.equal(result.output, undefined)
  }
  for (const edit of [
    xml => xml.replace('<w:p>', '<w:bookmarkStart w:id="8"/><w:p>').replace('</w:p>', '</w:p><w:bookmarkEnd w:id="8"/>'),
    xml => xml.replace('<w:r>', '<w:r><w:rPr><w:rStyle/><w:rStyle/></w:rPr>'),
    xml => xml.replace('<w:r>', '<w:hyperlink><w:r>').replace('</w:r>', '</w:r></w:hyperlink>'),
  ]) {
    const result = await binding.executeDocxSetTextFormatting(editDocument(input, edit), { target: { text: 'Target text' }, characterStyleId: 'Accent' })
    assert.equal(result.result.ok, false); assert.equal(result.output, undefined)
  }
  const protectedInput = editParts(input, parts => {
    parts['word/settings.xml'] = Buffer.from(parts['word/settings.xml'].toString().replace('</w:settings>', '<w:documentProtection w:enforcement="1"/></w:settings>'))
    return parts
  })
  const protectedResult = await binding.executeDocxSetTextFormatting(protectedInput, { target: { text: 'Target text' }, characterStyleId: 'Accent' })
  assert.equal(protectedResult.result.ok, false); assert.equal(protectedResult.output, undefined)
})

test('imported character style definitions and unselected references survive application and removal', async () => {
  const { editDocument, editParts, packageParts, documentXml } = await import('./picture-support.mjs')
  let input = await apply(binding.createBlankDocx(), 'executeDocxInsertParagraphs', { texts: ['Apply here', 'Keep reference'], placement: {kind:'end'} })
  input = editParts(input, parts => {
    parts['word/styles.xml'] = Buffer.from(parts['word/styles.xml'].toString().replace('</w:styles>', '<w:style w:type="character" w:styleId="ImportedAccent" w:customStyle="1"><w:name w:val="Imported accent"/><w:aliases w:val="Imported alias"/><w:rPr><w:i/><w:color w:val="124733"/></w:rPr></w:style><w:style w:type="table" w:styleId="TableOnly"><w:name w:val="Table only"/></w:style></w:styles>'))
    return parts
  })
  input = editDocument(input, xml => xml.replace('<w:t>Keep reference</w:t>', '<w:rPr><w:rStyle w:val="ImportedAccent"/></w:rPr><w:t>Keep reference</w:t>'))
  const applied = await apply(input, 'executeDocxSetTextFormatting', {target:{text:'Apply here'},characterStyleId:'ImportedAccent'})
  const cleared = await apply(applied, 'executeDocxSetTextFormatting', {target:{text:'Apply here'},clear:['characterStyleId']})
  assert.equal(documentXml(cleared).replace("<w:rPr></w:rPr>", ""),documentXml(input))
  assert.deepEqual(packageParts(cleared)['word/styles.xml'],packageParts(input)['word/styles.xml'])
  const wrongType = await binding.executeDocxSetTextFormatting(input,{target:{text:'Apply here'},characterStyleId:'TableOnly'})
  assert.equal(wrongType.result.diagnostics[0].code,'STYLE_TYPE_MISMATCH'); assert.equal(wrongType.output,undefined)
  const ambiguous = editParts(input, parts => {
    parts['word/styles.xml'] = Buffer.from(parts['word/styles.xml'].toString().replace('</w:styles>', '<w:style w:type="table" w:styleId="ImportedAccent"/></w:styles>'))
    return parts
  })
  const duplicate = await binding.executeDocxSetTextFormatting(ambiguous,{target:{text:'Apply here'},characterStyleId:'ImportedAccent'})
  assert.equal(duplicate.result.diagnostics[0].code,'DUPLICATE_STYLE_ID'); assert.equal(duplicate.output,undefined)
})
