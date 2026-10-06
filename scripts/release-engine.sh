#!/usr/bin/env bash
# Prepare (and optionally tag/push) an @opensuitehq/engine release.
# GitHub Actions publish-engine.yml remains the only npm publisher.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE_DIR="$ROOT/crates/opensuite-node"
PACKAGE_NAME="@opensuitehq/engine"
EXPECTED_BRANCH="main"
VERSION=""
DO_PUBLISH=0
DO_CHECK=0
# Explicit no-publish flag (default when VERSION is given without --publish).
DO_DRY_RUN=0

RELEASE_PATHS=(
  Cargo.lock
  crates/opensuite-node/package.json
  crates/opensuite-node/package-lock.json
  crates/opensuite-node/Cargo.toml
  crates/opensuite-node/index.js
)

# Expected CI artifact layout from publish-engine.yml (download-artifact).
PLATFORM_BINDING_ARTIFACTS=(
  artifacts/bindings-aarch64-apple-darwin/opensuite_node.darwin-arm64.node
  artifacts/bindings-x86_64-apple-darwin/opensuite_node.darwin-x64.node
  artifacts/bindings-x86_64-unknown-linux-gnu/opensuite_node.linux-x64-gnu.node
  artifacts/bindings-aarch64-unknown-linux-gnu/opensuite_node.linux-arm64-gnu.node
  artifacts/bindings-x86_64-pc-windows-msvc/opensuite_node.win32-x64-msvc.node
)

PUBLISH_ORDER=(
  "@opensuitehq/engine-darwin-arm64"
  "@opensuitehq/engine-darwin-x64"
  "@opensuitehq/engine-linux-x64-gnu"
  "@opensuitehq/engine-linux-arm64-gnu"
  "@opensuitehq/engine-win32-x64-msvc"
  "@opensuitehq/engine"
)

usage() {
  cat <<'EOF'
Usage:
  ./scripts/release-engine.sh --check              # non-destructive preflight
  ./scripts/release-engine.sh --check X.Y.Z        # preflight + assert repo already at X.Y.Z
  ./scripts/release-engine.sh X.Y.Z                # prepare + verify (no commit/tag/push)
  ./scripts/release-engine.sh X.Y.Z --dry-run      # same as X.Y.Z (explicit)
  ./scripts/release-engine.sh X.Y.Z --publish      # prepare + verify + commit + tag + push

What this script does:
  - Bumps Node release version locations (unless --check)
  - Verifies version synchronization
  - Runs fmt/check/clippy/test + local Node build/tests
  - Runs a local packaged-consumer smoke test (host platform binding)
  - With --publish: commits, tags vX.Y.Z, pushes (triggers CI npm publish)

What this script never does:
  - npm publish (only .github/workflows/publish-engine.yml publishes)
  - Create a GitHub Release

Environment:
  No special env vars required for prepare/--check.
  --publish needs git push access to origin.
  npm view / tag checks need network; skipped soft failures are noted.

Examples:
  ./scripts/release-engine.sh --check
  ./scripts/release-engine.sh --check 0.2.0
  ./scripts/release-engine.sh 0.2.0 --dry-run
  ./scripts/release-engine.sh 0.2.0 --publish
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

info() {
  echo "==> $*"
}

warn() {
  echo "warning: $*" >&2
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

parse_args() {
  if [[ $# -lt 1 ]]; then
    usage
    exit 1
  fi
  for arg in "$@"; do
    case "$arg" in
    --publish) DO_PUBLISH=1 ;;
    --dry-run) DO_DRY_RUN=1 ;;
    --check) DO_CHECK=1 ;;
    -h | --help)
      usage
      exit 0
      ;;
    --*)
      die "unknown option: $arg"
      ;;
    *)
      if [[ -n "$VERSION" ]]; then
        die "unexpected argument: $arg"
      fi
      VERSION="$arg"
      ;;
    esac
  done

  if [[ "$DO_CHECK" -eq 1 ]]; then
    if [[ "$DO_PUBLISH" -eq 1 || "$DO_DRY_RUN" -eq 1 ]]; then
      die "--check cannot be combined with --publish or --dry-run"
    fi
    return
  fi

  [[ -n "$VERSION" ]] || die "version X.Y.Z is required (or use --check)"

  if [[ "$DO_PUBLISH" -eq 1 && "$DO_DRY_RUN" -eq 1 ]]; then
    die "--publish and --dry-run are mutually exclusive"
  fi

  # Default path without --publish is a dry-run of git/npm publish side effects.
  if [[ "$DO_PUBLISH" -eq 0 ]]; then
    DO_DRY_RUN=1
  fi
}

