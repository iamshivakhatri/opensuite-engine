#!/usr/bin/env bash
# Prepare (and optionally tag/push) an @opensuitehq/engine release.
# GitHub Actions publish-engine.yml remains the only npm publisher.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE_DIR="$ROOT/crates/opensuite-node"
PACKAGE_NAME="@opensuitehq/engine"
VERSION=""
DO_PUBLISH=0
RELEASE_PATHS=(
  Cargo.lock
  crates/opensuite-node/package.json
  crates/opensuite-node/package-lock.json
  crates/opensuite-node/Cargo.toml
  crates/opensuite-node/index.js
)

usage() {
  cat <<'EOF'
Usage:
  ./scripts/release-engine.sh X.Y.Z           # prepare + verify (no commit/tag/push)
  ./scripts/release-engine.sh X.Y.Z --publish # prepare + verify + commit + tag + push

npm publication is performed only by .github/workflows/publish-engine.yml
after the vX.Y.Z tag is pushed. This script never runs npm publish.
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

info() {
  echo "==> $*"
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
  [[ -n "$VERSION" ]] || die "version X.Y.Z is required"
}

validate_semver() {
  [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
    || die "version must be X.Y.Z (digits only), got: $VERSION"
}

require_clean_worktree() {
  if [[ -n "$(git -C "$ROOT" status --porcelain)" ]]; then
    die "git worktree is dirty; commit or stash changes before releasing"
  fi
}

require_tag_absent() {
  local tag="v${VERSION}"
  if git -C "$ROOT" rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
    die "tag ${tag} already exists locally"
  fi
  git -C "$ROOT" fetch --tags --quiet origin 2>/dev/null || true
  if git -C "$ROOT" ls-remote --exit-code --tags origin "refs/tags/${tag}" >/dev/null 2>&1; then
    die "tag ${tag} already exists on origin"
  fi
}

require_npm_version_absent() {
  local status
  status="$(npm view "${PACKAGE_NAME}@${VERSION}" version 2>/dev/null || true)"
  if [[ "$status" == "$VERSION" ]]; then
    die "${PACKAGE_NAME}@${VERSION} already exists on npm"
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

assert_no_optional_deps() {
  node -e '
    const pkg = require(process.argv[1]);
    if (pkg.optionalDependencies && Object.keys(pkg.optionalDependencies).length) {
      console.error("package.json must not commit optionalDependencies");
      process.exit(1);
    }
  ' "$NODE_DIR/package.json"
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
    const updated = text.replace(/^version\s*=\s*"[^"]+"/m, `version = "${version}"`);
    if (updated === text) {
      console.error("failed to update version in", path);
      process.exit(1);
    }
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
  info "verifying version alignment"
  local pkg cargo lock loader
  pkg="$(node_pkg_version)"
  cargo="$(cargo_pkg_version)"
  lock="$(lock_pkg_version)"
  loader="$(loader_version)"
  [[ "$pkg" == "$VERSION" ]] || die "package.json version is ${pkg}, expected ${VERSION}"
  [[ "$cargo" == "$VERSION" ]] || die "opensuite-node Cargo.toml version is ${cargo}, expected ${VERSION}"
  [[ "$lock" == "$VERSION" ]] || die "package-lock.json version is ${lock}, expected ${VERSION}"
  [[ "$loader" == "$VERSION" ]] || die "index.js loader version is ${loader}, expected ${VERSION}"
  assert_no_optional_deps
}

run_checks() {
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

  info "assemble + pack dry-run (no npm publish)"
  (
    cd "$NODE_DIR"
    if [[ ! -d artifacts ]]; then
      die "missing ${NODE_DIR}/artifacts — download a Publish engine dry-run or place platform bindings there"
    fi
    npm run create-npm-dirs
    npm run artifacts -- --output-dir artifacts --npm-dir npm
    npm run pack:dry-run
    assert_no_optional_deps
  )
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
  if [[ "$DO_PUBLISH" -eq 0 ]]; then
    cat <<EOF
Dry run complete (no commit/tag/push).
Working tree now contains the prepared ${VERSION} bump for review.
To discard and publish for real from a clean tree:
  git reset --hard HEAD
  ./scripts/release-engine.sh ${VERSION} --publish
EOF
  fi
}

do_publish() {
  local tag="v${VERSION}"
  assert_only_release_paths_dirty

  info "creating release commit"
  (
    cd "$ROOT"
    git add -- "${RELEASE_PATHS[@]}"
    git commit -m "$(cat <<EOF
release: prepare ${VERSION}

EOF
)"
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
EOF
}

main() {
  parse_args "$@"
  validate_semver

  require_cmd git
  require_cmd node
  require_cmd npm
  require_cmd cargo
  require_cmd npx

  cd "$ROOT"
  require_clean_worktree
  require_tag_absent
  require_npm_version_absent

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
