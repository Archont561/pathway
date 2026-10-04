---
type: Market Intelligence
title: "Verified Data: Measured Fused-Walk Benchmark (Oct 2026) + Market Rounds 1–2"
description: "Measured fused-walk baseline (local 1.85x; CI sweep 1.06–1.15x on 10k–1M, both Bun lines — the ≥5x claim withdrawn) plus web-verified market facts: Node 24/26, stable node:fs.glob, Bun 1.3/1.4, Deno 2 NAPI, NAPI-RS iterators."
tags: [verification, benchmark, fused-walk, corrections, tinyglobby, bun, fdir, fast-glob, downloads, node, fs.glob, napi-rs]
status: stable
generated:
  by: pathway_kb/1.0
  at: 2026-10-03T00:00:00Z
verified:
  - by: process:web-search-round2
    at: 2026-09-16T00:00:00Z
  - by: process:benchmark-task-4
    at: 2026-10-03T00:00:00Z
  - by: process:ci-benchmark-sweep
    at: 2026-10-04T00:00:00Z
stale_after: 2026-12-16T00:00:00Z
sources:
  - id: web-verification-round2
    resource: "live web re-verification, 2026-09-16: Node release index and node:fs.glob API docs (nodejs.org), Bun 1.3/1.4 release notes and Bun.GlobScanOptions docs (bun.com), NAPI-RS iterator/async decision-table docs (napi.rs), Deno 2 NAPI compatibility notes, chokidar 5.0.0 release notes, unrs-resolver npm listing"
    title: Round 2 live web re-verification
    author: process:web-search
    last_modified: 2026-09-16T00:00:00Z
  - id: benchmark-task-4
    resource: "benches/walk harness, 100k files, Bun 1.3.11-canary.1 on a 2-core Linux container, release addon, warm page cache"
    title: Fused-walk measured baseline (first run of the harness)
    author: process:benchmark-task-4
    last_modified: 2026-10-03T00:00:00Z
  - id: ci-benchmark-sweep-2026-10-04
    resource: "GitHub Actions run 37163546083 (commit 605ee56): bench (bun 1.3 · pixi run bench) and bench (bun 1.4 · direct driver), both green, ubuntu-24.04 4-vCPU runners, release addon, 10k–1M; p50/first-entry/entries transcribed from the job logs, full metrics in the run's report artifacts"
    title: Full CI benchmark sweep, both Bun lines, 10k–1M files
    author: process:ci-benchmark-sweep
    last_modified: 2026-10-04T00:00:00Z
  - id: market-snapshot-round1
    resource: "npm registry weekly-download and GitHub star snapshot, July 2025"
    title: Round 1 market snapshot (historical)
    author: process:web-search
    last_modified: 2025-07-11T00:00:00Z
domain: competitive
decision: decided  # legacy KB status (decided|proposed|deprecated)
created: 2025-07-11
source: web-search
depends_on:
  - competitive/landscape
---

# Verified Market Data

## Benchmark baseline (2026-10-03) — the first measured numbers

> **The ≥5x fused-walk claim does not hold.** On 100k files the fused walk is
> **1.85x** faster than the strongest alternative on this container, not ≥5x,
> and **1.06–1.15x** when the same harness runs on CI hardware (see the CI
> sweep below — the margin is machine-dependent and never ≥5x). It is **5.7x**
> slower than `fdir` on raw traversal. Measured with `benches/walk`
> (backlog task-4); the full tables are in that package's generated
> `results/results/report.md`. This supersedes the *projected* figures in
> [fused-walk.md](/architecture/fused-walk.md), which were written before any
> harness existed.

### Configuration

| | |
|---|---|
| Platform | 2-core Linux container, overlayfs on loopback |
| Runtime | Bun `1.3.11-canary.1`, Node target 24.3.0 |
| Addon | **release** build (`napi build --release`) |
| Harness | `pixi run bench`, `PATHWAY_BENCH_SIZES=100000` |
| Tree | 100,000 files, ~6.4 MB of content, generated deterministically |
| Cache | whole tree read once before measuring (warm-cache steady state) |

Two methodology notes, both forced by observed failures, because they change
the numbers by more than the claim does:

- **The addon must be a release build.** The first run used the debug addon
  and put the fused walk at 245 ms where the release build measures 1380 ms —
  the debug profile is not 2x off, it changes the ranking.
