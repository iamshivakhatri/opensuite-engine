import assert from 'node:assert/strict'
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'
import { test } from 'node:test'
import { checkArtifact, sha256 } from '../scripts/artifact-metadata.mjs'

test('artifact checks reject a different commit, target, version, contract or binary', () => {
  const folder = mkdtempSync(join(tmpdir(), 'opensuite-identity-'))
  try {
    const binary = join(folder, 'engine.node')
    writeFileSync(binary, 'fresh native binary')
    const expected = { version: '0.1.9', sourceCommit: 'candidate', target: 'linux-x64', contractsSha256: 'contracts' }
    const record = { ...expected, binary: 'engine.node', sha256: sha256(binary) }
    checkArtifact(record, binary, expected)
    for (const key of Object.keys(expected)) assert.throws(() => checkArtifact({ ...record, [key]: 'old' }, binary, expected), /mismatch/)
    writeFileSync(binary, 'old native binary')
    assert.throws(() => checkArtifact(record, binary, expected), /hash mismatch/)
  } finally { rmSync(folder, { recursive: true, force: true }) }
})

test('optional package fallback rejects old native versions, missing exports and wrong hashes', () => {
  const folder = mkdtempSync(join(tmpdir(), 'opensuite-loader-'))
  const platform = `${process.platform}-${process.arch}${process.platform === 'linux' ? '-gnu' : process.platform === 'win32' ? '-msvc' : ''}`
  const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url)))
  const optional = join(folder, 'node_modules', '@opensuitehq', `engine-${platform}`)
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('NAPI_RS_') && key !== 'NODE_PATH'))
  const probe = () => spawnSync(process.execPath, ['-e', "require('./index.js')"], { cwd: folder, env, encoding: 'utf8' })
  try {
    mkdirSync(optional, { recursive: true })
    copyFileSync(new URL('../index.js', import.meta.url), join(folder, 'index.js'))
    writeFileSync(join(optional, 'package.json'), JSON.stringify({ version: pkg.version, main: 'index.js' }))
    for (const version of ['0.1.8', pkg.version]) {
      writeFileSync(join(optional, 'index.js'), `module.exports = { getDocxCapabilities: () => ({ engineVersion: ${JSON.stringify(version)} }) }`)
      const result = probe()
      assert.notEqual(result.status, 0)
      assert.match(result.stderr, /native binding version or exports mismatch/)
    }
    const filename = `opensuite_node.${platform}.node`
    copyFileSync(new URL(`../${filename}`, import.meta.url), join(optional, filename))
    writeFileSync(join(optional, 'package.json'), JSON.stringify({ version: pkg.version, main: filename }))
    assert.equal(probe().status, 0)
    writeFileSync(join(folder, 'native-artifacts.json'), JSON.stringify([{ version: pkg.version, binary: filename, sha256: 'old' }]))
    assert.match(probe().stderr, /native binding hash mismatch/)
    writeFileSync(join(folder, 'native-artifacts.json'), JSON.stringify([{ version: pkg.version, binary: filename, sha256: sha256(join(optional, filename)) }]))
    assert.equal(probe().status, 0)
  } finally { rmSync(folder, { recursive: true, force: true }) }
})
