---
title: "Competitive Landscape: Tiers 1–5, Full Library Map, Gap Matrix"
domain: competitive
status: decided
created: 2025-07-11
updated: 2025-07-11
source: conversation
depends_on:
  - meta/CONTEXT
  - features/walk-traversal
  - features/serializers
  - features/killer-features
tags: [competitive, landscape, gap-matrix, fdir, tinyglobby, fs-extra, pathe, bun]
---

# Competitive Landscape

## Overview

The JS/TS filesystem library ecosystem is **fragmented by concern** and
**shallow by implementation**. Every library solves one narrow problem
(path strings, or glob matching, or JSON read/write, or file locking) in
pure JavaScript. No library combines them into a unified, native-speed,
type-safe API.

This document maps the entire landscape across five tiers, identifies the
exact gap we occupy, and provides the feature matrix that justifies the
project's existence.

---

## Tier 1: Direct Competitors (Ergonomic FS Wrappers)

Libraries that attempt to provide a higher-level filesystem API on top of
`node:fs`. These are the closest philosophical matches.

### `pathe`

| Attribute | Value |
|-----------|-------|
| **Language** | TypeScript (pure JS, ESM) |
| **Stars** | ~2,500 |
| **Latest** | v2.0.3 (Feb 2025) |
| **Downloads** | High (used by Vite, Nuxt, UnJS ecosystem) |
| **Status** | Active |

**What it does:**
- Cross-platform path normalization (`join`, `resolve`, `relative`, etc.)
- Drop-in replacement for Node.js `path` module
- Ensures paths are normalized with forward slashes `/` on all platforms
- Zero dependencies, no Node.js dependency

**What it lacks:**
- **No I/O at all.** Pure string manipulation. No `read`, `write`, `walk`,
  `stat`, `mkdir`, `remove`.
- No serialization. No hashing. No locking. No async operations.
- No `Path` object — exports plain functions.

**Relationship to us:** Complementary, not competitive. We **use `pathe`
internally** for all path string manipulation (see
[napi-boundary.md](../architecture/napi-boundary.md)). `pathe` handles the
string layer; we handle the I/O layer.

---

### `fs-extra`

| Attribute | Value |
|-----------|-------|
| **Language** | JavaScript |
| **Stars** | ~9,500 |
| **Latest** | v11.x |
| **Downloads** | Very high (legacy workhorse) |
| **Status** | Maintenance mode |

**What it does:**
- `copy`, `copySync` (recursive directory copy)
- `move`, `moveSync`
- `ensureDir`, `ensureDirSync` (mkdir -p)
- `readJson`, `writeJson`, `outputJson` (JSON read/write shorthand)
- `emptyDir`, `remove` (recursive delete)
- `pathExists`

**What it lacks:**
- No TypeScript generics (types are `any`-heavy)
- No glob or regex walk
- No atomic writes
- No native speed (pure JS, `node:fs` wrapper)
- No pluggable serializers (JSON only, hardcoded)
- No async iterators
- No content hashing
- No file locking
- Callback-era API design (promisified later)

**Relationship to us:** The legacy incumbent we aim to replace. Users who
currently `npm install fs-extra` for `readJson`/`writeJson`/`copy`/`ensureDir`
should be able to switch to `@myorg/path` and get the same functionality
plus native speed, type safety, and a much richer API.

---

### `fs-jetpack`

| Attribute | Value |
|-----------|-------|
| **Language** | JavaScript |
| **Stars** | ~1,000 |
| **Latest** | v5.1.0 (**3+ years ago**) |
| **Downloads** | Low |
| **Status** | **Effectively unmaintained** |

**What it does:**
- Fluent API: `jetpack.read("file", "json")`, `jetpack.write("file", data)`
- `jetpack.find("*.ts")` (glob-based search)
- `jetpack.copy`, `jetpack.move`, `jetpack.remove`
- `jetpack.inspect()` (stat wrapper)
- `jetpack.dir()` (ensure directory)

**What it lacks:**
- Pure JS, no native speed
- No pluggable serializers (hardcoded JSON/YAML)
- No async iterators
- No TypeScript generics
- No atomic writes
- No content hashing
- No maintenance (last release 3+ years ago)

**TypeScript fork (`fs-jetpack-ts`):** Dead since Feb 2017. 0 stars, 0
dependents, 2 total releases.

