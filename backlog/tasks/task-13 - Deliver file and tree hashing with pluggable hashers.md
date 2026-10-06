---
id: task-13
title: "Deliver file and tree hashing with pluggable hashers"
status: Done
priority: High
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30'
labels:
  - rust
  - typescript
  - hashing
dependencies:
  - TASK-2
milestone: m-1
type: feature
---
# Deliver file and tree hashing with pluggable hashers

## Description

This task is derived from the project knowledge base and scheduled in m-1. It is the executable backlog representation of the referenced design and roadmap material.

## Acceptance Criteria

- [x] BLAKE3, xxhash, and SHA-256 support streaming 64 KB reads without whole-file loading.
- [x] Tree hashes are deterministic across runs and integrate with fused traversal.

## References

- [.knowledge/features/pluggable-patterns.md](../../.knowledge/features/pluggable-patterns.md)

## Definition of Done

- [x] Implementation, tests, and documentation are complete.
- [x] Related knowledge-base design remains accurate after implementation.

## Implementation Notes

The core exposes a streaming `Hasher` seam and 64 KiB reader, with BLAKE3,
xxhash, and SHA-256 implementations. Fused traversal provides deterministic
`hash_tree` output using versioned path/digest framing and sorted relative paths.
The N-API bridge exposes native file and byte hashing, while the TypeScript API
provides `Path.hash`, `Path.hashTree`, top-level helpers, and custom streaming
hasher fallback support.

## Final Summary

TASK-13 is complete. Native file/tree hashing is available through the Rust
core and TypeScript surface, with dedicated Rust and TypeScript coverage for
algorithm selection, chunk bounds, deterministic tree output, and content
changes.
