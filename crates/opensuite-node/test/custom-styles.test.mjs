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
  bytes = await apply(bytes, 'executeDocxUpdateStyle', { styleId: 'OpenSuiteReportHeading', styleType: 'paragraph', color: '235744', spacingAfterTwips: 160, clear: ['bold'] })
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