- **The tree is read once before measuring.** Generating 100k files leaves
  hundreds of MB of dirty pages; the first subject that then reads *content*
  pays for the generator's writeback. One run put the fused walk at **29.2 s**
  and the next at **1.3 s** for identical work. Scenarios A and B only read
  directory entries, so they looked stable throughout and hid the cause
  entirely.

### A — raw traversal (paths only)

| Implementation | p50 ms | p95 ms | first entry ms | peak heap | entries |
|---|---:|---:|---:|---:|---:|
| **`fdir`** | **70.8** | 73.8 | 81.2 | 33.6 MiB | 100,000 |
| `node-glob` | 245.5 | 275.4 | 94.2 | 26.6 MiB | 100,000 |
| `tinyglobby` | 288.9 | 343.4 | 206.3 | 35.2 MiB | 100,000 |
| pathway | 404.2 | 423.0 | 152.5 | 39.3 MiB | 100,000 |

**Pathway is 0.18x — it loses to `fdir` by 5.7x.** `fdir` uses
`readdir(withFileTypes: true)` and never stats; pathway pays for the `ignore`
crate's generality (gitignore support, pruning, parallel workers) whether or not
the caller uses it.

### B — traversal with an exclusion set

| Implementation | p50 ms | p95 ms | first entry ms | peak heap | entries |
|---|---:|---:|---:|---:|---:|
| **`fdir`** | **50.0** | 52.1 | 49.1 | 21.5 MiB | 38,660 |
| pathway | 121.6 | 124.4 | 118.8 | 10.5 MiB | 38,660 |
| `tinyglobby` | 633.8 | 748.9 | 585.0 | 15.8 MiB | 38,660 |
| `node-glob` | 824.2 | 1134.2 | 102.2 | 27.0 MiB | 38,660 |

**0.41x.** Pre-descent pruning (`node_modules`, `dist`, `.git` never entered)
is where pathway earns its keep — 38,660 entries instead of 100,000 — but the
scenarios measure equal work over equal outputs, so pruning does not show up as
speed here. It would show in a tree where the excluded directories are large.

### C — fused traversal + stat + hash (the claim)

| Implementation | p50 ms | p95 ms | first entry ms | peak heap | entries |
|---|---:|---:|---:|---:|---:|
| **pathway (fused)** | **1,380.1** | 1,401.1 | 1,093.2 | 14.3 MiB | 100,000 |
| `fdir` + 32-wide pool | 2,551.7 | 2,672.3 | 115.4 | 39.3 MiB | 100,000 |
| `fdir` (serial) | 4,856.9 | 4,983.5 | 133.8 | 40.9 MiB | 100,000 |
| `tinyglobby` (serial) | 4,990.3 | 5,575.8 | 213.6 | 33.8 MiB | 100,000 |
| `node-glob` (serial) | 5,280.2 | 5,370.9 | 101.1 | 29.3 MiB | 100,000 |

**1.85x against `fdir` + a 32-wide stat/hash pool on this container** — 3.6x
against `node:fs.glob` + `node:fs` + `node:crypto`, the baseline the ≥5x
threshold was written against. (The 2026-10-04 CI sweep below puts the same
comparison at 1.06–1.15x on 4-vCPU runners.) Peak heap is genuinely lower (14.3 MiB vs 39.3 MiB) and GC
pressure is lower (2.8 ms vs 20.6 ms per sample), which is the fusion working as
designed; the wall-clock margin is simply smaller than projected.

**Why the earlier 3.7x reading was wrong.** The first version of this baseline
ran each baseline's stat+hash in a plain `for` loop, serialising 100k round
trips. That measures pathway's internal parallelism against a single-threaded
consumer — not fusion. Adding the concurrent baseline halved the apparent win.
The honest strongest alternative is a consumer that keeps the disk busy too.

### Cancellation (pathway only)

Abort lands mid-walk; the iterator settles in **0.4 ms**. It does **not** surface
the abort reason, and **0 batches** had been delivered when the abort fired.

### Gap analysis — why the claim misses

1. **The baseline got stronger, not weaker.** The ≥5x threshold assumed
   `node:fs.glob` + serial post-processing. A competent consumer pools its I/O,
   and the pooled baseline is 2.1x faster than the serial one. Fusion removes
   JS↔native round trips; it does not remove the round trips to the kernel.
