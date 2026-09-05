# Runtime Boundary

This document defines the boundary between an application runtime and
OpenSuite Engine. It does not define product, agent, or storage behavior.

## Implemented Now

- The protocol crate exposes a versioned capability manifest and compact,
  machine-readable diagnostics.
- The CLI exposes the current manifest through `opensuite capabilities`.
- DOCX is the only declared format. Its capabilities describe implemented,
  read-only inspection only.

Source identities such as `NodeId`, XML paths, source spans, OPC relationships,
ZIP offsets, and byte positions remain engine implementation details. They are
not runtime protocol identifiers and must not be persisted by applications.

## Future Contract Requirements

The application owns immutable document versions, history, checkpoints, user
records, and storage. The engine never mutates application history in place.

```text
inspect version N
→ build typed semantic operation against N
→ execute with an opaque base-revision precondition
→ validate
→ serialize a new artifact
→ application stores the new immutable version
```

If the supplied revision does not match the application-selected current
version, execution must return a structured conflict or precondition failure.
The engine must never silently apply stale operations.

Future external operations must target semantic document concepts, never source
identities. Exact DOCX, PPTX, and XLSX target selectors will be defined with
the first mutation operations; no universal Office target type exists yet.

Operations will be typed semantic actions, not generic XML or ZIP patches. A
single operation should be atomic. Independent successful operations may
survive a later failed operation; callers must request an atomic batch
explicitly when they require all-or-nothing behavior. Future execution must
report stale, overlapping, or conflicting semantic targets without sacrificing
determinism or preservation.

Future operation results need structured status, diagnostics, affected semantic
regions, warnings, conflicts, change summaries, and an output artifact. The
engine may provide semantic change information, while the application owns
review, history, and revert workflows.

Targeted inspection must eventually support summaries, outlines, search, and
bounded semantic context. Formats remain format-specific behind shared runtime
envelopes for format, capabilities, and diagnostics.

Rendering remains separate: a future render boundary accepts an input and an
optional region, then returns rendered artifacts and metadata.
