# OpenSuite Engine Release

Canonical release guide for `@opensuitehq/engine`. Agents and humans should follow
this document; do not invent parallel procedures.

## Source of truth

| Item | Location |
| --- | --- |
| Release script | `scripts/release-engine.sh` |
| npm publisher | `.github/workflows/publish-engine.yml` (only place that runs `npm publish`) |
| Package name | `@opensuitehq/engine` |
| Release branch | `main` |
| Trigger | Push annotated tag `vX.Y.Z` |

The script prepares versions and may push the tag. It never runs `npm publish`.

## Before release

1. Engine is feature-complete/hardened for the documented DOCX V1 scope.
2. Work from a clean `main` worktree.
3. npm Trusted Publishing is configured for the org (one-time; see below).
4. Choose the next semver `X.Y.Z` (do not reuse an immutable published version).
5. Run preflight:

```bash
./scripts/release-engine.sh --check
```

6. Optionally assert the repo is already at a prepared version:

```bash
./scripts/release-engine.sh --check X.Y.Z
```

## Version locations

### A. Authoritative release version (must move together)

These are updated by `./scripts/release-engine.sh X.Y.Z`:

| Location | Purpose | Updated by script? |
| --- | --- | --- |
| `crates/opensuite-node/package.json` → `version` | npm root package version | yes |
| `crates/opensuite-node/Cargo.toml` → `package.version` | native crate version (must match npm) | yes |
| `crates/opensuite-node/package-lock.json` → root `version` + `packages[""].version` | lockfile root version only | yes |
| `crates/opensuite-node/index.js` → `bindingPackageVersion` checks | generated loader version pins | yes (`npm run generate-loader`) |
| `Cargo.lock` → `opensuite-node` package version | workspace lock alignment | yes (`cargo update -p opensuite-node`) |

### B. Release-time only (not committed)

| Location | Purpose | Updated by script? |
| --- | --- | --- |
| `crates/opensuite-node/npm/*/` platform manifests | `@opensuitehq/engine-<platform>` packages | no (CI / `napi` assembly) |
| Root `optionalDependencies` during pack | exact same-version platform pins | no (`napi prepublish` in CI; restored after local `pack:dry-run`) |

Committed `package.json` must **not** contain `optionalDependencies` (bootstrap /
`npm ci` would break before platform packages exist).

### C. Intentionally independent (do not bump for engine npm releases)

| Location | Purpose |
| --- | --- |
| `crates/opensuite-docx/Cargo.toml` `version` | internal Rust crate |
| `crates/opensuite-opc/Cargo.toml` `version` | internal Rust crate |
| `crates/opensuite-protocol/Cargo.toml` `version` | internal Rust crate |
| `crates/opensuite-pptx/Cargo.toml` `version` | inactive format crate |
| `bins/opensuite/Cargo.toml` `version` | CLI binary |
| Root `Cargo.toml` | workspace metadata only (no package version) |

### D. Historical docs (do not rewrite for a release)

Examples: older pins mentioned in `docs/status.md` history, E1 notes that record
the version at the time of a feature. Leave them alone unless the doc's purpose
is the current pin.

## Release command

Prepare + verify only (no commit/tag/push; never npm publish):

```bash
./scripts/release-engine.sh X.Y.Z
# or explicitly:
./scripts/release-engine.sh X.Y.Z --dry-run
```

Prepare + verify + commit + annotated tag + push (triggers CI publish):

```bash
./scripts/release-engine.sh X.Y.Z --publish
```

Help:

```bash
./scripts/release-engine.sh --help
```

## What the script does

1. Validate semver `X.Y.Z` and require clean `main`.
2. Refuse if local/remote tag `vX.Y.Z` exists or npm already has that version.
3. Bump the authoritative Node release version locations and regenerate `index.js`.
4. Verify all those locations agree.
5. Run `cargo fmt` / `check` / `clippy` / `test` for release-relevant crates.
6. Run `npm ci`, native build, and Node tests in `crates/opensuite-node`.
7. Run a **local packaged-consumer smoke test** (pack root JS, install in `/tmp`,
   copy host `.node`, create/inspect/mutate a blank DOCX).
8. If all five CI binding artifacts are already under `artifacts/`, run full
   six-package `pack:dry-run`; otherwise skip and rely on CI.
9. Print the release plan.
10. With `--publish` only: commit allowed paths, tag `vX.Y.Z`, push commit + tag.

## Native packages / targets

Configured in `crates/opensuite-node/package.json` → `napi.targets` and built by
`publish-engine.yml`:

