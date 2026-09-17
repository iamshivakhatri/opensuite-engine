# Local notes for publishing @opensuite/engine (N-API).

## One-time setup

1. Create npm org/user that can publish `@opensuite/*` (or change the package name).
2. Add GitHub Actions secret `NPM_TOKEN` (automation token) on this repo.
3. Tag a release: `git tag v0.1.0 && git push origin v0.1.0`
   — or run workflow **Publish engine** via `workflow_dispatch`.

## Local build

```bash
cd crates/opensuite-node
npm ci
npm run build
```

Produces `opensuite_node.<platform>.node` next to `index.js` for local `link:` / pnpm override consumers.
