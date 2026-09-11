# OpenSuite Engine — Agent Instructions

OpenSuite Engine is a preservation-first Rust engine for inspecting, modifying,
validating, and eventually rendering Office documents for AI agents. DOCX is the
only active format. PPTX and XLSX begin only after DOCX V1 is frozen.

## Read First

Before meaningful work, read:

- `docs/VISION.md`
- `docs/ARCHITECTURE.md`
- `docs/ENGINEERING.md`
- `docs/status.md`
- `docs/docx-engine.md`

The current code and worktree are the source of truth for implementation
details. Do not infer architecture from one incomplete file.

## DOCX Rules

- Preserve unknown OOXML and untouched package parts whenever possible.
- Keep source representation separate from semantic representation.
- Use typed, deterministic semantic operations; do not add generic XML editing.
- Keep parsing, mutation, verification, rendering, and protocol concerns separate.
- Return structured diagnostics and fail safely.
- Mutation code belongs in `crates/opensuite-docx/src/mutation/`, organized by
  document responsibility. `mutation.rs` is a thin facade for modules,
  re-exports, and genuinely shared helpers.
- Keep modules cohesive. Do not split a module only because it has many lines.
- Keep mutation tests with their matching domain.
- Every meaningful DOCX capability needs focused verification and Node/N-API
  exposure.
- Keep every feature in one cohesive domain module from the start. Facades hold
  declarations and re-exports, not feature code; tests stay with their domain.
  Add shared helpers only when they are genuinely shared, never as catch-all
  modules. If a file starts mixing responsibilities or approaches 1,000 lines,
  reassess ownership before adding more; line count alone is not a reason to
  split cohesive code.

## Change Discipline

- Prefer the smallest correct change. Do not add frameworks, infrastructure,
  dependencies, or broad abstractions without a concrete need.
- Do not change the architecture, preservation strategy, public protocol, or
  operation semantics without explicit direction.
- Do not add databases, networking to the core, async runtime requirements,
  cloud services, or non-DOCX format work unless explicitly requested.
- Update `docs/status.md` when present capability changes, and update
  `docs/docx-engine.md` when product capability or DOCX V1 direction changes.
- Run formatting, compilation, linting, and relevant tests before completion.
