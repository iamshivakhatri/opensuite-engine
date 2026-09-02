# OpenSuite Engine Architecture

## Architectural Direction

The conceptual system is:

AI / SDK / CLI
    ↓
Typed Operations
    ↓
Safety / Transaction Layer
    ↓
Semantic Document Model
    ↓
Source-Aware Preservation Layer
    ↓
Format Engine
    ↓
Shared OOXML / OPC Core
    ↓
ZIP Package

Rendering and visual inspection exist beside this pipeline, not inside the
document mutation architecture.

## Important Distinctions

These concepts must not collapse into one abstraction:

- ZIP archive
- OPC package
- OPC part
- relationship
- source XML
- parsed source representation
- semantic document model
- effective formatting
- operation
- patch
- transaction
- serialization
- validation
- rendering

## Preservation

The engine should retain both:

1. semantic understanding
2. original/source representation

Semantic objects should be able to refer back to their source representation.

Unknown or unsupported OOXML should normally survive operations untouched.

Untouched package parts should not be regenerated merely because a document was opened.

## Agent Execution

Agents should eventually interact through typed operations rather than unrestricted
OOXML mutation.

Conceptually:

intent
  ↓
typed operation
  ↓
target resolution
  ↓
preconditions
  ↓
patch plan
  ↓
transaction
  ↓
apply
  ↓
validate
  ↓
render / inspect
  ↓
commit or rollback

Failures should return structured, machine-readable diagnostics that help an
agent understand what failed and whether anything changed.

## Visual Feedback

Documents are visual artifacts.

The architecture must allow efficient rendering and inspection of affected pages,
slides, sheets, or regions so vision-capable agents can evaluate their own work.

Rendering remains replaceable and independent from the core editing engine.

## Performance

Rust should be used deliberately for:

- efficient representations
- predictable memory use
- lazy parsing
- minimal allocation where beneficial
- streaming where appropriate
- efficient indexes
- minimal serialization
- concurrency only where measurements justify it

Performance must be benchmarked rather than assumed.