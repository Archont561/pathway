---
id: task-3
title: Wire the engine walk bridge and unfreeze the TypeScript WalkIterator
status: To Do
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30 20:24'
labels:
  - engine
  - typescript
milestone: m-0
dependencies:
  - TASK-1
  - TASK-2
priority: high
---

## Description

Bridge the task-2 scanner over N-API using the transport frozen in task-1, and replace the deliberately-throwing stub in `packages/path/src/walk.ts` with the real batched `AsyncGenerator`. The boundary rule (D2) governs the whole task: bulk operations only — one boundary crossing per batch, never per entry; string manipulation stays in TypeScript.

## Acceptance Criteria

- [ ] `crates/engine` exposes the walk over the task-1 decision (`#[napi(async_iterator)]` or chunked paging via `AsyncTask`); the crate stays glue-only, with all logic in `pathway-fs-core`.
- [ ] The version guards keep their semantics: `engine_version()` and the Node-API floor, so a stale `.node` file is refused at load time.
- [ ] `walk()` in `packages/path/src/walk.ts` yields `WalkBatch`es; `walkFiles()` and `walkDirs()` delegate to it.
- [ ] `AbortSignal` (the `signal` option) wires through to the native `cancel()`.
- [ ] `packages/path` still imports without a built addon (`engineAvailable()` stays safe); runtime paths that need native fail with the existing `BUILD_HINT`.
- [ ] The package version test (`package.json` matches the workspace version) still passes.
- [ ] `pixi run typecheck` and `pixi run test` are green with the addon built.
- [ ] **If `src/binding.ts` starts importing the generated `native-engine.d.ts`** (i.e. the generated loader replaces the hand-written diagnostic front end), add `"build:native"` to `typecheck.dependsOn` in `turbo.json`. It is deliberately absent today: `typecheck` would otherwise make every `tsc --noEmit` wait on a cargo build, and nothing in `src/` references the generated dts yet. `test` already depends on `build:native`, so the addon itself is covered — this is only about the *types*. (turbo.json holds no comments, so this note lives here.)

## Definition of Done

- [ ] `pixi run gates` passes end to end with the addon built.
- [ ] A walk over a generated tree returns batched entries on both Bun and Node (a manual smoke run is acceptable until the task-4 harness exists).
