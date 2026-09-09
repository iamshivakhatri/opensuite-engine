# OpenSuite Engine — Agent Instructions

OpenSuite Engine is a long-term, open-source Rust systems project for safely
inspecting, modifying, validating, rendering, diffing, and versioning Office
documents for AI agents.

DOCX is the first target. PPTX and XLSX come later.

This is not a prototype. Correctness, preservation, performance, safety, and
architectural quality are more important than feature count or speed of delivery.

## Read Before Editing

Before making meaningful changes, read:

- `docs/VISION.md` — what OpenSuite is trying to become
- `docs/ARCHITECTURE.md` — architectural boundaries and invariants
- `docs/ENGINEERING.md` — implementation and verification rules
- `docs/STATUS.md` — what actually exists today
- `docs/docx-engine.md` — DOCX product roadmap and stopping rule for DOCX work

Do not infer architecture from incomplete code when these documents define it.

## Core Rules

1. Preserve unknown OOXML whenever possible.
2. Do not regenerate untouched document content unnecessarily.
3. Original source representation and semantic representation are different layers.
4. ZIP, OPC, XML/source representation, DOCX semantics, operations, rendering,
   and agent interaction must remain distinct concepts.
5. AI-facing mutations must eventually use typed, deterministic operations.
6. Operations must support structured diagnostics and safe failure.
7. Rendering is separate from editing.
8. Visual inspection is a first-class future capability.
9. Parse lazily and perform minimal work where practical.
10. Optimize using measurement, not assumptions.
11. Prefer explicit, boring correctness over clever abstractions.
12. Do not introduce infrastructure or dependencies without a concrete need.

## Architecture Authority

Do NOT independently modify architectural direction.

In particular, do not silently:

- introduce new major layers
- merge architectural layers
- introduce a framework
- add a database
- add networking to the core
- introduce async runtime requirements
- redesign operation semantics
- change preservation strategy
- change public protocol direction
- create new crates merely for organizational convenience

If implementation exposes an architectural question, stop and report it.

`docs/VISION.md` and `docs/ARCHITECTURE.md` are architecture-owner documents.
Modify them only when explicitly instructed.

## Documentation Maintenance

After implementation changes:

- update `docs/STATUS.md` when capabilities or milestone progress change
- update `docs/ENGINEERING.md` when build/test/development commands genuinely change
- do not duplicate documentation unnecessarily
- do not create new markdown files unless there is a clear long-term purpose
- keep documentation concise and current
- remove stale statements rather than appending contradictory notes

Code and documentation must agree.

## Working Style

Before substantial implementation:

1. inspect relevant code and documentation
2. state the small implementation goal
3. identify affected files
4. identify architectural uncertainty, if any
5. implement the smallest correct change
6. format, lint, test, and verify
7. summarize what changed and what remains unsupported

Do not implement adjacent features merely because they are convenient.

## Current Scope

We are currently building the DOCX foundation.

Do not implement PPTX, XLSX, cloud infrastructure, application authentication,
billing, databases, Python bindings, Node bindings, or distributed systems unless
explicitly requested.
