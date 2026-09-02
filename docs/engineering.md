# Engineering Workflow

## Development Standard

Every change should be:

- small
- reviewable
- tested
- measurable where performance matters
- consistent with documented architecture

Avoid speculative abstractions.

## Verification

Before considering implementation complete, run the project's canonical:

- formatter
- compiler/build
- linter
- unit/integration tests

As the project grows, this document will contain the exact canonical commands.

CI and local verification should agree.

## Testing Philosophy

Office files should be treated similarly to compiler inputs.

Over time the project should use:

- unit tests
- deterministic fixtures
- round-trip tests
- golden tests
- malformed-input tests
- property tests
- fuzzing
- cross-producer documents
- benchmarks
- visual regression tests

A particularly important invariant is the no-op round trip:

input document
  ↓
OpenSuite performs no mutation
  ↓
output

OpenSuite should preserve the input as faithfully as realistically possible.

## Dependencies

Dependencies require justification.

Prefer small, mature components for mechanical problems while keeping OpenSuite's
core architecture under our control.

Do not introduce infrastructure complexity to solve engine-design problems.