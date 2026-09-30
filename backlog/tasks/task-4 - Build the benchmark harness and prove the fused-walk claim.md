---
id: task-4
title: Build the benchmark harness and prove the fused-walk claim
status: To Do
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30 20:24'
labels:
  - bench
milestone: m-0
dependencies:
  - TASK-3
priority: medium
---

## Description

Phase 1 Step 1.4: a `benches/*` workspace, built to prove — or refute — the central claim that the fused walk is ≥5x faster than the best alternative on 100k+ files. The baseline to beat is the native C++ composite on Node 24: `node:fs.glob` + `fs.stat` + `crypto`, not a pure-JS globber.

**Wiring this task must restore (2026-09-30):** the `benches/*` scaffolding was removed because it was a phantom — the glob matched no directory, `bun.lock` had zero references, and the `bench` tasks therefore exited 0 having run nothing. Re-add all of it *together with the harness*, in one commit:

1. `benches/*` in the root `package.json` `workspaces`.
2. `"bench": "turbo run bench"` in the root `package.json` scripts.
3. A `bench` task in `turbo.json` — `dependsOn: ["^build", "build:native"]` (the harness needs the addon, and `^build` alone does not provide it), `outputs: ["results/**"]`.
4. A `bench` task in `pixi.toml`: `{ cmd = "bun run bench", depends-on = ["bun-install"] }`.
5. The biome override relaxing `noExplicitAny` / `noConsole` for `benches/**`.
6. `benches/**` back into the `js-typecheck` glob in `lefthook.yml`.

The bench package extends `@repo/typescript-config/base.json` (not `library.json` — it emits no declarations).

## Acceptance Criteria

- [ ] File-tree generator supports 10k, 100k, 500k and 1M files.
- [ ] Benchmark A: raw traversal (paths only).
- [ ] Benchmark B: traversal with a complex exclusion set.
- [ ] Benchmark C: the fused walk (traverse + stat + hash in one pass).
- [ ] Baselines compared: `node:fs.glob` (+ stat + crypto), `fdir`, `tinyglobby`, and `Bun.Glob.scan()` on both Bun 1.3 and Bun 1.4.
- [ ] Every run records wall time (p50/p95), peak heap, GC pressure, time-to-first-entry and cancellation cost.
- [ ] Results are documented in `.knowledge` with per-configuration tables.
- [ ] The verdict is recorded explicitly: ≥5x on 100k+ files against the best alternative, or the gap analysis of why the claim misses.

## Definition of Done

- [ ] `pixi run bench` runs the full harness through turbo.
- [ ] `.knowledge/competitive/verified-data.md` carries the measured numbers with the date they were pulled.
