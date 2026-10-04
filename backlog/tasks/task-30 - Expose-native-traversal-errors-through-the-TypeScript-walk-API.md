---
id: TASK-30
title: Expose native traversal errors through the TypeScript walk API
status: To Do
assignee: []
created_date: '2026-10-04 18:16'
labels:
  - rust
  - typescript
  - api
  - testing
dependencies:
  - TASK-3
priority: medium
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust scanner collects traversal failures and the N-API Walker exposes errors(), but packages/path/src/walk.ts never reads or surfaces that data. Define the public TypeScript error-reporting contract, wire the boundary, and add an integration regression test so unreadable or otherwise failed traversal entries are not silently lost.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The public TypeScript walk contract documents how aggregate traversal errors are surfaced.
- [ ] #2 Native Walker.errors() is consumed or replaced by an equivalent boundary-safe mechanism, with no traversal error silently discarded.
- [ ] #3 A test exercises the public TypeScript walk seam and verifies the error behavior.
- [ ] #4 The native and TypeScript behavior is documented consistently.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Implementation and public-seam tests are complete.
- [ ] #2 The TDD and refactor skills were followed, with behavior changes separated from refactoring.
<!-- DOD:END -->
