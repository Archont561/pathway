---
id: task-11
title: "Finalize Phase 1 cross-platform API and release gates"
status: Done
priority: High
assignee: []
created_date: '2026-09-30'
updated_date: '2026-10-06'
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

- [x] Node 24, Node 26, Bun 1.3, and Bun 1.4 test jobs pass on Linux, macOS, and Windows targets.
- [x] NAPI platform package installation, Node-API floor, docs, and smoke tests pass.

## References

- [.knowledge/implementation/ci-distribution.md](../../.knowledge/implementation/ci-distribution.md)

## Definition of Done

- [x] Implementation, tests, and documentation are complete.
- [x] Related knowledge-base design remains accurate after implementation.

## Implementation Notes

2026-10-06: GitHub runtime matrix run [37382716742](https://github.com/Archont561/pathway/actions/runs/37382716742) on main commit `70bb89d` is green for all 12 runtime jobs: Node 24, Node 26, Bun 1.3.11, and Bun 1.4.2 on Ubuntu, macOS, and Windows. The same run is green for all three packed-install smoke jobs (`ubuntu-latest`, `macos-latest`, `windows-latest`), each of which builds the native addon, builds the TypeScript package, packs `packages/path`, runs `npm publish --dry-run --provenance`, installs the tarball into a clean prefix, imports `@archont561/pathway`, checks `engineAvailable()`, and exercises a native-backed walk.

2026-10-06: the post-merge main runs for the same commit are green: `ci` run [37382716586](https://github.com/Archont561/pathway/actions/runs/37382716586), `docs` run [37382876688](https://github.com/Archont561/pathway/actions/runs/37382876688), and `publish sandbox` run [37382716767](https://github.com/Archont561/pathway/actions/runs/37382716767). The Node-API floor is enforced by the loader (`packages/path/src/binding.ts`) comparing `process.versions.napi` with the native `napiVersion()` export, while `packages/path/package.json` declares `engines.node >=24` and the smoke jobs prove the current single-package `dist/` artifact loads on each supported OS. Optional platform-package publication remains future release-work and is not claimed here.

## Final Summary

TASK-11 is complete: the Phase 1 runtime matrix, packed-install smoke tests, Node-API loader floor, docs workflow, and repository CI are green with recorded run evidence. The task record, README, phase plan, docs, and CI distribution knowledge base now distinguish the verified current single-package distribution path from future platform-package publication.