| Target triple | npm package | Artifact |
| --- | --- | --- |
| `aarch64-apple-darwin` | `@opensuitehq/engine-darwin-arm64` | `opensuite_node.darwin-arm64.node` |
| `x86_64-apple-darwin` | `@opensuitehq/engine-darwin-x64` | `opensuite_node.darwin-x64.node` |
| `x86_64-unknown-linux-gnu` | `@opensuitehq/engine-linux-x64-gnu` | `opensuite_node.linux-x64-gnu.node` |
| `aarch64-unknown-linux-gnu` | `@opensuitehq/engine-linux-arm64-gnu` | `opensuite_node.linux-arm64-gnu.node` |
| `x86_64-pc-windows-msvc` | `@opensuitehq/engine-win32-x64-msvc` | `opensuite_node.win32-x64-msvc.node` |

Local Mac/Linux/Windows development builds only the host binding. Cross-platform
release binaries are produced in CI, not by a single local machine.

## Publish order

CI publishes **platform packages first**, then the root package:

1. `@opensuitehq/engine-darwin-arm64@X.Y.Z`
2. `@opensuitehq/engine-darwin-x64@X.Y.Z`
3. `@opensuitehq/engine-linux-x64-gnu@X.Y.Z`
4. `@opensuitehq/engine-linux-arm64-gnu@X.Y.Z`
5. `@opensuitehq/engine-win32-x64-msvc@X.Y.Z`
6. `@opensuitehq/engine@X.Y.Z` (with `optionalDependencies` injected; `--ignore-scripts`)

Exact directory iteration order comes from `npm/*/` after `napi create-npm-dirs`
+ artifact assembly; platforms always precede the root publish step in the
workflow.

## During release

Watch for:

- Script phase banners: `PREPARE` → `BUILD/VERIFY` → optional `PUBLISH`
- Version alignment errors
- Test failures
- Local consumer smoke failure
- After `--publish`: GitHub Actions **Publish engine** run for tag `vX.Y.Z`

Optional CI-only dry-run (no tag): Actions → Publish engine → `dry_run=true`
(default) builds/packs tarballs without publishing.

## After publish

1. Confirm workflow green on the tag.
2. Query registry:

```bash
npm view @opensuitehq/engine version
npm view @opensuitehq/engine@X.Y.Z optionalDependencies
```

3. Clean install smoke (outside the repo):

```bash
tmpdir="$(mktemp -d)"
cd "$tmpdir"
npm init -y
npm install @opensuitehq/engine@X.Y.Z
node -e "const e=require('@opensuitehq/engine'); console.log(typeof e.createBlankDocx)"
```

4. In the **opensuite** app repo, bump pins (see below). Do not leave production
   acceptance on `OPENSUITE_ENGINE_PATH`.

## OpenSuite app follow-up

After engine publish, update the separate `opensuite` repo in the same release
window:

1. `packages/engine-client/package.json` — exact `@opensuitehq/engine` version
2. `pnpm-lock.yaml` — refresh; confirm Linux `x64-gnu` and `arm64-gnu` optional
   binaries appear (a macOS-generated lockfile can omit them)
3. `pnpm-workspace.yaml` — `minimumReleaseAgeExclude` for `@opensuitehq/engine`
   and each platform package at the new version
4. Version pins/comments in `Dockerfile`, `docker-compose.atlas.yml`,
   `docs/deploy.md`, `docs/engine_integration.md`, and any status/agent docs that
   state the pin
5. Build the API Docker image for the target Linux architecture and require the
   Dockerfile engine-load check to pass before deploy

Also see `AGENTS.md` → Publishing / version tags.

## Failure / partial publish

| Situation | What to do |
| --- | --- |
| Platform package published, later package failed | Do **not** republish the same version for successful packages. Fix the failure, then either finish remaining packages from a repaired CI job or cut `X.Y.Z+1` if the root never published cleanly. npm versions are immutable. |
| Root package failed after platforms | Platforms at `X.Y.Z` may already be public. Prefer fixing and publishing only the root if policy allows a re-run of that step; otherwise bump. |
| Version already exists | Choose a new semver; never overwrite. |
| Registry propagation delay | Retry `npm view` / clean install with short backoff; do not assume install failure means publish failed. |
| Local prepare left a dirty bump | `git reset --hard HEAD` from a review-only dry-run, then re-run from a clean tree. |

## One-time npm setup

1. Publish under npm org `opensuitehq` (`@opensuitehq/*`).
2. Configure Trusted Publishing on each package:
   - GitHub owner: `iamshivakhatri`
   - Repository: `opensuite-engine`
   - Workflow: `publish-engine.yml`
3. No long-lived `NPM_TOKEN` is required for CI (OIDC only).

## Do not

- Republish the same immutable version
- Manually patch a registry package
- Publish from a dirty or unreviewed worktree
- Run `npm publish` from a laptop (CI only)
- Skip the packaged-consumer smoke path the script runs
- Manually edit five version files when the script can bump them
- Bump unrelated internal crate versions “for consistency”
