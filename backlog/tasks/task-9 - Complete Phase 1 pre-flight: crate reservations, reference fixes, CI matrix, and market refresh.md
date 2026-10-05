---
id: task-9
title: "Complete Phase 1 pre-flight: crate reservations, reference fixes, CI matrix, and market refresh"
status: In Progress
priority: High
assignee: []
created_date: '2026-09-30'
updated_date: '2026-10-05'
labels:
  - foundation
  - release
dependencies:
milestone: m-0
type: task
---
# Complete Phase 1 pre-flight: crate reservations, reference fixes, CI matrix, and market refresh

## Description

This task is derived from the project knowledge base and scheduled in m-0. It is the executable backlog representation of the referenced design and roadmap material.

## Acceptance Criteria

- [x] Reserve pathway-fs and pathway-fs-core on crates.io and document the engine publication policy.
- [ ] Refresh Node 24/26 and Bun 1.3/1.4 CI coverage, install smoke tests, reference-code fixes, and stale market data.

## References

- [backlog/docs/phase-plan.md](../docs/phase-plan.md)

## Definition of Done

- [ ] Implementation, tests, and documentation are complete.
- [ ] Related knowledge-base design remains accurate after implementation.

## Implementation Notes

2026-10-05: crates.io pages confirm both names are reserved; each page shows one published version. `pathway-fs`: https://crates.io/crates/pathway-fs; `pathway-fs-core`: https://crates.io/crates/pathway-fs-core. The pages describe `pathway-fs` as the Phase 1 scaffold and `pathway-fs-core` as the NAPI-free Phase 1 engine. AC#2 remains open.
