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

2026-10-05: added `.github/workflows/runtime-matrix.yml` with 12 native rows (Node 24/26 and Bun 1.3.11/1.4.2 on Ubuntu, macOS, and Windows) plus three OS-specific packed-install smoke jobs. The workflow builds through Bun, runs the Bun suite or Node smoke suite, packs `packages/path`, validates npm provenance metadata, and imports a clean install. `packages/path/test/node-smoke.mjs` covers Node ESM loading, normalization, and a native-backed walk. Local actionlint, Node smoke, `npm publish --dry-run --provenance`, and packed-install smoke pass; AC#2 remains open until GitHub reports the matrix green.

2026-10-05: reconciled the loader/reference docs, README, installation docs, phase plan, and CI/distribution design with the current single-package `dist/` layout. Refreshed competitive data with npm API counts for 2026-09-28 through 2026-10-04 in `.knowledge/competitive/verified-data.md` Round 3 and updated the dependent landscape/checklist records.
