/** Install the release archives in isolation and exercise the shipped native code. */
import assert from 'node:assert/strict'
import { mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, join, resolve } from 'node:path'
import { createRequire } from 'node:module'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

for (const key of Object.keys(process.env)) {
  assert.ok(key !== 'OPENSUITE_ENGINE_PATH' && !key.startsWith('NAPI_RS_') && key !== 'NODE_PATH', `unset ${key} for package acceptance`)
}
const root = fileURLToPath(new URL('..', import.meta.url))
const pkg = JSON.parse(readFileSync(join(root, 'package.json')))
const platform = `${process.platform}-${process.arch}${process.platform === 'linux' ? '-gnu' : process.platform === 'win32' ? '-msvc' : ''}`
const archives = readdirSync(join(root, 'dist-tarballs')).filter(name => name.endsWith('.tgz'))
assert.equal(archives.length, 6, 'all six package archives are required')
const installedFolder = process.argv[2] === '--installed' ? process.argv[3] : undefined
const folder = installedFolder ?? realpathSync(mkdtempSync(join(tmpdir(), 'opensuite-packed-')))
function run(command, args, cwd = folder) {
  const result = spawnSync(command, args, { cwd, encoding: 'utf8', shell: process.platform === 'win32' })
  assert.equal(result.status, 0, result.stdout + result.stderr)
  return result
}
try {
  if (!installedFolder) {
    writeFileSync(join(folder, 'package.json'), '{"private":true}')
    const names = [`opensuitehq-engine-${pkg.version}.tgz`, `opensuitehq-engine-${platform}-${pkg.version}.tgz`]
    run('npm', ['install', '--cache', join(folder, 'cache'), '--offline', '--ignore-scripts', '--no-audit', '--no-fund', ...names.map(name => resolve(root, 'dist-tarballs', name))])
    // Loading in a child lets Windows release the native library before cleanup.
    console.log(run(process.execPath, [fileURLToPath(import.meta.url), '--installed', folder]).stdout.trim())
  } else {
    const require = createRequire(join(folder, 'package.json'))
    const engine = require('@opensuitehq/engine')
    const installed = require.resolve('@opensuitehq/engine')
    assert.ok(installed.startsWith(folder), installed)
    assert.equal(require('@opensuitehq/engine/package.json').version, pkg.version)
    const records = require('@opensuitehq/engine/native-artifacts.json')
    const record = records.find(record => record.binary === `opensuite_node.${platform}.node`)
    assert.ok(record, 'target identity missing')
    assert.deepEqual(engine.getDocxCapabilities(), record.capabilities)
    assert.equal(record.capabilities.engineVersion, pkg.version)
    assert.equal(record.capabilities.protocolVersion, 1)
    const loaded = Object.keys(require.cache).find(path => basename(path) === record.binary)
    assert.ok(loaded?.startsWith(join(folder, 'node_modules', '@opensuitehq', `engine-${platform}`)), loaded)

    let bytes = engine.createBlankDocx()
    async function apply(method, input) {
      const result = await engine[method](bytes, input)
      assert.equal(result.result.ok, true, JSON.stringify(result.result))
      bytes = result.output
    }
    await apply('executeDocxInsertParagraph', { text: 'release candidate', placement: { kind: 'end' } })
    await apply('executeDocxReplaceText', { target: { text: 'release candidate' }, expectedCurrentText: 'release candidate', replacement: 'verified candidate' })
    await apply('executeDocxCreateTable', { rows: [['Item', 'Value'], ['Logo', 'Keep caption']], placement: { kind: 'end' } })
    await apply('executeDocxSetTableColumnWidths', { table: { headerCells: ['Item', 'Value'] }, widthsTwips: [3000, 3000] })
    const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL7WQAAAABJRU5ErkJggg==', 'base64')
    await apply('executeDocxInsertPicture', { imageBytes: png, placement: { kind: 'end' } })
    const table = (await engine.inspectDocx(bytes, { focus: { kind: 'tables' } })).tables.items[0]
    await apply('executeDocxInsertPicture', { imageBytes: png, placement: { kind: 'tableCell', handle: table.rows[1].pictureCellHandles[1] } })
    const section = JSON.parse(await engine.inspectDocxSections(bytes)).sections[0]
    await apply('executeDocxInsertPicture', { imageBytes: png, placement: { kind: 'header', variant: 'default', handle: section.handle } })
    writeFileSync(join(folder, 'saved.docx'), bytes)
    bytes = readFileSync(join(folder, 'saved.docx'))
    const paragraphs = await engine.inspectDocx(bytes, { focus: { kind: 'paragraphs', limit: 10 } })
    assert.ok(paragraphs.paragraphs.items.some(item => item.text === 'verified candidate'))
    const images = JSON.parse(await engine.inspectDocxImages(bytes)).images
    assert.equal(images.length, 3)
    for (const image of images) {
      const result = await engine.readDocxEmbeddedImage(bytes, { handle: image.handle })
      assert.equal(result.result.ok, true)
      assert.deepEqual(result.imageBytes, png)
    }
    const probe = () => spawnSync(process.execPath, ['-e', "require('@opensuitehq/engine')"], { cwd: folder, encoding: 'utf8' })
    const metadataPath = join(installed, '..', 'native-artifacts.json')
    const metadata = readFileSync(metadataPath)
    writeFileSync(metadataPath, JSON.stringify(records.map(item => ({ ...item, sha256: '0'.repeat(64) }))))
    const stale = probe()
    assert.notEqual(stale.status, 0)
    assert.match(stale.stderr, /native binding hash mismatch/)
    writeFileSync(metadataPath, metadata)
    renameSync(loaded, `${loaded}.missing`)
    try {
      const missing = probe()
      assert.notEqual(missing.status, 0)
      assert.match(missing.stderr, /Cannot find native binding|Failed to load native binding/)
    } finally { renameSync(`${loaded}.missing`, loaded) }
    const docx = record.capabilities.formats.find(item => item.format === 'docx')
    console.log(JSON.stringify({ version: pkg.version, target: record.target, sourceCommit: record.sourceCommit, sha256: record.sha256, capabilities: docx.capabilities.length, operations: docx.operations.length, exports: new Set(docx.operations.map(item => item.nodeExport)).size, loaded, overrides: 'none', runtime: process.report.getReport().header.glibcVersionRuntime, result: 'PASS' }))
  }
} finally { if (!installedFolder) rmSync(folder, { recursive: true, force: true }) }
