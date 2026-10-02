---
id: task-26
title: "Build the content-addressed store and garbage collection"
status: To Do
priority: High
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30'
labels:
  - rust
  - typescript
  - cas
  - caching
dependencies:
  - TASK-13
  - TASK-16
milestone: m-3
type: feature
---
# Build the content-addressed store and garbage collection

## Description

This task is derived from the project knowledge base and scheduled in m-3. It is the executable backlog representation of the referenced design and roadmap material.

## Acceptance Criteria

- [ ] Digests include path and content and reuse the Hasher interface with chunked I/O.
- [ ] GC takes the file lock and preserves referenced blobs across interrupted operations.

## References

- [.knowledge/features/pluggable-patterns.md](../../.knowledge/features/pluggable-patterns.md)

## Definition of Done

- [ ] Implementation, tests, and documentation are complete.
- [ ] Related knowledge-base design remains accurate after implementation.
