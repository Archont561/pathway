---
id: task-18
title: Add symlink policy and atomic-write hardening
status: Done
assignee:
  - '@me'
created_date: '2026-09-30'
updated_date: '2026-10-04 18:43'
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
- [x] #1 Symlink behavior is covered for walking, copying, and sandbox containment.
- [x] #2 O_EXCL temp creation and best-effort directory fsync after rename are implemented and tested.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Implementation, tests, and documentation are complete.
- [x] #2 Related knowledge-base design remains accurate after implementation.
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Keep symlink policy consistent across the walker, copy operations, and sandbox containment; retain the existing atomic-write hardening and verify all gates.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Atomic writes use exclusive same-directory temporary files, file sync, rename, best-effort parent-directory sync, and cleanup on failure. Walker tests verify no-follow traversal. TASK-15 now supplies sandbox containment tests, while TASK-17 adds copy tests that preserve symlinks by default and supports an explicit followSymlinks option.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Completed symlink and atomic-write hardening across walking, sandbox containment, and copy operations. Full repository gates pass.
<!-- SECTION:FINAL_SUMMARY:END -->

# Add symlink policy and atomic-write hardening

## References

- [backlog/docs/phase-plan.md](../docs/phase-plan.md)
