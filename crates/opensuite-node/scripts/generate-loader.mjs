/**
 * Emit the standard napi-rs CJS platform loader into index.js.
 *
 * @napi-rs/cli writeJsBinding is the same template `napi build --platform` uses.
 * We invoke it explicitly because napi-derive 2.16 writes TYPE_DEF_TMP_PATH while
 * CLI 3.x looks for NAPI_TYPE_DEF_TMP_FOLDER, so `napi build` currently finds no
 * type-def idents and skips regenerating the loader.
 */
import { readFileSync, readdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { writeJsBinding } from '@napi-rs/cli'

const dir = dirname(fileURLToPath(import.meta.url))
const root = join(dir, '..')
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'))
const dts = readFileSync(join(root, 'index.d.ts'), 'utf8')
// Rust annotations own the exported names. Missing types stop the build.
const idents = readdirSync(join(root, 'src')).filter((name) => name.endsWith('.rs'))
  .flatMap((name) => [...readFileSync(join(root, 'src', name), 'utf8')
    .matchAll(/#\[napi\(js_name\s*=\s*"(\w+)"\)\]/g)].map((match) => match[1]))
  .sort()
const declared = [...dts.matchAll(/^export function (\w+)/gm)].map((match) => match[1]).sort()
if (JSON.stringify(idents) !== JSON.stringify(declared)) {
  throw new Error('generate-loader: Rust exports and index.d.ts differ; update the declarations')
}

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

console.log(`generate-loader: wrote ${output.path} (${idents.length} exports)`)
