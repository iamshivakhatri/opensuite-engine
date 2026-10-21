/** Compile Rust inputs, generate schemas/declarations, then embed those schemas. */
import { mkdtempSync, rmSync, readFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'
import { generateInputs } from './generate-inputs.mjs'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
// A development rebuild replaces release bytes; packaging regenerates their hashes.
rmSync(join(root, 'native-artifacts.json'), { force: true })
const folder = mkdtempSync(join(tmpdir(), 'opensuite-types-'))
const args = process.argv.slice(2)
const napiArgs = ['napi', 'build', '--platform', ...(args.includes('--debug') ? [] : ['--release']), '--no-js', '--dts', '.native-cli.d.ts', ...args.filter((arg) => arg !== '--debug')]
function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { cwd: root, env, stdio: 'inherit', shell: process.platform === 'win32' })
  if (result.status !== 0) throw new Error(`${command} failed: ${result.error?.message ?? result.status}`)
}
try {
  // A cached crate does not re-emit type records; only clear this adapter's outputs.
  const targetIndex = args.indexOf('--target')
  const target = targetIndex >= 0 ? args[targetIndex + 1]
    : spawnSync('rustc', ['-vV'], { encoding: 'utf8' }).stdout?.match(/^host: (.+)$/m)?.[1]
  if (!target) throw new Error('Cannot determine the native build target')
  // napi builds into target/<triple>, even for a host build.
  run('cargo', ['clean', '-p', 'opensuite-node', '--target', target, ...(args.includes('--debug') ? [] : ['--release'])])
  run('npx', napiArgs, { ...process.env, TYPE_DEF_TMP_PATH: join(folder, 'opensuite-node') })
  await generateInputs(root, folder)
  const contracts = readFileSync(join(root, 'operation-inputs.json'), 'utf8')
  // include_str! makes Cargo rebuild when generated contracts change.
  run('npx', napiArgs)
  run(process.execPath, [join(root, 'scripts/generate-loader.mjs')])
  if (readFileSync(join(root, 'operation-inputs.json'), 'utf8') !== contracts) throw new Error('Input contracts changed during the second build')
} finally {
  rmSync(folder, { recursive: true, force: true })
  rmSync(join(root, ".native-cli.d.ts"), { force: true })
}
