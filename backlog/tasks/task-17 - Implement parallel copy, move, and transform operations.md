---
id: task-17
title: 'Implement parallel copy, move, and transform operations'
status: Done
assignee:
  - '@me'
created_date: '2026-09-30'
updated_date: '2026-10-04 18:45'
labels:
  - rust
  - typescript
  - parallel-io
milestone: m-2
dependencies:
  - TASK-2
priority: medium
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
This task is derived from the project knowledge base and scheduled in m-2. It is the executable backlog representation of the referenced design and roadmap material.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Parallel copy of 50k files meets the documented comparison target.
- [x] #2 Transformer execution order and failure/error aggregation are tested.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Implementation, tests, and documentation are complete.
- [x] #2 Related knowledge-base design remains accurate after implementation.
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Implement Path.copyTo, Path.moveTo, and Path.transform test-first. Use a native recursive copy fast path for unfiltered trees, a bounded worker pool for filtered/symlink-aware operations, ordered text transformers with per-file error aggregation, cross-device move fallback, and a reproducible 50k-file benchmark.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Path.copyTo supports root-relative glob filters, directory exclusions, explicit concurrency, symlink preservation by default, and optional symlink following. Unfiltered copies use Node’s native recursive copy path and report counts; filtered copies use a bounded worker pool. Path.moveTo uses rename and falls back to copy-then-remove across filesystems. Path.transform applies one callback or an ordered callback list left to right, continuing other files when one file fails and returning per-file errors. Documentation and a benchmark harness were added.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Delivered TASK-17: tested copy, move, and transform APIs, symlink policy coverage, bounded bulk execution, ordered transformer/error aggregation, docs, and a 50k-file benchmark. The benchmark measured 4.37x over the sequential Node baseline against a 3x target. Full repository gates pass.
<!-- SECTION:FINAL_SUMMARY:END -->

# Implement parallel copy, move, and transform operations

## References

- [backlog/docs/phase-plan.md](../docs/phase-plan.md)
