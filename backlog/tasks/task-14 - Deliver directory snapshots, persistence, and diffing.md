---
id: task-14
title: "Deliver directory snapshots, persistence, and diffing"
status: To Do
priority: High
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30'
labels:
  - rust
  - typescript
  - snapshot
dependencies:
  - TASK-2
  - TASK-13
milestone: m-1
type: feature
---
# Deliver directory snapshots, persistence, and diffing

## Description

This task is derived from the project knowledge base and scheduled in m-1. It is the executable backlog representation of the referenced design and roadmap material.

## Acceptance Criteria

- [ ] 100k-file snapshot and diff meets the documented performance target.
- [ ] Nanosecond mtime precision and sorted-path folds make persisted snapshots deterministic.

## References

- [.knowledge/features/killer-features.md](.knowledge/features/killer-features.md)

## Definition of Done

- [ ] Implementation, tests, and documentation are complete.
- [ ] Related knowledge-base design remains accurate after implementation.