validate_semver() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
    || die "version must be X.Y.Z (digits only), got: $1"
}

require_clean_worktree() {
  if [[ -n "$(git -C "$ROOT" status --porcelain)" ]]; then
    die "git worktree is dirty; commit or stash changes before releasing"
  fi
}

require_expected_branch() {
  local branch
  branch="$(git -C "$ROOT" rev-parse --abbrev-ref HEAD)"
  if [[ "$branch" != "$EXPECTED_BRANCH" ]]; then
    die "expected branch ${EXPECTED_BRANCH}, currently on ${branch}"
  fi
}

require_tag_absent() {
  local tag="v${VERSION}"
  if git -C "$ROOT" rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
    die "tag ${tag} already exists locally"
  fi
  if git -C "$ROOT" fetch --tags --quiet origin 2>/dev/null; then
    if git -C "$ROOT" ls-remote --exit-code --tags origin "refs/tags/${tag}" >/dev/null 2>&1; then
      die "tag ${tag} already exists on origin"
    fi
  else
    warn "could not fetch tags from origin; skipped remote tag check"
  fi
}

require_npm_version_absent() {
  local status
  if ! status="$(npm view "${PACKAGE_NAME}@${VERSION}" version 2>/dev/null)"; then
    # Missing package/version is the success case for a new release.
    return 0
  fi
  if [[ "$status" == "$VERSION" ]]; then
    die "${PACKAGE_NAME}@${VERSION} already exists on npm"
  fi
}

check_npm_auth_soft() {
  if npm whoami >/dev/null 2>&1; then
    info "npm whoami: $(npm whoami 2>/dev/null)"
  else
    warn "npm whoami failed (local auth not required; CI uses OIDC Trusted Publishing)"
  fi
}

node_pkg_version() {
  node -p "require('${NODE_DIR}/package.json').version"
}

cargo_pkg_version() {
  # shellcheck disable=SC2016
  node -e '
    const fs = require("fs");
    const text = fs.readFileSync(process.argv[1], "utf8");
    const match = text.match(/^version\s*=\s*"([^"]+)"/m);
    if (!match) process.exit(1);
    process.stdout.write(match[1]);
  ' "$NODE_DIR/Cargo.toml"
}

lock_pkg_version() {
  node -p "require('${NODE_DIR}/package-lock.json').version"
}

loader_version() {
  # First hardcoded bindingPackageVersion check in the generated loader.
  node -e '
    const fs = require("fs");
    const text = fs.readFileSync(process.argv[1], "utf8");
    const match = text.match(/bindingPackageVersion !== '\''([^'\'']+)'\''/);
    if (!match) {
      console.error("could not find bindingPackageVersion in index.js");
      process.exit(1);
    }
    process.stdout.write(match[1]);
  ' "$NODE_DIR/index.js"
}

cargo_lock_version() {
  node -e '
    const fs = require("fs");
    const text = fs.readFileSync(process.argv[1], "utf8");
    const match = text.match(/name = "opensuite-node"\nversion = "([^"]+)"/);
    if (!match) {
      console.error("could not find opensuite-node version in Cargo.lock");
      process.exit(1);
    }
    process.stdout.write(match[1]);
  ' "$ROOT/Cargo.lock"
}

