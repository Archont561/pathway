---
title: "Verified Market Data: Round 1 (July 2025) + Round 2 (Sept 2026)"
domain: competitive
status: decided
created: 2025-07-11
updated: 2026-09-16
source: web-search
depends_on:
  - competitive/landscape
tags: [verification, corrections, tinyglobby, bun, fdir, fast-glob, downloads, node, fs.glob, napi-rs]
---

# Verified Market Data

## Round 2 (September 2026) — Current

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
phase-plan.md).

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
[fused-walk.md](../architecture/fused-walk.md) for benchmark criteria.

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
| **Node 24 LTS / 26 Current / 20 EOL (Sept 2026)** | CI matrix refresh: 24 + 26 (+22 optional) |
| **`node:fs.glob` stable in Node core** | Enters benchmark + competitor set; moat reframed as "fused pipeline vs native glob" |
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
