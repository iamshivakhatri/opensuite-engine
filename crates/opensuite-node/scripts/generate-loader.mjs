/**
 * Emit the standard napi-rs CJS platform loader into index.js.
 *
 * @napi-rs/cli writeJsBinding is the same template `napi build --platform` uses.
 * We invoke it explicitly because napi-derive 2.16 writes TYPE_DEF_TMP_PATH while
 * CLI 3.x looks for NAPI_TYPE_DEF_TMP_FOLDER, so `napi build` currently finds no
 * type-def idents and skips regenerating the loader.
 */
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { writeJsBinding } from '@napi-rs/cli'

const dir = dirname(fileURLToPath(import.meta.url))
const root = join(dir, '..')
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'))
const dts = readFileSync(join(root, 'index.d.ts'), 'utf8')
const idents = [...dts.matchAll(/^export function (\w+)/gm)].map((match) => match[1])

if (idents.length === 0) {
  throw new Error('generate-loader: no export function names found in index.d.ts')
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
