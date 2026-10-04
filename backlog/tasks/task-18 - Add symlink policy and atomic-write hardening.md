---
id: task-18
title: Add symlink policy and atomic-write hardening
status: In Progress
assignee:
  - '@me'
created_date: '2026-09-30'
updated_date: '2026-10-04 18:20'
labels:
  - rust
  - security
  - filesystem
milestone: m-2
dependencies:
  - TASK-2
  - TASK-10
priority: high
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
This task is derived from the project knowledge base and scheduled in m-2. It is the executable backlog representation of the referenced design and roadmap material.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Symlink behavior is covered for walking, copying, and sandbox containment.
- [x] #2 O_EXCL temp creation and best-effort directory fsync after rename are implemented and tested.
<!-- AC:END -->

# Add symlink policy and atomic-write hardening

## References

- [backlog/docs/phase-plan.md](../docs/phase-plan.md)

## Definition of Done

- [ ] Implementation, tests, and documentation are complete.
- [ ] Related knowledge-base design remains accurate after implementation.

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Implement the Rust atomic-write primitive test-first, add symlink regression coverage for the existing walker policy, then update documentation and verify all gates. Sandbox and copy APIs belong to TASK-15 and TASK-17 because they do not yet exist in this repository.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Rust atomic writes now use tempfile exclusive creation in the target directory, sync the file before persist/rename, best-effort sync the parent directory, and clean up failed renames.

Regression coverage covers replacement, failed rename cleanup, replacing a symlink without modifying its target, and the existing walker no-follow symlink policy.

The copy and sandbox portions of AC #1 remain owned by TASK-17 and TASK-15 because those public APIs are not implemented yet.
<!-- SECTION:NOTES:END -->
