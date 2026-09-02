# OpenSuite Engine Status

## Current Phase

Foundation — Week 1: OPC package discovery

## Current Format

DOCX

PPTX and XLSX have not started.

## Current Objective

Establish the shared OPC package discovery layer before implementing DOCX
semantics.

## Implemented

- Minimal Rust workspace, CI, and native CLI.
- Read-only OPC package opening with safe ZIP entry indexing.
- `[Content_Types].xml` Default and Override content type resolution.
- Package-level `_rels/.rels` parsing and main office document discovery through
  Transitional or Strict `officeDocument` relationships.
- `opensuite inspect <path-to-office-file>` JSON package metadata output.

## Current Repository Target

- `opensuite-opc`
- `opensuite-docx`
- `opensuite-protocol`
- `opensuite` CLI binary

## Explicitly Not Implemented Yet

- DOCX parsing
- document mutation
- transactions
- rendering
- server
- agent runtime
- Node bindings
- Python bindings
- PPTX
- XLSX

## Next Milestone

Build the next explicitly selected OPC capability without starting DOCX
semantic parsing.
