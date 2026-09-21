# Local notes for publishing @opensuitehq/engine (N-API).

## One-time setup

1. Publish under the npm org `opensuitehq` (scope `@opensuitehq/*`).
2. Configure **Trusted Publishing** on npm for each package, pointing at:
   - GitHub owner: `iamshivakhatri`
   - Repository: `opensuite-engine`
   - Workflow: `publish-engine.yml`
3. No long-lived `NPM_TOKEN` is required for CI publishes (OIDC only).

## Release triggers

- **Manual dry-run (default):** Actions → Publish engine → Run workflow (`dry_run=true`).
  Builds, assembles, packs tarballs; does **not** publish.
- **Manual publish:** same workflow with `dry_run=false` (only after bumping version).
- **Tag release:** `git tag vX.Y.Z && git push origin vX.Y.Z` always publishes.

Ordinary branch pushes never publish.

## Local build

```bash
cd crates/opensuite-node
npm ci
npm run build
```

Produces `opensuite_node.<platform>.node` next to `index.js` for local `link:` / pnpm override consumers.

## Platform optionalDependencies (release-time only)

Committed `package.json` / `package-lock.json` do **not** list the five
`@opensuitehq/engine-*` optionalDependencies. Those packages only exist on npm
*after* a release, so locking them at the next version breaks `npm ci` (bootstrap).

`napi prepublish` injects exact same-version optionalDependencies into the root
manifest during pack/publish (`pack:dry-run` does this, then restores the
committed `package.json`). The packed/published `@opensuitehq/engine` still
declares all five platform packages.

## Dry-run pack (local, no publish)

After a green **Publish engine** build (or with all five `.node` artifacts under `artifacts/`):

```bash
cd crates/opensuite-node
# optional: gh run download <run-id> -D artifacts
npm run create-npm-dirs
npm run artifacts -- --output-dir artifacts --npm-dir npm
npm run pack:dry-run
```

Creates `dist-tarballs/*.tgz` for the root package and all five platform packages.
The root tarball includes injected `optionalDependencies`; the working-tree
`package.json` is restored afterward.

Local install smoke (current platform only):

```bash
TMP=$(mktemp -d)
cd "$TMP"
npm init -y
npm install \
  /path/to/opensuitehq-engine-0.1.1.tgz \
  /path/to/opensuitehq-engine-darwin-arm64-0.1.1.tgz
node -e 'require("@opensuitehq/engine").getDocxCapabilities()'
```
