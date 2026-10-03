---
id: task-3
title: Wire the engine walk bridge and unfreeze the TypeScript WalkIterator
status: Done
assignee: []
created_date: '2026-09-30'
updated_date: '2026-10-03 11:00'
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

<!-- SECTION:DESCRIPTION:BEGIN -->
Bridge the task-2 scanner over N-API using the transport frozen in task-1, and replace the deliberately-throwing stub in `packages/path/src/walk.ts` with the real batched `AsyncGenerator`. The boundary rule (D2) governs the whole task: bulk operations only — one boundary crossing per batch, never per entry; string manipulation stays in TypeScript.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `crates/engine` exposes the walk over the task-1 decision (`#[napi(async_iterator)]` or chunked paging via `AsyncTask`); the crate stays glue-only, with all logic in `pathway-fs-core`.
- [x] #2 The version guards keep their semantics: `engine_version()` and the Node-API floor, so a stale `.node` file is refused at load time.
- [x] #3 `walk()` in `packages/path/src/walk.ts` yields `WalkBatch`es; `walkFiles()` and `walkDirs()` delegate to it.
- [x] #4 `AbortSignal` (the `signal` option) wires through to the native `cancel()`.
- [x] #5 `packages/path` still imports without a built addon (`engineAvailable()` stays safe); runtime paths that need native fail with the existing `BUILD_HINT`.
- [x] #6 The package version test (`package.json` matches the workspace version) still passes.
- [x] #7 `pixi run typecheck` and `pixi run test` are green with the addon built.
- [x] #8 **If `src/binding.ts` starts importing the generated `native-engine.d.ts`** (i.e. the generated loader replaces the hand-written diagnostic front end), add `"build:native"` to `typecheck.dependsOn` in `turbo.json`. It is deliberately absent today: `typecheck` would otherwise make every `tsc --noEmit` wait on a cargo build, and nothing in `src/` references the generated dts yet. `test` already depends on `build:native`, so the addon itself is covered — this is only about the *types*. (turbo.json holds no comments, so this note lives here.)
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `pixi run gates` passes end to end with the addon built.
- [x] #2 A walk over a generated tree returns batched entries on both Bun and Node (a manual smoke run is acceptable until the task-4 harness exists).
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Wired per the task-1 frozen decision (chunked AsyncTask paging). crates/engine/src/walk.rs is pure translation: Walker{new,scan,nextBatch,cancel,errors} over NativeScanner; FusedEntry->WalkEntry conversion happens in compute() off the JS thread; WalkEntry is shaped exactly like PathEntry so walk.ts yields batches without re-mapping. walk() yields files+dirs; walkFiles (native filesOnly) and walkDirs (TS narrowing) delegate to one drive() loop; early break cancels natively in finally. AbortSignal: throwIfAborted before each yield + abort listener -> native cancel. Version guards implemented for real in loadEngine(): engineVersion()==package.json version (stale build refused with BUILD_HINT) and napiVersion() floor vs process.versions.napi. AC#8: binding.ts still does NOT import native-engine.d.ts (hand-written NativeEngine interface kept), so typecheck does not depend on build:native and turbo.json is deliberately unchanged. Smoke run: 1203 entries in 3 batches (batch 512) + glob+xxhash over 1200 files on node 22.22.3, bun 1.3.11 and bun 1.4.2. Bonus fix caught by the Node smoke: bare require() in ESM was Bun-only; now createRequire. napi8 feature added to the workspace napi dep for BigInt (i128 modifiedNanos). TDD: 11 red tests in packages/path/test/walk.test.ts written first (0 pass -> 11 pass). pixi run gates green end to end.
<!-- SECTION:NOTES:END -->
