# Publishing `@opensuitehq/engine` (N-API)

Canonical release procedure: [`docs/releasing.md`](../../docs/releasing.md).

Quick commands from repo root:

```bash
./scripts/release-engine.sh --check
./scripts/release-engine.sh X.Y.Z --dry-run
./scripts/release-engine.sh X.Y.Z --publish
```

This package directory never runs `npm publish` for releases.
`.github/workflows/publish-engine.yml` is the only npm publisher.
