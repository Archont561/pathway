---
id: task-18
title: "Add symlink policy and atomic-write hardening"
status: To Do
priority: High
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30'
labels:
  - rust
  - security
  - filesystem
dependencies:
  - TASK-2
  - TASK-10
milestone: m-2
type: task
---
# Add symlink policy and atomic-write hardening

## Description

This task is derived from the project knowledge base and scheduled in m-2. It is the executable backlog representation of the referenced design and roadmap material.

## Acceptance Criteria

- [ ] Symlink behavior is covered for walking, copying, and sandbox containment.
- [ ] O_EXCL temp creation and best-effort directory fsync after rename are implemented and tested.

## References

- [.knowledge/implementation/phase-plan.md](.knowledge/implementation/phase-plan.md)

## Definition of Done

- [ ] Implementation, tests, and documentation are complete.
- [ ] Related knowledge-base design remains accurate after implementation.