**Relationship to us:** The **closest philosophical match** — fluent path
objects with read/write/find. But it's abandoned and pure JS. We are the
modern, maintained, native-speed successor to the idea `fs-jetpack` pioneered.

---

### `typedfs`

| Attribute | Value |
|-----------|-------|
| **Language** | TypeScript |
| **Stars** | ~50 |
| **Status** | Niche, low adoption |

**What it does:**
- Typed filesystem with schema validation on read/write
- `TypedFs.readJson<T>(path, schema)`

**What it lacks:**
- Tiny ecosystem, no community
- Pure JS, no native speed
- No traversal, no glob, no walk
- No atomic writes, no locking

**Relationship to us:** Validates the demand for typed filesystem operations.
We subsume this with `read<T>(serializer, { validate })`.

---

### `file-system-cache`

| Attribute | Value |
|-----------|-------|
| **Language** | TypeScript |
| **Stars** | ~200 |
| **Status** | Narrow scope |

**What it does:**
- Read/write with automatic JSON serialization
- File locking for cache consistency

**What it lacks:**
- Narrow scope (caching only)
- No walk, no glob, no traversal
- No native speed

**Relationship to us:** Validates the demand for serialization + locking.
We provide both as composable features.

---

## Tier 2: Traversal / Glob Engines

Libraries focused exclusively on finding files. This is the most competitive
tier and the one where our fused-walk architecture must prove its value.

### `tinyglobby`

| Attribute | Value |
|-----------|-------|
| **Language** | TypeScript (pure JS) |
| **Stars** | ~523 |
| **Weekly Downloads** | **186 million** |
| **Dependencies** | 2 (`fdir` + `picomatch`) |
| **Status** | **New ecosystem default** |

**What it does:**
- Drop-in replacement for `globby` and `fast-glob`
- Glob pattern matching with negation support
- Uses `fdir` for traversal, `picomatch` for pattern matching
- Recommended as replacement by the `fast-glob` author himself
- Used by Vite 6+ and growing rapidly