assert_no_optional_deps() {
  node -e '
    const pkg = require(process.argv[1]);
    if (pkg.optionalDependencies && Object.keys(pkg.optionalDependencies).length) {
      console.error("package.json must not commit optionalDependencies");
      process.exit(1);
    }
  ' "$NODE_DIR/package.json"
}

assert_package_layout() {
  node -e '
    const fs = require("fs");
    const path = require("path");
    const root = process.argv[1];
    const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
    const expectedTargets = [
      "x86_64-apple-darwin",
      "aarch64-apple-darwin",
      "x86_64-unknown-linux-gnu",
      "aarch64-unknown-linux-gnu",
      "x86_64-pc-windows-msvc",
    ];
    if (pkg.name !== "@opensuitehq/engine") {
      console.error("unexpected package name:", pkg.name);
      process.exit(1);
    }
    if (!pkg.napi || pkg.napi.packageName !== "@opensuitehq/engine") {
      console.error("napi.packageName must be @opensuitehq/engine");
      process.exit(1);
    }
    const targets = pkg.napi.targets || [];
    if (targets.length !== expectedTargets.length ||
        expectedTargets.some((t, i) => targets[i] !== t)) {
      console.error("napi.targets mismatch.\n expected:", expectedTargets.join(", "),
        "\n actual:  ", targets.join(", "));
      process.exit(1);
    }
    for (const rel of ["index.js", "index.d.ts", "Cargo.toml", "package-lock.json"]) {
      if (!fs.existsSync(path.join(root, rel))) {
        console.error("missing required file:", rel);
        process.exit(1);
      }
    }
    const files = pkg.files || [];
    if (!(files.includes("index.js") && files.includes("index.d.ts"))) {
      console.error("package.json files must include index.js and index.d.ts");
      process.exit(1);
    }
    console.log("package layout ok:", pkg.name, "targets=", targets.length);
  ' "$NODE_DIR"
}

# Verify all release version locations agree with each other.
# If EXPECTED is non-empty, also require that shared version equals EXPECTED.
verify_versions_synced() {
  local expected="${1:-}"
  local pkg cargo lock loader cargo_lock
  info "verifying version alignment"
  pkg="$(node_pkg_version)"
  cargo="$(cargo_pkg_version)"
  lock="$(lock_pkg_version)"
  loader="$(loader_version)"
  cargo_lock="$(cargo_lock_version)"

  [[ "$pkg" == "$cargo" ]] || die "package.json (${pkg}) != Cargo.toml (${cargo})"
  [[ "$pkg" == "$lock" ]] || die "package.json (${pkg}) != package-lock.json (${lock})"
  [[ "$pkg" == "$loader" ]] || die "package.json (${pkg}) != index.js loader (${loader})"
  [[ "$pkg" == "$cargo_lock" ]] || die "package.json (${pkg}) != Cargo.lock opensuite-node (${cargo_lock})"

  if [[ -n "$expected" ]]; then
    [[ "$pkg" == "$expected" ]] || die "repo is at ${pkg}, expected ${expected}"
  fi

  assert_no_optional_deps
  echo "    synchronized at ${pkg}"
}

