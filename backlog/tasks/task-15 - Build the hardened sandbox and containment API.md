---
id: task-15
title: "Build the hardened sandbox and containment API"
status: To Do
priority: High
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30'
labels:
  - rust
  - typescript
  - security
  - sandbox
dependencies:
  - TASK-10
milestone: m-2
type: feature
---
# Build the hardened sandbox and containment API

## Description

This task is derived from the project knowledge base and scheduled in m-2. It is the executable backlog representation of the referenced design and roadmap material.

## Acceptance Criteria

- [ ] Traversal escapes, intermediate symlinks, loops, case-insensitive paths, Unicode normalization, and prefix collisions are blocked.
- [ ] openat/O_NOFOLLOW or equivalent hardening and TOCTOU limitations are documented.

## References

- [.knowledge/features/killer-features.md](.knowledge/features/killer-features.md)

## Definition of Done

- [ ] Implementation, tests, and documentation are complete.
- [ ] Related knowledge-base design remains accurate after implementation.