**What it lacks:**
- Pure JS, hits V8/libuv ceiling on large trees
- No regex filtering (glob only)
- No content hashing
- No metadata (stat) in results
- No serialization, no atomic writes
- No directory pruning before descent (relies on `fdir`'s filtering)
- No fused walk (returns paths only, not stat+hash)

**Relationship to us:** The primary traversal competitor. Our benchmark
must beat `tinyglobby` + post-traversal stat/hash pipeline by ≥5x on the
**full fused pipeline** (not just raw traversal). See
[fused-walk.md](../architecture/fused-walk.md).

---

### `fast-glob`

| Attribute | Value |
|-----------|-------|
| **Language** | TypeScript (pure JS) |
| **Stars** | ~2,695 |
| **Weekly Downloads** | **73 million** |
| **Dependencies** | 17 |
| **Status** | Mature, author recommends tinyglobby |

**What it does:**
- `fg.sync()`, `fg.async()`, `fg.stream()`
- Glob matching with brace expansion, extglobs, negation
- `onlyFiles`, `onlyDirectories` options
- `deep` (max depth) option
- `ignore` patterns

**What it lacks:**
- Pure JS
- No regex filtering
- No content hashing or metadata
- 17 dependencies (vs tinyglobby's 2)
- Author now recommends tinyglobby as replacement

**Relationship to us:** Legacy traversal standard being displaced by
tinyglobby. We compete with both.

---

### `fdir`

| Attribute | Value |
|-----------|-------|
| **Language** | TypeScript (pure JS) |
| **Stars** | ~1,674 |
| **Weekly Downloads** | **37 million** |
| **Status** | Active, fastest JS crawler |

**What it does:**
- Claims fastest directory crawler in Node.js
- ~1 million files in under 1 second
- `fdir().glob("**/*.ts").crawl(root)`
- `withFullPaths()`, `withRelativePaths()`, `withDirs()`
- `filter()` callback for custom filtering
- `withPathSeparator()` for cross-platform normalization
- Powers `tinyglobby` internally

**What it lacks:**
- Pure JS (micro-optimized but still V8/libuv bound)
- No regex filtering (glob via picomatch only)
- No content hashing
- No metadata in default output (can include `Dirent` with `withStats()`)
- No serialization, no atomic writes
- No fused walk

**Relationship to us:** The JS traversal ceiling. `fdir` is as fast as pure
JS can get. Our Rust engine must beat it on the **full pipeline** (traverse
+ stat + hash), not on raw traversal alone where the gap may be narrow.

---

### `globby`

| Attribute | Value |
|-----------|-------|
| **Language** | TypeScript (pure JS) |
| **Stars** | ~3,000 |
| **Weekly Downloads** | ~40 million |
| **Dependencies** | 23 |
| **Status** | Mature, being displaced by tinyglobby |

**What it does:**
- Wraps `fast-glob` with gitignore support
- Negation patterns, `expandDirectories`
- `gitignore: true` option

**What it lacks:**
- Pure JS, slower than `fast-glob` and `fdir`
- 23 dependencies
- No regex, no hashing, no metadata, no serialization

**Relationship to us:** Being displaced by tinyglobby. Not a primary
competitor going forward.

---

### `readdirp`

| Attribute | Value |
|-----------|-------|
| **Language** | JavaScript |
| **Stars** | ~1,000 |
| **Status** | Maintenance mode |

**What it does:**
- Streaming `readdir` with filtering
- `fileFilter`, `directoryFilter`, `type` options
- Returns `ReadableStream` of entry objects

**What it lacks:**
- Pure JS
- No glob matching
- No regex filtering
- No metadata batching
- No hashing

**Relationship to us:** Older streaming approach. Our async iterator API
provides the same streaming semantics with native speed.

---

## Tier 3: Rust-Backed Native Tools (NAPI-RS / Neon)

Tools that use Rust via NAPI-RS for performance-critical operations. None
of these are standalone filesystem libraries, but they prove the architecture
and contain internal filesystem engines we can learn from.

### `turbo` (Vercel)

| Attribute | Value |
|-----------|-------|
| **Binding** | NAPI-RS (partial) |
| **What it does** | Monorepo build system with Rust file hashing and caching |

**Relevance:** Turbo's Rust core does exactly the kind of file hashing +
traversal we're proposing — but it's embedded in a build tool, not exposed
as a library. **Closest real-world analog to our fused walk.**

### `oxc` (Oxidation Compiler)

| Attribute | Value |
|-----------|-------|
| **Binding** | NAPI-RS |
| **What it does** | Rust-based linter/parser for JS/TS |

**Relevance:** Uses the `ignore` crate for filesystem traversal. Proves that
`ignore` + NAPI-RS is a viable architecture for JS tooling.

### `swc`

| Attribute | Value |
|-----------|-------|
| **Binding** | NAPI-RS |
| **What it does** | Rust-based JS/TS compiler |

**Relevance:** Proves the NAPI-RS performance model at scale. Millions of
users, complex Rust↔JS data marshaling.

### `@parcel/watcher`

| Attribute | Value |
|-----------|-------|
| **Binding** | N-API (C++) |
| **What it does** | Native filesystem watcher |

**Relevance:** Shows native filesystem watchers work via N-API. Used by
Parcel and VS Code. We'd compete here in v1.0+ with `Path.watch()`.

### `lightningcss`

| Attribute | Value |
|-----------|-------|
| **Binding** | NAPI-RS |
| **What it does** | Rust CSS transformer |

**Relevance:** Proof of the NAPI-RS multi-platform distribution model
(platform-specific npm packages).

### `node-rs` packages

| Package | What it does |
|---------|-------------|
| `@node-rs/xxhash` | xxHash hashing |
| `@node-rs/crc` | CRC checksums |
| `@node-rs/bcrypt` | Password hashing |
| `@node-rs/jieba` | Chinese text segmentation |

**Relevance:** Proves NAPI-RS for compute-heavy utilities. But **none are
filesystem libraries**. The gap is wide open.

---

## Tier 4: Bun's Native Capabilities

Bun ships with native filesystem primitives that overlap with parts of our
API. Understanding these is critical for positioning.

| Feature | Bun Built-in | Our Value-Add |
|---------|-------------|---------------|
| `Bun.file()` | ✅ Fast reads via `mmap` | Path objects, serializers, atomic writes |
| `Bun.write()` | ✅ Fast writes | Atomic writes, pluggable serialization |
| `Bun.Glob` | ✅ Native glob matching + `scan()` traversal | Regex, fused walk (stat+hash), exclude pruning |
| `Bun.hash` | ✅ Native hashing (wyhash, murmur, city) | Tree hashing, BLAKE3, streaming |
| `node:fs` compat | ✅ Full Node compat | Same limitations as Node `fs` |
| `node:fs.glob` | ✅ Node compat glob | Fused walk, serialization, sandboxing |

**Key insight:** Bun has **fast primitives** but **no compositional API**.
`Bun.file("x.json").json()` is nice but doesn't scale to "walk 100k files,
hash them, filter by regex, and write transformed output atomically." Our
library provides the orchestration layer that Bun intentionally doesn't ship.

---

## Tier 5: Rust Ecosystem (Internal Building Blocks)

The Rust crates we'll use internally. Not competitors — they're our
foundation.

| Crate | What it does | Our usage |
|-------|-------------|-----------|
| `ignore` | Parallel traversal with gitignore (powers ripgrep) | Primary walker |
| `walkdir` | Simple recursive walking | Simpler alternative |
| `jwalk` | Parallel walkdir via rayon | Max parallelism option |
| `globset` | Glob pattern compilation and matching | Glob filtering |
| `regex` | Rust regex engine | Regex filtering |
| `serde` + `serde_json` | Serialization framework | JSON codec (if native) |
| `toml` | TOML parser via Serde | Native TOML codec |
| `serde_yaml` | YAML parser via Serde | Native YAML codec |
| `blake3` | Fastest cryptographic hash | Content hashing |
| `xxhash-rust` | Fastest non-crypto hash | Build cache hashing |
| `tempfile` | Secure temp files with OS cleanup | `Path.temp()` |
| `fs2` / `fd-lock` | Cross-platform file locking | `Path.withLock()` |
| `notify` | Cross-platform filesystem watcher | `Path.watch()` |
| `infer` | MIME type from magic bytes | `Path.detect()` |
| `flate2` | gzip/zlib compression | Transformer codec |
| `encoding_rs` | Character encoding detection | Encoding detector |

---

## The Gap Matrix

This is the definitive comparison. **No existing library fills more than
a fraction of the cells.**

| Capability | `pathe` | `fs-extra` | `fs-jetpack` | `fast-glob` | `fdir` | `tinyglobby` | `Bun` | **@myorg/path** |
|-----------|---------|-----------|-------------|------------|--------|-------------|-------|----------------|
| Path objects | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Typed generics | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Read/Write | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ |
| Pluggable serializers | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Glob walk | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Regex walk | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Directory pruning | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ⚠️ | ✅ (native) |
| Native Rust speed | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚡ | ⚡ |
| Fused walk (stat+hash) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Content hashing | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ (single) | ✅ (tree) |
| Atomic writes | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Temp dirs (RAII) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| File locking | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Snapshots/diff | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Sandbox/containment | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Transactions | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Parallel bulk ops | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| Async iterators | ❌ | ❌ | ❌ | ✅ (stream) | ✅ | ✅ | ✅ | ✅ |
| Watch | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ (v1.0) |

---

## Key Findings

1. **The market gap is real and large.** No library occupies more than 4–5
   cells in the gap matrix. We target 18+.

2. **The traversal space is consolidating.** `tinyglobby` (186M downloads)
   is becoming infrastructure. We don't compete on raw glob matching — we
   compete on the **fused pipeline** that comes after the glob.

3. **NAPI-RS filesystem gap is wide open.** Every NAPI-RS project found
   involves compute (hashing, parsing, compiling), not filesystem
   orchestration. Turbo and oxc have internal Rust FS engines but don't
   publish them.

4. **`fs-jetpack` is dead.** The closest philosophical match (fluent path
   objects + read/write/find) hasn't been updated in 3+ years. The idea
   is validated; the implementation is abandoned.

5. **Bun is an ally.** Bun's native primitives are fast but uncomposed.
   We provide the orchestration layer that makes Bun's speed usable for
   complex workflows.

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Use `pathe`, don't compete | Complementary; handles string ops we don't want in Rust |
| Replace `fs-extra` | Legacy incumbent; we subsume its API with better types and speed |
| Succeed `fs-jetpack` | Closest philosophical match; abandoned; we're the modern successor |
| Beat `tinyglobby` on fused pipeline | Not on raw glob — on walk+stat+hash combined |
| Learn from Turbo/oxc | They prove the architecture internally; we productize it |
| Complement Bun | Use Bun's speed; add the composition layer it lacks |
| Build on Rust crate ecosystem | `ignore`, `blake3`, `serde` are battle-tested foundations |
