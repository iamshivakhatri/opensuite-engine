/**
 * Emit the standard napi-rs CJS platform loader into index.js.
 *
 * @napi-rs/cli writeJsBinding is the same template `napi build --platform` uses.
 * We invoke it explicitly because napi-derive 2.16 writes TYPE_DEF_TMP_PATH while
 * CLI 3.x looks for NAPI_TYPE_DEF_TMP_FOLDER, so `napi build` currently finds no
 * type-def idents and skips regenerating the loader.
 */
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { writeJsBinding } from '@napi-rs/cli'

const dir = dirname(fileURLToPath(import.meta.url))
const root = join(dir, '..')
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'))
const dts = readFileSync(join(root, 'index.d.ts'), 'utf8')
// Declarations are generated from compiled Rust N-API type records.
const idents = [...dts.matchAll(/^export (?:declare )?function (\w+)/gm)].map((match) => match[1]).sort()

if (idents.length === 0) {
  throw new Error('generate-loader: no Rust N-API exports found')
}

const output = await writeJsBinding({
  platform: true,
  idents,
  binaryName: pkg.napi.binaryName,
  packageName: pkg.napi.packageName,
  version: pkg.version,
  outputDir: root,
})

if (!output) {
  throw new Error('generate-loader: writeJsBinding returned no output')
}

const loader = readFileSync(output.path, 'utf8')
const guard = `
// Check the native code itself, including when the loader used an optional package.
const capabilities = nativeBinding.getDocxCapabilities?.()
if (capabilities?.engineVersion !== ${JSON.stringify(pkg.version)} ||
    ${JSON.stringify(idents)}.some(name => typeof nativeBinding[name] !== 'function')) {
  throw new Error('OpenSuite native binding version or exports mismatch; reinstall @opensuitehq/engine@${pkg.version}')
}
const { existsSync } = require('node:fs')
const metadataPath = require('node:path').join(__dirname, 'native-artifacts.json')
if (existsSync(metadataPath)) {
  const records = JSON.parse(readFileSync(metadataPath, 'utf8'))
  const loaded = Object.values(require.cache).find(entry => entry.exports === nativeBinding && entry.filename.endsWith('.node'))
  const hash = loaded && require('node:crypto').createHash('sha256').update(readFileSync(loaded.filename)).digest('hex')
  if (!records.some(record => record.version === ${JSON.stringify(pkg.version)} && record.binary === require('node:path').basename(loaded?.filename || '') && record.sha256 === hash)) {
    throw new Error('OpenSuite native binding hash mismatch; reinstall @opensuitehq/engine@${pkg.version}')
  }
}
`
if (!loader.includes('module.exports = nativeBinding')) throw new Error('generate-loader: native export marker missing')
writeFileSync(output.path, loader.replace('module.exports = nativeBinding', guard + '\nmodule.exports = nativeBinding'))

console.log(`generate-loader: wrote ${output.path} (${idents.length} exports)`)
