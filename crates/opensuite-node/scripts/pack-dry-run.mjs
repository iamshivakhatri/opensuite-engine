/**
 * Dry-run packaging: inject optionalDependency metadata via napi prepublish,
 * validate, npm pack, then restore the committed package.json.
 *
 * Platform optionalDependencies are release-time metadata (napi-rs standard).
 * They must not live in the committed package.json / lockfile, or `npm ci`
 * fails before the same-version platform packages exist on the registry.
 *
 * Does not publish. Requires `npm/` to already contain assembled platform packages.
 */
import { mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const packageJsonPath = join(root, 'package.json')
const tarballDir = join(root, 'dist-tarballs')

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  })
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

const committedPackageJson = readFileSync(packageJsonPath, 'utf8')

try {
  // Inject exact optionalDependencies for the current version into package.json.
  run('npx', ['napi', 'prepublish', '-t', 'npm', '--skip-optional-publish'])
  run(process.execPath, [join(root, 'scripts/validate-packages.mjs')])

  rmSync(tarballDir, { recursive: true, force: true })
  mkdirSync(tarballDir, { recursive: true })

  run('npm', ['pack', '--pack-destination', tarballDir])
  for (const dir of readdirSync(join(root, 'npm'))) {
    run('npm', ['pack', `./npm/${dir}`, '--pack-destination', tarballDir])
  }

  const tarballs = readdirSync(tarballDir)
    .filter((name) => name.endsWith('.tgz'))
    .sort()
  if (tarballs.length !== 6) {
    throw new Error(`expected 6 tarballs, found ${tarballs.length}: ${tarballs.join(', ')}`)
  }

  console.log('pack-dry-run: tarballs')
  for (const name of tarballs) {
    const size = statSync(join(tarballDir, name)).size
    console.log(`  ${name} (${size} bytes)`)
  }
} finally {
  // Keep the working tree free of unpublished optionalDependencies.
  writeFileSync(packageJsonPath, committedPackageJson)
}