bump_versions() {
  info "bumping ${PACKAGE_NAME} to ${VERSION}"
  node -e '
    const fs = require("fs");
    const path = process.argv[1];
    const version = process.argv[2];
    const pkg = JSON.parse(fs.readFileSync(path, "utf8"));
    pkg.version = version;
    fs.writeFileSync(path, JSON.stringify(pkg, null, 2) + "\n");
  ' "$NODE_DIR/package.json" "$VERSION"

  node -e '
    const fs = require("fs");
    const path = process.argv[1];
    const version = process.argv[2];
    const text = fs.readFileSync(path, "utf8");
    const match = text.match(/^version\s*=\s*"([^"]+)"/m);
    if (!match) {
      console.error("failed to find version in", path);
      process.exit(1);
    }
    if (match[1] === version) {
      process.exit(0);
    }
    const updated = text.replace(/^version\s*=\s*"[^"]+"/m, `version = "${version}"`);
    fs.writeFileSync(path, updated);
  ' "$NODE_DIR/Cargo.toml" "$VERSION"

  # Keep package-lock.json root version aligned without rewriting the tree.
  node -e '
    const fs = require("fs");
    const path = process.argv[1];
    const version = process.argv[2];
    const lock = JSON.parse(fs.readFileSync(path, "utf8"));
    lock.version = version;
    if (lock.packages && lock.packages[""]) {
      lock.packages[""].version = version;
    }
    fs.writeFileSync(path, JSON.stringify(lock, null, 2) + "\n");
  ' "$NODE_DIR/package-lock.json" "$VERSION"

  (
    cd "$ROOT"
    cargo update -p opensuite-node --precise "$VERSION" >/dev/null
  )

  (
    cd "$NODE_DIR"
    npm run generate-loader
  )
}

verify_versions() {
  verify_versions_synced "$VERSION"
}

have_all_platform_artifacts() {
  local rel
  for rel in "${PLATFORM_BINDING_ARTIFACTS[@]}"; do
    if [[ ! -f "${NODE_DIR}/${rel}" ]]; then
      return 1
    fi
  done
  return 0
}

maybe_pack_dry_run() {
  if have_all_platform_artifacts; then
    info "all five platform artifacts present — running full pack dry-run"
    (
      cd "$NODE_DIR"
      npm run create-npm-dirs
      npm run artifacts -- --output-dir artifacts --npm-dir npm
      npm run pack:dry-run
      assert_no_optional_deps
    )
  else
    info "skipping full six-package pack dry-run (cross-platform artifacts not present locally)"
    echo "    Cross-platform build, package assembly, and publish validation run in"
    echo "    .github/workflows/publish-engine.yml after the v${VERSION} tag is pushed."
  fi
}

