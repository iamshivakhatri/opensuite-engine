/** Record native identity at build time; check it again before packaging. */
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync, readdirSync, writeFileSync } from 'node:fs'
import { basename, join, resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'

export const sha256 = path => createHash('sha256').update(readFileSync(path)).digest('hex')
export const sourceCommit = root => execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim()

export function checkArtifact(record, binary, expected) {
  for (const [key, value] of Object.entries(expected)) assert.equal(record[key], value, `artifact ${key} mismatch: ${binary}`)
  assert.equal(record.binary, basename(binary), 'artifact filename mismatch')
  assert.equal(record.sha256, sha256(binary), `artifact hash mismatch: ${binary}`)
}

export function checkDownloads(root) {
  const pkg = JSON.parse(readFileSync(join(root, 'package.json')))
  assert.equal(readFileSync(join(root, 'artifacts/engine-version'), 'utf8').trim(), pkg.version, 'artifact version marker mismatch')
  for (const target of pkg.napi.targets) {
    const folder = join(root, 'artifacts', `bindings-${target}`)
    const binaries = readdirSync(folder).filter(name => name.endsWith('.node'))
    assert.equal(binaries.length, 1, `expected one binary for ${target}`)
    const binary = join(folder, binaries[0])
    checkArtifact(JSON.parse(readFileSync(`${binary}.json`)), binary, {
      version: pkg.version, sourceCommit: sourceCommit(root), target,
      contractsSha256: sha256(join(root, 'operation-inputs.json')),
    })
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv[2] === '--check-downloads') {
    checkDownloads(process.cwd())
  } else {
    const [target, filename] = process.argv.slice(2)
    assert.ok(target && filename, 'usage: node scripts/artifact-metadata.mjs <target> <binary>')
    execFileSync('git', ['diff', '--exit-code', 'HEAD', '--'], { cwd: process.cwd(), stdio: 'inherit' })
    const binary = resolve(filename)
    const engine = createRequire(import.meta.url)(binary)
    const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url)))
    const capabilities = engine.getDocxCapabilities()
    assert.equal(capabilities.engineVersion, pkg.version, 'native version mismatch')
    assert.ok(target.startsWith(process.arch === 'arm64' ? 'aarch64-' : 'x86_64-'), 'record identity on the target CPU')
    assert.ok(target.includes({ darwin: 'apple-darwin', linux: 'linux-gnu', win32: 'windows-msvc' }[process.platform]), 'record identity on the target OS')
    const record = {
      version: pkg.version, sourceCommit: sourceCommit(process.cwd()), target,
      binary: basename(binary), sha256: sha256(binary),
      contractsSha256: sha256(new URL('../operation-inputs.json', import.meta.url)),
      capabilities,
    }
    writeFileSync(`${binary}.json`, JSON.stringify(record, null, 2) + '\n')
    console.log(JSON.stringify({ ...record, capabilities: undefined }))
  }
}