2. **The traversal is already cheap.** At ~71 ms for 100k entries, `fdir`
   spends 0.7 µs per file. The stat+hash pass costs ~1.8 s fused — hashing and
   I/O are ~20x the traversal, so fusion can only ever amortise the smaller term.
3. **`withMetadata: false` is load-bearing.** The N-API binding defaults it to
   `true`, which stats every entry. Left on, scenario A gave pathway a per-file
   `stat` that no baseline performs.
4. **Streaming is not incremental yet.** `scan()` runs the entire traversal
   before JavaScript can pull the first batch, so time-to-first-entry is 79% of
   total (1,093 of 1,380 ms) while every baseline's first entry arrives in
   ~100 ms. On a *streaming* metric pathway is currently the worst option, and
   for a build tool reacting to a large tree that matters more than p50.

### What would have to change to reach 5x

Nothing in the harness. The gap is architectural: 1.06–1.85x against a pooled
baseline (CI runner vs 2-core sandbox) means the fusion win is real but bounded,
and closing it to 5x requires either a faster hash (the pass is I/O- and
hash-bound) or true incremental batching so the traversal overlaps the consumer. **The ≥5x figure should be
withdrawn from marketing until re-measured**, and [fused-walk.md](/architecture/fused-walk.md)'s
projected 10–20x replaced with these numbers.

### Not yet measured

