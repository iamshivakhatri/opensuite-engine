import assert from 'node:assert/strict'
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'
import { test } from 'node:test'

test('failed package validation restores the original manifest', { skip: process.platform === 'win32' }, () => {
  const folder = mkdtempSync(join(tmpdir(), 'opensuite-pack-failure-'))
  try {
    mkdirSync(join(folder, 'scripts'))
    mkdirSync(join(folder, 'bin'))
    const original = '{"name":"@opensuitehq/engine","version":"0.1.7"}\n'
    writeFileSync(join(folder, 'package.json'), original)
    copyFileSync(new URL('../scripts/pack-dry-run.mjs', import.meta.url), join(folder, 'scripts/pack-dry-run.mjs'))
    // Simulate napi's release metadata write, then a real child validation failure.
    writeFileSync(join(folder, 'bin/npx'), `#!/usr/bin/env node
const fs = require('node:fs')
fs.writeFileSync('command-args.json', JSON.stringify(process.argv.slice(2)))
const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'))
pkg.optionalDependencies = { 'test-platform': pkg.version }
fs.writeFileSync('package.json', JSON.stringify(pkg))
`, { mode: 0o755 })
    writeFileSync(join(folder, 'scripts/validate-packages.mjs'), "process.exit(7)\n")
    const result = spawnSync(process.execPath, [join(folder, 'scripts/pack-dry-run.mjs')], {
      env: { ...process.env, PATH: `${join(folder, 'bin')}:${process.env.PATH}` }, encoding: 'utf8',
    })
    assert.notEqual(result.status, 0)
    assert.equal(readFileSync(join(folder, 'package.json'), 'utf8'), original)
    const args = JSON.parse(readFileSync(join(folder, 'command-args.json'), 'utf8'))
    assert.ok(args.includes('--skip-optional-publish'))
    assert.ok(args.includes('--no-gh-release'))
  } finally {
    rmSync(folder, { recursive: true, force: true })
  }
})
