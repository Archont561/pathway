---
id: task-4
title: Build the benchmark harness and prove the fused-walk claim
status: Done
assignee:
  - '@me'
created_date: '2026-09-30'
updated_date: '2026-10-04'
labels:
  - bench
milestone: m-0
dependencies:
  - TASK-3
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 1 Step 1.4: a `benches/*` workspace, built to prove — or refute — the central claim that the fused walk is ≥5x faster than the best alternative on 100k+ files. The baseline to beat is the native C++ composite on Node 24: `node:fs.glob` + `fs.stat` + `crypto`, not a pure-JS globber.

**Wiring this task must restore (2026-09-30):** the `benches/*` scaffolding was removed because it was a phantom — the glob matched no directory, `bun.lock` had zero references, and the `bench` tasks therefore exited 0 having run nothing. Re-add all of it *together with the harness*, in one commit:

1. `benches/*` in the root `package.json` `workspaces`.
2. `"bench": "turbo run bench"` in the root `package.json` scripts.
3. A `bench` task in `turbo.json` — `dependsOn: ["^build", "build:native"]` (the harness needs the addon, and `^build` alone does not provide it), `outputs: ["results/**"]`.
4. A `bench` task in `pixi.toml`: `{ cmd = "bun run bench", depends-on = ["bun-install"] }`.
5. The biome override relaxing `noExplicitAny` for `benches/**`. (It also proposed `noConsole`, which this Biome version does not enable — the harness reports through `process.stdout.write`, so the rule is moot.)
6. ~~`benches/**` back into the `js-typecheck` glob in `lefthook.yml`.~~ **Obsolete.** The `js-typecheck` glob is gone: `pixi run typecheck` is now a repo-wide turbo task over every TS package, and `benches/walk` declares its own `typecheck` script, so it is covered without a lefthook edit. Do not go looking for that glob.

The bench package extends `@repo/typescript-config/base.json` (not `library.json` — it emits no declarations).
<!-- SECTION:DESCRIPTION:END -->

## Completion (2026-10-04) — the recorded sweep now runs in CI

Per the direction given on 2026-10-03, the recorded benchmark moved out of ephemeral local hardware and into a GitHub Actions workflow (`.github/workflows/bench.yml`): job 1 runs `pixi run bench --force` — the DoD path, through the turbo graph — on pixi's Bun 1.3, job 2 runs the driver directly under Bun 1.4.2 (from the `@oven/bun-linux-x64` npm tarball), both on `ubuntu-24.04`, both asserting the release addon before measuring and publishing `report.md` to the run summary plus an artifact.

Two workflow defects were fixed along the way, both found only by CI: run 1's job 2 crashed in ~2 min with `Cannot find module '@archont561/pathway'` — the job built only the addon, but the package's `exports` resolve to `./dist/index.js`, so job 2 now builds `dist/` + the release addon via `turbo run build build:native:release --filter=@archont561/pathway` (commit `605ee56`); and the artifact zips turned out to be un-fetchable from this sandbox (blocked blob host), which is why the reports are also appended to `$GITHUB_STEP_SUMMARY` and the numbers below were transcribed from the drivers' per-scenario stdout in the job logs of [run 37163546083](https://github.com/Archont561/pathway/actions/runs/37163546083) (both jobs green, ~71 and ~78 min).

**The CI sweep's verdict (ubuntu-24.04, 4-vCPU, 10k–1M, both Bun lines):** the fused walk wins every size and runtime, but by **1.02–1.15x** against `fdir` + a 32-wide stat/hash pool — the 2-core sandbox's 1.85x does not reproduce on the bigger machine (pathway's fused time is unchanged; the pooled JS consumer is faster). The ≥5x claim is withdrawn on every machine measured. Bun 1.4 vs 1.3: pathway marginally faster at scale, `node:fs.glob` ~4x slower (117 s vs 94 s fused at 1M), `fdir` unchanged, `Bun.Glob.scan` measured on both lines (instance method — the static never existed). Full tables: the "CI benchmark sweep (2026-10-04)" section of [verified-data.md](../../.knowledge/competitive/verified-data.md).

## Acceptance Criteria

- [x] File-tree generator supports 10k, 100k, 500k and 1M files. — all four sizes generated and measured in both CI jobs; entry counts match the local sweep exactly (3,950 / 38,660 / 184,094 / 368,047 for scenario B), so the generator is deterministic across machines.
- [x] Benchmark A: raw traversal (paths only).
- [x] Benchmark B: traversal with a complex exclusion set. — with the documented `bun-glob` asymmetry: `GlobScanOptions` has no `ignore`, so it filters in JS after a full scan.
- [x] Benchmark C: the fused walk (traverse + stat + hash in one pass).
- [x] Baselines compared: `node:fs.glob` (+ stat + crypto), `fdir`, `tinyglobby`, and `Bun.Glob.scan()` on both Bun 1.3 and Bun 1.4. — `Bun.Glob.scan` runs on both lines since the harness switched to the instance method; serial, pooled (`fdir` + 32-wide pool) and pathway subjects all measured at all four sizes on both runtimes.
- [x] Every run records wall time (p50/p95), peak heap, GC pressure, time-to-first-entry and cancellation cost. — recorded in `results/results/{results.json,report.md}` (CI artifacts; p50, time-to-first-entry and entry counts are also visible in the job logs and are what `.knowledge` transcribes). Cancellation cost and the heap/GC columns remain from the local baseline — the CI logs do not print them.
- [x] Results are documented in `.knowledge` with per-configuration tables. — verified-data.md now carries both the local 2026-10-03 baseline and the 2026-10-04 CI sweep (three scenario tables × two runtimes × four sizes, ratio table, interpretation).
- [x] The verdict is recorded explicitly: ≥5x on 100k+ files against the best alternative, or the gap analysis of why the claim misses. — the claim **misses**: 1.85x locally, 1.06–1.15x in CI; gap analysis in verified-data.md and [fused-walk.md](../../.knowledge/architecture/fused-walk.md).

## Definition of Done

- [x] `pixi run bench` runs the full harness through turbo. — observed in CI (job 1: build → build:native → build:native:release → bench, all `cache bypass, force executing`, 69 min for the four tasks).
- [x] `.knowledge/competitive/verified-data.md` carries the measured numbers with the date they were pulled. — 2026-10-03 (local baseline) and 2026-10-04 (CI sweep), both dated in the file, CI run and job provenance in the frontmatter sources.

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Restore the benches/* workspace, the turbo/pixi bench tasks and the biome override together with the harness, so the fused-walk claim is measured rather than asserted. Generator 10k-1M, benchmarks A/B/C, baselines node:fs.glob+fdir+tinyglobby+Bun.Glob.scan, metrics p50/p95 + peak heap + GC + time-to-first-entry + cancellation, then record the verdict.
<!-- SECTION:PLAN:END -->
