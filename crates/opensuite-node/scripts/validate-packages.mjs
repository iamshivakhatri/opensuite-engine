/**
 * Validate assembled napi-rs npm packages before pack/publish.
 * Expects `npm/` to already contain platform packages with binaries.
 */
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'))
const npmDir = join(root, 'npm')

const expected = [
  {
    dir: 'darwin-arm64',
    name: '@opensuitehq/engine-darwin-arm64',
    binary: 'opensuite_node.darwin-arm64.node',
    os: ['darwin'],
    cpu: ['arm64'],
  },
  {
    dir: 'darwin-x64',
    name: '@opensuitehq/engine-darwin-x64',
    binary: 'opensuite_node.darwin-x64.node',
    os: ['darwin'],
    cpu: ['x64'],
  },
  {
    dir: 'linux-x64-gnu',
    name: '@opensuitehq/engine-linux-x64-gnu',
    binary: 'opensuite_node.linux-x64-gnu.node',
    os: ['linux'],
    cpu: ['x64'],
    libc: ['glibc'],
  },
  {
    dir: 'linux-arm64-gnu',
    name: '@opensuitehq/engine-linux-arm64-gnu',
    binary: 'opensuite_node.linux-arm64-gnu.node',
    os: ['linux'],
    cpu: ['arm64'],
    libc: ['glibc'],
  },
  {
    dir: 'win32-x64-msvc',
    name: '@opensuitehq/engine-win32-x64-msvc',
    binary: 'opensuite_node.win32-x64-msvc.node',
    os: ['win32'],
    cpu: ['x64'],
  },
]

function assert(condition, message) {
  if (!condition) {
    throw new Error(message)
  }
}

function sameList(actual, expectedList) {
  return (
    Array.isArray(actual) &&
    actual.length === expectedList.length &&
    expectedList.every((value, index) => actual[index] === value)
  )
}

assert(pkg.name === '@opensuitehq/engine', `unexpected root name: ${pkg.name}`)
assert(typeof pkg.version === 'string' && pkg.version.length > 0, 'root version missing')
assert(existsSync(join(root, 'index.js')), 'missing root index.js')
assert(existsSync(join(root, 'index.d.ts')), 'missing root index.d.ts')
assert(statSync(join(root, 'index.d.ts')).size > 0, 'root index.d.ts is empty')

const optional = pkg.optionalDependencies ?? {}
for (const platform of expected) {
  assert(
    optional[platform.name] === pkg.version,
    `optionalDependencies missing exact ${platform.name}@${pkg.version} (got ${optional[platform.name]})`,
  )
}

assert(existsSync(npmDir), 'npm/ directory missing — run create-npm-dirs + artifacts first')

for (const platform of expected) {
  const dir = join(npmDir, platform.dir)
  const manifestPath = join(dir, 'package.json')
  const binaryPath = join(dir, platform.binary)
  assert(existsSync(manifestPath), `missing ${platform.dir}/package.json`)
  assert(existsSync(binaryPath), `missing ${platform.dir}/${platform.binary}`)
  assert(statSync(binaryPath).size > 1024, `${platform.binary} looks empty`)

  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
  assert(manifest.name === platform.name, `${platform.dir} name mismatch`)
  assert(manifest.version === pkg.version, `${platform.dir} version mismatch`)
  assert(sameList(manifest.os, platform.os), `${platform.dir} os mismatch`)
  assert(sameList(manifest.cpu, platform.cpu), `${platform.dir} cpu mismatch`)
  if (platform.libc) {
    assert(sameList(manifest.libc, platform.libc), `${platform.dir} libc mismatch`)
  } else {
    assert(manifest.libc === undefined, `${platform.dir} unexpected libc`)
  }
  assert(manifest.main === platform.binary, `${platform.dir} main mismatch`)
  assert(
    Array.isArray(manifest.files) &&
      manifest.files.length === 1 &&
      manifest.files[0] === platform.binary,
    `${platform.dir} files must only list the .node binary`,
  )

  const entries = readdirSync(dir)
  for (const entry of entries) {
    assert(
      entry === 'package.json' ||
        entry === 'README.md' ||
        entry === platform.binary,
      `${platform.dir} has unexpected file: ${entry}`,
    )
  }
}

console.log(
  `validate-packages: ok — root ${pkg.name}@${pkg.version} with ${expected.length} platform packages`,
)
