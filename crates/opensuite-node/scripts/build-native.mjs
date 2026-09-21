/**
 * Native build wrapper for @opensuite/engine.
 *
 * `napi build` (CLI 3.x + napi-derive 2.16) currently emits an empty index.d.ts
 * because type-def env vars disagree (NAPI_TYPE_DEF_TMP_FOLDER vs TYPE_DEF_TMP_PATH).
 * Preserve the hand-maintained declarations, then emit the standard napi-rs loader.
 */
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const dtsPath = join(root, 'index.d.ts')
const dtsBackup = readFileSync(dtsPath)

const args = process.argv.slice(2)
const debug = args.includes('--debug')
const passthrough = args.filter((arg) => arg !== '--debug')

const napiArgs = [
  'napi',
  'build',
  '--platform',
  ...(debug ? [] : ['--release']),
  '--no-js',
  ...passthrough,
]

const build = spawnSync('npx', napiArgs, {
  cwd: root,
  stdio: 'inherit',
  shell: process.platform === 'win32',
})
if (build.status !== 0) {
  process.exit(build.status ?? 1)
}

writeFileSync(dtsPath, dtsBackup)

const generate = spawnSync(process.execPath, [join(root, 'scripts/generate-loader.mjs')], {
  cwd: root,
  stdio: 'inherit',
})
process.exit(generate.status ?? 1)