# Strongest local packaged-consumer path: npm-pack the root JS package, install
# into a temp project, drop the host .node next to the loader, then inspect/mutate.
local_consumer_smoke() {
  local binding="" pkg_version tarball tmp consumer installed
  local candidate
  shopt -s nullglob
  for candidate in "$NODE_DIR"/opensuite_node.*.node; do
    binding="$candidate"
    break
  done
  shopt -u nullglob
  if [[ -z "$binding" ]]; then
    die "local_consumer_smoke: no opensuite_node.*.node next to package (run npm run build first)"
  fi

  pkg_version="$(node_pkg_version)"
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/opensuite-engine-smoke.XXXXXX")"
  info "local packaged-consumer smoke in ${tmp}"

  (
    cd "$NODE_DIR"
    # Root pack contains only index.js + index.d.ts (native via optionalDeps on registry).
    npm pack --pack-destination "$tmp" --silent >/dev/null
  )

  tarball=""
  shopt -s nullglob
  for candidate in "$tmp"/opensuitehq-engine-"${pkg_version}".tgz; do
    tarball="$candidate"
    break
  done
  shopt -u nullglob
  [[ -n "$tarball" ]] || die "npm pack did not produce opensuitehq-engine-${pkg_version}.tgz"

  consumer="${tmp}/consumer"
  mkdir -p "$consumer"
  (
    cd "$consumer"
    npm init -y >/dev/null
    npm install --no-save "$tarball" >/dev/null
  )

  installed="${consumer}/node_modules/@opensuitehq/engine"
  [[ -d "$installed" ]] || die "smoke install missing ${installed}"
  [[ -f "${installed}/index.js" ]] || die "smoke package missing index.js"
  [[ -f "${installed}/index.d.ts" ]] || die "smoke package missing index.d.ts"
  cp "$binding" "$installed/"

  (
    cd "$consumer"
    node --input-type=commonjs <<'EOF'
const assert = require('node:assert/strict')
const engine = require('@opensuitehq/engine')
assert.equal(typeof engine.createBlankDocx, 'function')
assert.equal(typeof engine.inspectDocx, 'function')
assert.equal(typeof engine.executeDocxInsertParagraph, 'function')

async function main() {
  let bytes = engine.createBlankDocx()
  assert.ok(Buffer.isBuffer(bytes) && bytes.length > 0)

  const inserted = await engine.executeDocxInsertParagraph(bytes, {
    text: 'release smoke',
    placement: { kind: 'end' },
  })
  assert.equal(inserted.result.ok, true, JSON.stringify(inserted.result))
  assert.ok(Buffer.isBuffer(inserted.output))
  bytes = inserted.output

  const inspected = await engine.inspectDocx(bytes, {
    focus: { kind: 'paragraphs', offset: 0, limit: 10 },
  })
  assert.equal(inspected.ok, true, JSON.stringify(inspected))
  const texts = (inspected.paragraphs?.items || []).map((p) => p.text)
  assert.ok(texts.includes('release smoke'), `missing paragraph text: ${JSON.stringify(texts)}`)

  const reopened = await engine.inspectDocx(bytes, { focus: { kind: 'overview' } })
  assert.equal(reopened.ok, true, JSON.stringify(reopened))
  console.log('local packaged-consumer smoke: ok')
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
EOF
  )

  rm -rf "$tmp"
}

run_checks() {
  info "PHASE: BUILD/VERIFY"

  info "cargo fmt --check"
  (cd "$ROOT" && cargo fmt --check)

  info "cargo check (release-relevant crates)"
  (cd "$ROOT" && cargo check -p opensuite-node -p opensuite-docx -p opensuite-opc -p opensuite-protocol)

  info "cargo clippy (release-relevant crates; excludes opensuite-pptx)"
  (
    cd "$ROOT"
    cargo clippy \
      -p opensuite-node \
      -p opensuite-docx \
      -p opensuite-opc \
      -p opensuite-protocol \
      --all-targets \
      -- -D warnings
  )

  info "cargo test (release-relevant crates)"
  (
    cd "$ROOT"
    cargo test \
      -p opensuite-node \
      -p opensuite-docx \
      -p opensuite-opc \
      -p opensuite-protocol
  )

  info "npm ci + native build + node tests"
  (
    cd "$NODE_DIR"
    npm ci
    npm run build
    npm test
  )

  assert_no_optional_deps
  local_consumer_smoke
  maybe_pack_dry_run
}

assert_only_release_paths_dirty() {
  local path allowed
  while IFS= read -r path; do
    [[ -z "$path" ]] && continue
    allowed=0
    for expected in "${RELEASE_PATHS[@]}"; do
      if [[ "$path" == "$expected" ]]; then
        allowed=1
        break
      fi
    done
    if [[ "$allowed" -eq 0 ]]; then
      git -C "$ROOT" status --porcelain >&2
      die "unexpected dirty path before commit: ${path}"
    fi
  done < <(git -C "$ROOT" status --porcelain --untracked-files=no | awk '{print $2}')
}

print_publish_order() {
  local i=1
  echo "  CI npm publish order (platforms then root):"
  for name in "${PUBLISH_ORDER[@]}"; do
    echo "    ${i}. ${name}@${VERSION:-<version>}"
    i=$((i + 1))
  done
}

print_plan() {
  local tag="v${VERSION}"
  cat <<EOF

Release plan for ${PACKAGE_NAME}@${VERSION}
  tag:              ${tag}
  commit message:   release: prepare ${VERSION}
  files touched:    crates/opensuite-node/package.json
                    crates/opensuite-node/package-lock.json
                    crates/opensuite-node/Cargo.toml
                    crates/opensuite-node/index.js
                    Cargo.lock (if changed)

  npm publisher:    .github/workflows/publish-engine.yml (on tag push)
  app install pin:  ${PACKAGE_NAME}@${VERSION}

EOF
  print_publish_order
  cat <<EOF

  Cross-platform native builds and full npm package validation happen in
  publish-engine.yml (not required locally for a normal Mac release).

EOF
  if [[ "$DO_PUBLISH" -eq 0 ]]; then
    cat <<EOF
Dry run complete (no commit/tag/push; this script never runs npm publish).
Working tree now contains the prepared ${VERSION} bump for review.
To discard and publish for real from a clean tree:
  git reset --hard HEAD
  ./scripts/release-engine.sh ${VERSION} --publish
EOF
  fi
}

do_publish() {
  local tag="v${VERSION}"
  info "PHASE: PUBLISH (git commit/tag/push only — not npm)"
  assert_only_release_paths_dirty

  info "creating release commit"
  (
    cd "$ROOT"
    git add -- "${RELEASE_PATHS[@]}"
    if git diff --cached --quiet; then
      info "release files already at ${VERSION}; skipping prepare commit"
    else
      git commit -m "$(cat <<EOF
release: prepare ${VERSION}

EOF
)"
    fi
  )

  info "creating annotated tag ${tag}"
  git -C "$ROOT" tag -a "$tag" -m "Release ${PACKAGE_NAME}@${VERSION}"

  info "pushing release commit"
  git -C "$ROOT" push origin HEAD

  info "pushing tag ${tag} (triggers publish-engine.yml)"
  git -C "$ROOT" push origin "$tag"

  cat <<EOF

Pushed ${tag}. GitHub Actions workflow publish-engine.yml now builds platform
binaries and publishes to npm (platforms first, then root). This script did
not run npm publish.

Install in the main opensuite repo:
  ${PACKAGE_NAME}@${VERSION}

See docs/releasing.md for post-publish verification and app follow-up.
EOF
}

