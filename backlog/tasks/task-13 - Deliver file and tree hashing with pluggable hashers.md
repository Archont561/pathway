---
id: task-13
title: "Deliver file and tree hashing with pluggable hashers"
status: To Do
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

- [ ] BLAKE3, xxhash, and SHA-256 support streaming 64 KB reads without whole-file loading.
- [ ] Tree hashes are deterministic across runs and integrate with fused traversal.

## References

- [.knowledge/features/pluggable-patterns.md](../../.knowledge/features/pluggable-patterns.md)

## Definition of Done

- [ ] Implementation, tests, and documentation are complete.
- [ ] Related knowledge-base design remains accurate after implementation.