~~10k / 500k / 1M trees, and `Bun.Glob.scan` on Bun 1.4~~ — **measured 2026-10-04**, in the CI
sweep below. What this local baseline still uniquely contributes: peak heap, GC pressure, p95
and the cancellation measurement — the CI job logs expose p50, time-to-first-entry and entry
counts (the driver's own progress output), while the full metric set lives in the workflow's
`report.md` artifacts. Single-run p50 on a 2-core container; repeat runs vary by up to 2x, so
treat these as order-of-magnitude.

---

## CI benchmark sweep (2026-10-04) — GitHub Actions, both Bun lines, 10k–1M

The recorded sweep now runs where benchmarks belong — in CI
([run 37163546083](https://github.com/Archont561/pathway/actions/runs/37163546083), commit
`605ee56`), not on ephemeral local hardware. Two jobs: Bun `1.3.11-canary.1` through
`pixi run bench --force` (turbo graph, westus3 runner) and Bun `1.4.2` through the driver
directly (eastus2 runner) — both on the `ubuntu-24.04` image (GitHub's standard 4-vCPU
runner), release addon asserted by the workflow before measuring, page cache settled before
each size (`read 64,000,000 bytes in 99.0 s / 101.5 s` at 1M). The numbers below are p50 ms,
transcribed from the drivers' own per-scenario output in the job logs; p95, peak heap, GC and
cancellation live in the `bench-report-bun-1-3` / `bench-report-bun-1-4` artifacts
(digests `7e3fdda6…` / see run page). Entry counts matched across every implementation at
every size — and match the local sweep exactly (3,950 / 38,660 / 184,094 / 368,047 for
scenario B), so the tree generator is deterministic across machines.

> **The CI hardware moves the headline again — downward.** Against `fdir` + a 32-wide
> stat/hash pool the fused walk is **1.06x at 100k, 1.10x at 500k, 1.09x at 1M** on Bun 1.3
> and **1.08x / 1.15x / 1.11x** on Bun 1.4 (parity, 1.02–1.03x, at 10k). The 2-core
> sandbox's 1.85x does not reproduce on the 4-vCPU runner: pathway's fused time is the same
> on both machines (506.9 vs 500.3 ms at 100k), but the pooled JS consumer is faster on the
> bigger runner (538.4 vs 729.1 ms) — the margin is machine-dependent, and **on no measured
> machine does it reach ≥5x**. The claim is withdrawn everywhere, not just locally.

### C — fused traversal + stat + hash (the claim), p50 ms

| Implementation | 10k | 100k | 500k | 1M |
|---|---:|---:|---:|---:|
| **Bun 1.3.11** | | | | |
| **pathway (fused)** | 56.5 | **506.9** | **2,572.2** | **5,155.8** |
| `fdir` + 32-wide pool | **58.1** | 538.4 | 2,820.4 | 5,597.3 |
| `fdir` (serial) | 887.9 | 8,794.5 | 44,715.3 | 88,350.4 |
| `node-glob` (serial) | 933.5 | 9,258.0 | 46,781.3 | 93,877.3 |
| `tinyglobby` (serial) | 903.8 | 9,006.8 | 45,503.3 | 90,959.8 |
| `bun-glob` (serial) | 912.9 | 9,212.0 | 46,025.2 | 92,639.2 |
| **Bun 1.4.2** | | | | |
| **pathway (fused)** | **52.1** | **502.0** | **2,494.6** | **4,979.4** |
| `fdir` + 32-wide pool | 53.0 | 544.1 | 2,869.2 | 5,521.8 |
| `fdir` (serial) | 999.3 | 9,885.5 | 49,645.4 | 99,705.8 |
| `node-glob` (serial) | 1,115.0 | 11,026.6 | 56,589.9 | 117,184.0 |
| `tinyglobby` (serial) | 996.0 | 9,996.4 | 50,445.3 | 100,984.9 |
| `bun-glob` (serial) | 1,020.3 | 10,198.5 | 51,002.6 | 102,141.2 |

**Verdict: the fused walk wins every size on both runtimes, but by 1.02–1.15x against the
pooled baseline, not ≥5x.** The serial baselines are 17–22x slower than both pathway and the
pool — the serial comparison flatters whatever it is measured against, which is why the pool
is the comparator of record. Time-to-first-entry stays the architectural sore point at scale:
at 1M, pathway's first entry lands at 4,120 ms (1.3) / 4,080 ms (1.4) of a ~5.0–5.2 s total —
**~80%** — while `fdir`+pool's first entry arrives at 360/312 ms.

### A — raw traversal (paths only), p50 ms

| Implementation | 10k | 100k | 500k | 1M |
|---|---:|---:|---:|---:|
| **Bun 1.3.11** | | | | |
| **`fdir`** | **4.1** | **29.7** | **159.1** | **339.1** |
| `tinyglobby` | 7.7 | 69.7 | 390.5 | 868.7 |
| `bun-glob` | 11.1 | 103.9 | 555.4 | 1,183.9 |
| pathway | 12.6 | 120.2 | 571.9 | 1,214.2 |
| `node-glob` | 15.2 | 145.9 | 790.8 | 1,626.6 |
| **Bun 1.4.2** | | | | |
| **`fdir`** | **4.1** | **32.3** | **160.4** | **323.1** |
| `tinyglobby` | 7.4 | 66.6 | 348.6 | 737.2 |
| `bun-glob` | 10.8 | 104.9 | 529.0 | 1,065.5 |
| pathway | 12.3 | 106.5 | 527.3 | **1,050.6** |
| `node-glob` | 57.5 | 557.0 | 2,902.8 | 6,171.0 |

Pathway loses raw traversal to `fdir` by **3.0–4.0x** everywhere (it was 5.7x in the first
local 100k run) — `fdir` reads `readdir(withFileTypes)` and never stats, while pathway pays
for the `ignore` crate's generality. On Bun 1.4 pathway pulls level with `bun-glob` and ahead
of a collapsing `node-glob` (below).

### B — traversal with an exclusion set, p50 ms

| Implementation | 10k | 100k | 500k | 1M |
|---|---:|---:|---:|---:|
| **Bun 1.3.11** | | | | |
| **`fdir`** | **2.3** | **18.1** | **99.9** | **185.3** |
| pathway | 7.2 | 49.1 | 249.9 | 493.1 |
| `bun-glob` | 16.5 | 159.4 | 905.4 | 1,918.0 |
| `node-glob` | 38.4 | 380.0 | 1,902.8 | 3,778.1 |
| `tinyglobby` | 39.2 | 386.0 | 1,859.7 | 3,735.4 |
| **Bun 1.4.2** | | | | |
| **`fdir`** | **2.2** | **17.5** | **98.5** | **193.4** |
| pathway | 7.3 | 51.9 | 235.7 | 446.4 |
| `bun-glob` | 13.5 | 132.4 | 676.4 | 1,348.5 |
| `tinyglobby` | 32.9 | 319.9 | 1,568.2 | 3,113.3 |
| `node-glob` | 99.0 | 908.2 | 4,462.0 | 9,312.3 |

Pathway is 2.5–3.1x behind `fdir` on equal work (38,660 of 100k entries survive the
exclusions; the pruning itself is the product, not the speed). The `bun-glob` rows carry the
documented asymmetry: `Bun.GlobScanOptions` has no `ignore`, so scenario B filters in JS
after a full scan — its scenario-B cost is not its traversal cost.

### What the sweep settles

1. **The ≥5x claim is dead on every machine measured.** 1.85x (2-core sandbox, 100k) →
   1.06–1.15x (4-vCPU CI runner, 10k–1M, both Bun lines). The margin is real, positive and
   machine-dependent; the honest one-line claim is *"fused stat+hash beats the best JS
   pipeline by ~1.1x on CI hardware, with lower memory and GC"* (heap/GC figures from the
   local baseline, artifact-preserved in CI).
2. **The unresolved ratio-trend question is closed.** Local sweeps disagreed about the
   direction (1.53→1.44 vs 1.29→1.58 across sizes); CI says flat ~1.1x with a mild hump at
   500k. No size regime rescues the claim.
3. **Bun 1.4 vs 1.3, measured:** pathway's fused walk is marginally *faster* on 1.4 at scale
   (4,979 vs 5,156 ms at 1M); `node:fs.glob` regressed hard on 1.4 (557 vs 146 ms at 100k
   raw, 117.2 s vs 93.9 s fused at 1M — the worst serial baseline); `fdir` is unchanged;
   `Bun.Glob.scan` (instance method — the static never existed) now runs on both lines and
   sits between `fdir` and `node-glob` on raw traversal, ~17x off the pool when fused.
4. **The two jobs ran on different physical runners** (westus3 / eastus2, same image), so
   cross-runtime deltas carry that caveat; within-job comparisons share a runner.

---

## Round 2 (September 2026) — Market

Live web re-verification, 2026-09-16. Supersedes Round 1 where they
conflict. **Re-pull npm download stats again before any external
publication.**

### Node.js runtime lines (Sept 2026)

| Line | State | Notes |
|------|-------|-------|
| Node 24 "Krypton" | **LTS** (24.21.0) | Active LTS from Oct 2025; Maintenance window from Oct 2026; supported to Apr 2028 |
| Node 26 | **Current** (26.8.2) | Released May 2026; becomes Active LTS Oct 2026 |
| Node 22 | Maintenance | EOL Apr 2027 — optional CI leg at most |
| Node 20 | **EOL** | Drop from all support statements |

Consequence: CI matrix is Node 24 + 26 (+22 optional); `engines` is
`node >=24`.

### `node:fs.glob` — stable native glob in Node core

- **Stability marked in v22.17.0 / v24.0.0** (nodejs.org docs).
- Options: `cwd` (URL instances supported since v22.17.0/v24.0.0),
  `exclude` (function **or** array of glob patterns; glob-form exclude since
  v22.14.0/v23.7.0), `withFileTypes` (Dirents, since v22.2.0),
  `followSymlinks` (since v26.1.0).
- Async-iterator + sync variants; **C++-native in the runtime core**.

This is now the traversal baseline on Node. It returns paths/Dirents only —
no stat+hash fusion, no metadata, no composition. The fused-walk moat
survives; the benchmark must include it (done in fused-walk.md and
`backlog/docs/phase-plan.md`).

### Bun 1.3 (Jan 2026) and Bun 1.4 (Aug 20, 2026)

- Bun 1.3: major release (unified DB APIs, frontend tooling, cgroup-aware
  parallelism; `Bun.Glob.scan` ~2× faster per the 1.3.12 notes).
- **Bun 1.4: Bun was rewritten in Rust.** Ships `Bun.WebView`, `Bun.Image`,
  `Bun.markdown`, JSON5/JSONL, cron APIs, Node.js 26.3 compatibility
  (1,517 newly passing tests), Windows ARM64 builds, global virtual store
  for installs (up to 7× faster).
- Current `Bun.GlobScanOptions` (bun.com docs): `absolute`, `cwd`,
  `dot` (**default false**), `followSymlinks`, `onlyFiles` (default true),
  `throwErrorOnBrokenSymlink`; plus `scanSync`.
- **All Zig-era assumptions from Round 1 are void.** CI validates N-API on
  both 1.3.x and 1.4.x; all Bun benchmarks run on both.

### Deno 2

- Node-API addons supported since Deno 2.0 (local `node_modules` +
  `--allow-ffi`); as of Deno 2.8, >75% of Node's own test suite passes.
- Deno 2.7 (Feb 2026) added Windows on ARM builds.
- Consequence: "best-effort, don't CI-test" is under-inclusive — add an
  optional (non-blocking) Deno 2 smoke job from v0.2.

### chokidar 5.0.0 (Nov 25, 2025)

ESM-only, Node ≥20.19, single dependency (readdirp 5), built on core
`fs.watch`. The "uninstall chokidar" pitch remains valid but watch ships in
Phase 4 (v0.4) via the `notify` crate.

### NAPI-RS platform state (Sept 2026)

- **Iterators:** `#[napi(iterator)]` (Generator) and
  `#[napi(async_iterator)]` (AsyncGenerator) are **experimental** but
  first-class; pull-based; `return()` is the cancellation hook; `Yield`
  must be `Send + 'static`; overlapping `next()` not serialized; docs
  include a test checklist (forced GC, early break, worker shutdown).
- **Async decision table** (napi.rs): `#[napi] async fn` → NAPI-RS Tokio
  runtime; **blocking/CPU work → `AsyncTask<T>` on the libuv thread pool**;
  lazy sequences → iterators; byte streaming → `web_stream` feature. A
  custom `async-runtime` SPI exists (Tokio is no longer the only option).
- **Distribution:** `napi-postinstall` (esbuild-style helper; cites
  npm/cli#4828) exists for legacy npm versions that mishandle
  platform-specific `optionalDependencies`.

### New adjacent projects

- **`unrs-resolver`** (MIT, Rust + NAPI-RS, on npm): enhanced-resolve +
  tsconfig-paths + tsconfck port; Yarn PnP; concurrent LRU cache; 74
  platform targets; WASM + JS fallbacks. **Directly subsumes our v1.0
  Resolver build plan** — we now integrate it (pluggable-patterns.md §E).
- **Effect Platform (`@effect/platform`, v4 RC)**: typed FileSystem/Path
  services for Node/Bun/Deno/browser, streams, watch via
  `@parcel/watcher`. Occupies the "typed FS for TypeScript" narrative
  space; Effect-based tooling (a named target audience) may standardize on
  it. Our fused-walk moat is unaffected; the landscape doc records it.

### Temp-file semantics (correction to killer-features.md claim)

- Rust `tempfile` crate: `mkstemp`/`mkdtemp` + destructor cleanup.
  **Does not survive SIGKILL** (destructor never runs; file leaks until an
  OS tmp reaper).
- Hard guarantee requires OS flags: `O_TMPFILE` (Linux local FS — not NFS)
  or `FILE_FLAG_DELETE_ON_CLOSE` (Windows). Docs now state the tiered
  guarantee explicitly.

---

## Round 1 (July 2025) — Historical

> Figures below are 14 months old. Directionally still useful; **do not
> cite externally without re-pulling**.

## Overview

This document records corrections to the initial competitive analysis based
on live web search verification. The original analysis was directionally
correct but had several outdated data points that significantly affect
strategic positioning.

---

## Correction 1: `tinyglobby` Is the New Ecosystem Default

### Original Claim
> `tinyglobby` ~800 stars, narrow scope, used by Vite 6+.

### Verified Data (July 2025)
- **GitHub stars:** 523
- **Weekly npm downloads:** **186 million** (per npm registry)
- **Dependencies:** Only 2 (`fdir` + `picomatch`)
- **Description:** "A fast and minimal alternative to globby and fast-glob"
- **Endorsement:** The `fast-glob` author himself recommends tinyglobby as
  the drop-in replacement
- **Adoption:** Used by Vite 6+, and Eleventy is actively evaluating it for
  the 3.1.0 milestone (issue: "test viability of fast-glob replacements")

### Strategic Impact
This is the **most important correction** of Round 1. `tinyglobby` at 186M
downloads/week is no longer a "newer alternative" — it is **infrastructure**.
It has more weekly downloads than `fast-glob` (73M) and `fdir` (37M)
individually.

**Implication for us:** We are not competing against a fragmented glob
ecosystem. We are competing against a **consolidating standard** — and,
since Sept 2026, against **Node core's own `fs.glob`** as the native
baseline (see Round 2). Our Rust walker must beat both by a wide margin on
the **full pipeline** (walk + stat + hash), not on raw glob matching.

---

## Correction 2: `Bun.Glob.scan()` Does Full Traversal

### Original Claim
> Bun `Bun.Glob` — "matching only, no traversal."

### Verified Data (July 2025; superseded by Round 2 for versions/options)
Bun now provides **full async directory traversal** via `Bun.Glob`:

```ts
const glob = new Bun.Glob("**/*.ts");

// Full async traversal (returns AsyncIterable<string>)
for await (const file of glob.scan({ cwd: "/project", dot: false })) {
  console.log(file);
}

// Sync traversal
for (const file of glob.scanSync("/project")) {
  console.log(file);
}

// Pattern matching only (no I/O)
glob.match("src/index.ts");  // true/false
```

Additionally, Bun implements `node:fs.glob` for Node.js compatibility:
```ts
import { glob } from "node:fs/promises";
for await (const entry of glob("**/*.ts", { exclude: (f) => f.name === "node_modules" })) {
  console.log(entry);
}
```

### Strategic Impact
Our original analysis **undersold Bun's capabilities**. Bun is not just a
pattern matcher — it has a real, native traversal engine with streaming
async iteration.

**Revised Bun strategy (re-validated Sept 2026 for Bun 1.3/1.4):**
1. We do **not** compete with `Bun.Glob.scan()` on raw traversal speed.
   Bun's native implementation (Zig in 1.3, Rust in 1.4) is comparable to
   our Rust `ignore` crate.
2. We compete on the **composition layer**: path objects, serialization,
   hashing, atomic writes, sandboxing, transactions — none of which Bun
   provides.
3. The fused walk (stat + hash in one pass) remains our differentiator
   even on Bun, because `Bun.Glob.scan()` returns strings only (confirmed
   against current docs).
4. We use NAPI-RS across both Node and Bun. Bun's N-API support is
   best-effort upstream, so our CI is the support contract — **now on both
   1.3.x and the Rust-rewritten 1.4.x**.

---

## Correction 3: `fdir` Is Larger Than Estimated

### Original Claim
> `fdir` ~1.5k stars.

### Verified Data (July 2025)
- **GitHub stars:** 1,674
- **Weekly npm downloads:** **37 million**
- **Claim:** "Crawl around 1 million files in under 1 second"
- **Architecture:** Heavily micro-optimized JS that maxes out `fs.readdir`
  with `withFileTypes: true` to avoid separate `stat()` calls
- **Role:** Powers `tinyglobby` as its traversal engine

### Strategic Impact
`fdir` is the **JS traversal ceiling**. It is as fast as pure JavaScript
can get for directory crawling. Our benchmark must acknowledge that on
**raw traversal alone** (just returning paths), the gap between `fdir` and
our Rust walker may be narrow (perhaps 2–3x, not 10x).

The decisive advantage is the **fused pipeline**: when you add stat + hash
to the traversal, `fdir` requires 100k+ additional JS↔libuv round-trips
while our Rust engine does it in a single pass. See
[fused-walk.md](/architecture/fused-walk.md) for benchmark criteria.

---

## Correction 4: `fast-glob` Star Count

### Original Claim
> `fast-glob` ~3.5k stars.

### Verified Data (July 2025)
- **GitHub stars:** 2,695
- **Weekly npm downloads:** **73 million**
- **Dependencies:** 17

### Strategic Impact
Minor correction. The download count (73M) is much higher than the star
count suggests, indicating massive passive usage (transitive dependency).
The author's recommendation of `tinyglobby` as a replacement signals that
`fast-glob` is entering maintenance mode.

---

## Correction 5: `fs-jetpack` Confirmed Dead

### Original Claim
> `fs-jetpack` ~1k stars, "unmaintained."

### Verified Data (July 2025)
- **Latest release:** v5.1.0, **3+ years ago**
- **TypeScript fork (`fs-jetpack-ts`):** Last release Feb 2017, 0 stars,
  0 dependents, 2 total releases
- **Status:** Confirmed abandoned

### Strategic Impact
Reinforces our positioning as the modern successor to the `fs-jetpack`
idea. The fluent path-object API is a validated concept with no maintained
implementation.

---

## Correction 6: `pathe` v2.0.3 Active

### Original Claim
> `pathe` ~2.5k stars, pure string manipulation.

### Verified Data (July 2025)
- **Latest:** v2.0.3, published February 11, 2025
- **Description:** "Drop-in replacement of Node.js's path module, ensures
  paths are normalized with slash `/`"
- **Status:** Active, modern ESM/TypeScript, no Node.js dependency

### Strategic Impact
Confirms `pathe` as the right choice for our string manipulation layer.
It's actively maintained, ESM-native, and has zero Node.js dependency
(important for Bun/Deno compatibility).

---

## Updated Download Rankings (July 2025 — STALE)

| Library | Weekly Downloads | Trend |
|---------|-----------------|-------|
| `tinyglobby` | **186M** | 📈 Rapidly growing, new default |
| `fast-glob` | 73M | 📉 Being displaced by tinyglobby |
| `globby` | ~40M | 📉 Being displaced by tinyglobby |
| `fdir` | 37M | 📈 Growing (powers tinyglobby) |
| `readdirp` | ~15M | ➡️ Stable, maintenance |
| `fs-extra` | ~30M | ➡️ Stable legacy |
| `pathe` | ~20M | 📈 Growing (Vite/Nuxt ecosystem) |

---

## NAPI-RS Ecosystem Verification (Round 1, July 2025)

### Confirmed
- NAPI-RS v3 is the current stable version
- Builds without `node-gyp` (pure Rust/JS toolchain)
- Multi-platform binary distribution is mature (platform-specific npm packages)
- Powers: SWC, Turbo, oxc, lightningcss, `node-rs/*`
- **No published standalone filesystem library exists** in the NAPI-RS
  ecosystem (re-verified Sept 2026 — still true; unrs-resolver is a
  resolver, not an FS library)

### Bun Compatibility
- NAPI-RS works on Bun via standard Node-API ABI
- Bun's N-API support is **best-effort** upstream (per NAPI-RS docs)
- Package authors must test Bun explicitly — cannot assume compatibility
- `bun:ffi` is experimental; Node-API is the recommended stable route

---

## Decision Summary

| Correction | Impact |
|-----------|--------|
| **Fused walk measured at 1.85x locally, 1.06–1.15x in CI (Oct 2026)** | The ≥5x claim **misses on every machine measured**; projected 10–20x was off by ~6x locally, ~10x in CI. Memory/GC claims hold. Withdraw 5x and 10–20x from marketing; re-baseline on incremental batching |
| **Node 24 LTS / 26 Current / 20 EOL (Sept 2026)** | CI matrix refresh: 24 + 26 (+22 optional) |
| **`node:fs.glob` stable in Node core** | Enters benchmark + competitor set; moat reframed as "fused pipeline vs native glob". Measured: 3.8x *serial*, 1.06–1.85x against a pooled baseline (CI vs sandbox) |
| **Bun 1.4 Rust rewrite (Aug 2026)** | All Zig-era assumptions void; CI + benchmarks on 1.3 **and** 1.4 |
| **NAPI-RS iterators experimental** | 1–2 day spike gates the streaming architecture (resolves Open Q1) |
| **`AsyncTask` for blocking work** | Walk path uses libuv pool, not Tokio workers (resolves Open Q2) |
| **`unrs-resolver` exists** | v1.0 Resolver becomes an adapter, not a build-from-scratch |
| **Effect Platform v4 (RC)** | Recorded as adjacent competitor in landscape.md |
| **chokidar 5.0.0 ESM-only** | Watch ships v0.4 (Phase 4); pitch timing corrected |
| **tempfile SIGKILL leak** | Tiered temp-dir guarantee documented (killer-features.md) |
| Deno 2 NAPI supported | Optional Deno smoke job from v0.2 |
| tinyglobby = 186M downloads (2025) | Compete on fused pipeline, not raw glob |
| Bun.Glob.scan() = real traversal | Bun is an ally with fast primitives; we add composition |
| fdir = 37M downloads, 1.6k stars | JS ceiling is high; fused walk is the differentiator |
| fast-glob = 2.7k stars, 73M downloads | Entering maintenance; tinyglobby is the successor |
| fs-jetpack = dead 3+ years | Validates our positioning as modern successor |
| No NAPI-RS filesystem library | Confirms the gap is wide open |
