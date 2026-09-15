---
title: "Web Search Corrections: tinyglobby, Bun.Glob.scan(), fdir Stats"
domain: competitive
status: decided
created: 2025-07-11
updated: 2025-07-11
source: web-search
depends_on:
  - competitive/landscape
tags: [verification, corrections, tinyglobby, bun, fdir, fast-glob, downloads]
---

# Verified Market Data (July 2025)

## Overview

This document records corrections to the initial competitive analysis based
on live web search verification. The original analysis was directionally
correct but had several outdated data points that significantly affect
strategic positioning.

---

## Correction 1: `tinyglobby` Is the New Ecosystem Default

### Original Claim
> `tinyglobby` ~800 stars, narrow scope, used by Vite 6+.

### Verified Data
- **GitHub stars:** 523
- **Weekly npm downloads:** **186 million** (per npm registry)
- **Dependencies:** Only 2 (`fdir` + `picomatch`)
- **Description:** "A fast and minimal alternative to globby and fast-glob"
- **Endorsement:** The `fast-glob` author himself recommends tinyglobby as
  the drop-in replacement
- **Adoption:** Used by Vite 6+, and Eleventy is actively evaluating it for
  the 3.1.0 milestone (issue: "test viability of fast-glob replacements")

### Strategic Impact
This is the **most important correction**. `tinyglobby` at 186M downloads/week
is no longer a "newer alternative" — it is **infrastructure**. It has more
weekly downloads than `fast-glob` (73M) and `fdir` (37M) individually.

**Implication for us:** We are not competing against a fragmented glob
ecosystem. We are competing against a **consolidating standard**. Our Rust
walker must beat `tinyglobby` by a wide margin on the **full pipeline**
(walk + stat + hash), not on raw glob matching where `tinyglobby` + `fdir`
is already very fast.

---

## Correction 2: `Bun.Glob.scan()` Does Full Traversal

### Original Claim
> Bun `Bun.Glob` — "matching only, no traversal."

### Verified Data
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
pattern matcher — it has a real, native (Zig-based) traversal engine with
streaming async iteration.

**Revised Bun strategy:**
1. We do **not** compete with `Bun.Glob.scan()` on raw traversal speed.
   Bun's Zig implementation is likely comparable to our Rust `ignore` crate.
2. We compete on the **composition layer**: path objects, serialization,
   hashing, atomic writes, sandboxing, transactions — none of which Bun
   provides.
3. The fused walk (stat + hash in one pass) remains our differentiator
   even on Bun, because `Bun.Glob.scan()` returns strings only.
4. We use NAPI-RS across both Node and Bun. Bun's N-API support is
   best-effort upstream, so our CI is the support contract.

---

## Correction 3: `fdir` Is Larger Than Estimated

### Original Claim
> `fdir` ~1.5k stars.

### Verified Data
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

### Verified Data
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

### Verified Data
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

### Verified Data
- **Latest:** v2.0.3, published February 11, 2025
- **Description:** "Drop-in replacement of Node.js's path module, ensures
  paths are normalized with slash `/`"
- **Status:** Active, modern ESM/TypeScript, no Node.js dependency

### Strategic Impact
Confirms `pathe` as the right choice for our string manipulation layer.
It's actively maintained, ESM-native, and has zero Node.js dependency
(important for Bun/Deno compatibility).

---

## Updated Download Rankings

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

## NAPI-RS Ecosystem Verification

### Confirmed
- NAPI-RS v3 is the current stable version
- Builds without `node-gyp` (pure Rust/JS toolchain)
- Multi-platform binary distribution is mature (platform-specific npm packages)
- Powers: SWC, Turbo, oxc, lightningcss, `node-rs/*`
- **No published standalone filesystem library exists** in the NAPI-RS ecosystem

### Bun Compatibility
- NAPI-RS works on Bun via standard Node-API ABI
- Bun's N-API support is **best-effort** upstream (per NAPI-RS docs)
- Package authors must test Bun explicitly — cannot assume compatibility
- `bun:ffi` is experimental; Node-API is the recommended stable route

---

## Decision Summary

| Correction | Impact |
|-----------|--------|
| tinyglobby = 186M downloads | We compete on fused pipeline, not raw glob |
| Bun.Glob.scan() = real traversal | Bun is an ally with fast primitives; we add composition |
| fdir = 37M downloads, 1.6k stars | JS ceiling is high; fused walk is the differentiator |
| fast-glob = 2.7k stars, 73M downloads | Entering maintenance; tinyglobby is the successor |
| fs-jetpack = dead 3+ years | Validates our positioning as modern successor |
| No NAPI-RS filesystem library | Confirms the gap is wide open |
