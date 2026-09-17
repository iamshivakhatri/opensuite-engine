// prettier-ignore
/* eslint-disable */
// @ts-nocheck
/* Loader for @opensuite/engine — local .node first, then optional platform packages. */

const { existsSync } = require('fs')
const { join } = require('path')

const loadErrors = []

function tryRequire(id) {
  try {
    return require(id)
  } catch (err) {
    loadErrors.push(err)
    return null
  }
}

function requireNative() {
  if (process.env.NAPI_RS_NATIVE_LIBRARY_PATH) {
    const binding = tryRequire(process.env.NAPI_RS_NATIVE_LIBRARY_PATH)
    if (binding) return binding
  }

  const local = (suffix) => {
    const file = join(__dirname, `opensuite_node.${suffix}.node`)
    if (existsSync(file)) {
      return tryRequire(file)
    }
    return null
  }

  if (process.platform === 'darwin' && process.arch === 'arm64') {
    return (
      local('darwin-arm64') ||
      tryRequire('@opensuite/engine-darwin-arm64')
    )
  }

  if (process.platform === 'linux' && process.arch === 'x64') {
    // Prefer gnu (Docker/glibc). Musl not published in v1.
    return (
      local('linux-x64-gnu') ||
      local('linux-x64') ||
      tryRequire('@opensuite/engine-linux-x64-gnu')
    )
  }

  loadErrors.push(
    new Error(
      `Unsupported platform for @opensuite/engine: ${process.platform}-${process.arch}`,
    ),
  )
  return null
}

const nativeBinding = requireNative()

if (!nativeBinding) {
  const messages = loadErrors.map((e) => String(e && e.message ? e.message : e))
  throw new Error(
    `@opensuite/engine failed to load native binding. Tried local opensuite_node.*.node and optional platform packages. Errors:\n${messages.join('\n')}`,
  )
}

module.exports = nativeBinding