run_check_mode() {
  info "PHASE: PREFLIGHT (--check, non-destructive)"
  require_cmd git
  require_cmd node
  require_cmd npm
  require_cmd cargo
  require_cmd npx

  cd "$ROOT"

  local branch
  branch="$(git -C "$ROOT" rev-parse --abbrev-ref HEAD)"
  if [[ "$branch" != "$EXPECTED_BRANCH" ]]; then
    warn "expected release branch ${EXPECTED_BRANCH}, currently on ${branch}"
  else
    info "branch: ${branch}"
  fi

  if [[ -n "$(git -C "$ROOT" status --porcelain)" ]]; then
    warn "git worktree is dirty (allowed for --check)"
    git -C "$ROOT" status --short
  else
    info "git worktree is clean"
  fi

  assert_package_layout

  if [[ -n "$VERSION" ]]; then
    validate_semver "$VERSION"
    verify_versions_synced "$VERSION"
    require_tag_absent
    require_npm_version_absent
  else
    verify_versions_synced
  fi

  check_npm_auth_soft

  cat <<EOF

Preflight OK.
  current synchronized version: $(node_pkg_version)
  release docs:                 docs/releasing.md
  prepare (no publish):         ./scripts/release-engine.sh X.Y.Z --dry-run
  publish via tag push:         ./scripts/release-engine.sh X.Y.Z --publish

EOF
  VERSION="$(node_pkg_version)"
  print_publish_order
  echo
}

main() {
  parse_args "$@"

  if [[ "$DO_CHECK" -eq 1 ]]; then
    run_check_mode
    return
  fi

  validate_semver "$VERSION"

  require_cmd git
  require_cmd node
  require_cmd npm
  require_cmd cargo
  require_cmd npx

  cd "$ROOT"
  require_expected_branch
  require_clean_worktree
  assert_package_layout
  require_tag_absent
  require_npm_version_absent

  info "PHASE: PREPARE (bump versions)"
  bump_versions
  verify_versions
  run_checks
  verify_versions
  print_plan

  if [[ "$DO_PUBLISH" -eq 1 ]]; then
    do_publish
  fi
}

main "$@"
