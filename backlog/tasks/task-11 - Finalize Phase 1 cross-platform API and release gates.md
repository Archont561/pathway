---
id: task-11
title: "Finalize Phase 1 cross-platform API and release gates"
status: To Do
priority: High
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30'
labels:
  - ci
  - release
  - compatibility
dependencies:
  - TASK-1
  - TASK-2
  - TASK-3
  - TASK-4
milestone: m-0
type: task
---
# Finalize Phase 1 cross-platform API and release gates

## Description

This task is derived from the project knowledge base and scheduled in m-0. It is the executable backlog representation of the referenced design and roadmap material.

## Acceptance Criteria

- [ ] Node 24, Node 26, Bun 1.3, and Bun 1.4 test jobs pass on Linux, macOS, and Windows targets.
- [ ] NAPI platform package installation, Node-API floor, docs, and smoke tests pass.

## References

- [.knowledge/implementation/ci-distribution.md](../../.knowledge/implementation/ci-distribution.md)

## Definition of Done

- [ ] Implementation, tests, and documentation are complete.
- [ ] Related knowledge-base design remains accurate after implementation.
