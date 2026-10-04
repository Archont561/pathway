---
id: task-15
title: Build the hardened sandbox and containment API
status: Done
assignee:
  - '@me'
created_date: '2026-09-30'
updated_date: '2026-10-04 18:31'
labels:
  - rust
  - typescript
  - security
  - sandbox
milestone: m-2
dependencies:
  - TASK-10
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
This task is derived from the project knowledge base and scheduled in m-2. It is the executable backlog representation of the referenced design and roadmap material.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Traversal escapes, intermediate symlinks, loops, case-insensitive paths, Unicode normalization, and prefix collisions are blocked.
- [x] #2 openat/O_NOFOLLOW or equivalent hardening and TOCTOU limitations are documented.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Implementation, tests, and documentation are complete.
- [x] #2 Related knowledge-base design remains accurate after implementation.
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Implement the public FileSystem.sandbox seam and native containment primitive test-first. Add lexical and host-normalized containment, existing-ancestor symlink checks, Unix openat(O_NOFOLLOW) descriptor traversal, explicit TOCTOU documentation, and run all gates.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The TypeScript API adds FileSystem.sandbox(root), Sandbox, SandboxPath, and ContainmentError. SandboxPath preserves the serializer registry and confinement across resolve, join, parent, and all existing Path I/O methods; it checks lexical boundaries, prefix collisions, case and Unicode host behavior, and deepest existing ancestors with lstat/realpath before construction and I/O. The Rust core now provides a descriptor-anchored Sandbox::open_read using openat(O_NOFOLLOW) on Unix, with a documented canonicalize fallback on platforms without openat. No copy/move or atomic-write APIs were added.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Delivered TASK-15: public sandbox containment API, red/green traversal and symlink regression tests, Unix descriptor-based native primitive, docs, and updated knowledge-base design. Full repository gates pass.
<!-- SECTION:FINAL_SUMMARY:END -->

# Build the hardened sandbox and containment API

## References

- [.knowledge/features/killer-features.md](../../.knowledge/features/killer-features.md)
