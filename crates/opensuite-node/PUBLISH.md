# Publishing `@opensuitehq/engine` (N-API)

## Normal release path

From a clean worktree at repo root:

```bash
# Prepare + verify only (no commit/tag/push)
./scripts/release-engine.sh X.Y.Z

# Prepare + verify + commit + annotated tag + push
# (tag push triggers GitHub Actions npm publish)
./scripts/release-engine.sh X.Y.Z --publish
```

The script bumps only the Node package versions, regenerates `index.js`, runs
checks, and (with `--publish`) pushes `vX.Y.Z`. It never runs `npm publish`.

`.github/workflows/publish-engine.yml` remains the only npm publisher: it builds
the five platform binaries, publishes platform packages first, then the root
package via npm Trusted Publishing (OIDC).

After the workflow succeeds, install in the main `opensuite` repo:

```text
@opensuitehq/engine@X.Y.Z
```

## One-time setup

1. Publish under the npm org `opensuitehq` (scope `@opensuitehq/*`).
2. Configure **Trusted Publishing** on npm for each package, pointing at:
   - GitHub owner: `iamshivakhatri`
   - Repository: `opensuite-engine`
   - Workflow: `publish-engine.yml`
3. No long-lived `NPM_TOKEN` is required for CI publishes (OIDC only).

## Local build (development)

```bash
cd crates/opensuite-node
npm ci
npm run build
```

Produces `opensuite_node.<platform>.node` next to `index.js` for local dogfood.

## Platform optionalDependencies (release-time only)

Committed `package.json` / `package-lock.json` do **not** list the five
`@opensuitehq/engine-*` optionalDependencies. Those packages only exist on npm
*after* a release, so locking them at the next version breaks `npm ci` (bootstrap).

`napi prepublish` injects exact same-version optionalDependencies into the root
manifest during pack/publish (`pack:dry-run` does this, then restores the
committed `package.json`). The packed/published `@opensuitehq/engine` still
declares all five platform packages.

## Manual CI dry-run (optional)

Actions → Publish engine → Run workflow (`dry_run=true`) builds and packs
tarballs without publishing. Ordinary branch pushes never publish.

## Local pack dry-run (optional)

After a green **Publish engine** build (or with all five bindings under
`crates/opensuite-node/artifacts/`):

```bash
cd crates/opensuite-node
# optional: gh run download <run-id> -D artifacts
npm run create-npm-dirs
npm run artifacts -- --output-dir artifacts --npm-dir npm
npm run pack:dry-run
```

The release script runs this pack dry-run automatically when artifacts are present.
