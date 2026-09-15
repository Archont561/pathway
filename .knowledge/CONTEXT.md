---
title: "Project Context and Decision Log"
domain: meta
status: active
created: 2025-07-11
updated: 2025-07-11
source: conversation
depends_on: []
tags: [context, decisions, roadmap]
---

# @myorg/path — Project Context

## Identity

A **native, pathlib-inspired filesystem API** for TypeScript, Bun, and Node.
Built on a 3-layer stack: ergonomic TS surface → NAPI-RS bridge → Rust engine.

**One-line pitch:**
> The filesystem library that build tools wish they had.

**Target audience:**
Build tool authors, monorepo infrastructure, CLI frameworks, server frameworks —
anyone currently stitching together `tinyglobby` + `fs-extra` + `pathe` +
`proper-lockfile` + `tmp` + custom hashing.

---

## Current State

**Phase:** Pre-implementation. Architecture designed, competitive landscape
verified, Phase 1 plan defined. No code written yet.

**Next action:** Generate `batch2.sh`–`batch6.sh` knowledge files, then begin
Phase 1 implementation (Rust walker + TS Path class + benchmark harness).

---

## Foundational Decisions

### D1: Three-layer architecture
- **Layer 1 — TypeScript API:** `Path` objects, `Serializer<T>`, async iterators.
  Feels "almost boring." Path string manipulation stays in JS via `pathe`.
- **Layer 2 — NAPI-RS bridge:** Coarse-grained. No per-`join()` FFI hops.
  Crosses boundary only for bulk operations (walk, hash, read, write, copy).
- **Layer 3 — Rust engine:** `ignore` crate (ripgrep's walker) + `globset` +
  `regex` + `blake3` + `serde`. Owns the `FileSystem` trait internally but
  does NOT expose it through N-API.

### D2: Rust is NOT for `join()`
Rust exists exclusively for **large-scale filesystem operations**: traversal,
pruning, content hashing, parallel I/O. Single-string path manipulation stays
in TypeScript. This is the technical justification for the native dependency.

### D3: The Fused Walk is the defensible moat
Competitors (`fdir`, `tinyglobby`, `Bun.Glob.scan()`) return paths only.
Real applications immediately stat, hash, and filter those paths — generating
thousands of additional JS↔C++ boundary crossings. Our Rust engine performs
traversal + stat + hash + filter in a **single syscall pass** across OS worker
threads, yielding fully populated batches. This is the benchmark we must prove.

### D4: Pluggable serializer pattern
`read(toml)` / `write(json, data)` with TypeScript generics. Serializers are
independent strategy objects (`parse`/`stringify`), not baked into `Path`.
Supports JS serializers and native Serde codecs interchangeably. No global
registration — registries are per-`FileSystem` instance.

### D5: Bun is an ally, not a competitor
`Bun.Glob.scan()` provides fast native traversal but lacks composition
(path objects, serialization, hashing, atomicity, sandboxing). We use NAPI-RS
across both Node and Bun. Bun's N-API support is best-effort upstream, so our
own CI matrix is the support contract.

### D6: Rejections
- **WASM/WASI:** Skipped. Filesystem packages need real OS access, not
  sandboxed WASM. NAPI-RS docs warn against claiming Bun/Deno WASI support.
- **`bun:ffi`:** Rejected. Bun describes it as experimental; Node-API is the
  stable production route.
- **Global serializer registry:** Rejected. `Path.register()` is mutable global
  state. Use per-instance registries via `FileSystem.create({ serializers })`.
- **Per-file FFI hops:** Rejected. `path.join("a").join("b").join("c")` must
  NOT cross the N-API boundary three times.

---

## Verified Market Data (July 2025)

| Library | Weekly Downloads | Stars | Notes |
|---------|-----------------|-------|-------|
| `tinyglobby` | **186M** | 523 | New ecosystem default. Uses `fdir` + `picomatch`. |
| `fast-glob` | 73M | 2,695 | Author recommends tinyglobby as replacement. |
| `fdir` | 37M | 1,674 | Fastest JS crawler. ~1M files in <1s. |
| `pathe` | — | ~2.5k | Pure path strings. No I/O. Used by Vite/Nuxt. |
| `fs-extra` | — | ~9.5k | Legacy workhorse. No TS generics, no native speed. |
| `fs-jetpack` | — | ~1k | Closest philosophical match. **Unmaintained 3+ years.** |
| `Bun.Glob` | — | — | Native `scan()` traversal + `match()`. No composition. |

**Key gap confirmed:** No published library combines path objects + native
traversal + pluggable serialization + TypeScript generics. Turbo and oxc have
internal Rust filesystem engines but don't publish them as libraries.

---

## Phase Roadmap

| Phase | Scope | Key Deliverable |
|-------|-------|-----------------|
| **v0.1** | `Path`, `walk`, `read/write`, `json` serializer, `exclude` | Prove architecture. Benchmark vs fdir/Bun. |
| **v0.2** | `temp`, `hash`, `snapshot/diff`, `blake3`/`xxhash` | Build system adoption. |
| **v0.3** | `sandbox`, `withLock`, `copyTo` (parallel), `transform` | Server & infra adoption. |
| **v0.4** | `transaction`, `detect`, native TOML/YAML serializers | Power users, monorepos. |
| **v1.0** | `resolve`, `watch`, streaming transforms, full CI matrix | Ecosystem replacement. |

---

## Open Questions

1. **NAPI-RS v3 streaming:** Has `AsyncGenerator` binding stabilized, or is
   explicit chunked paging still the cleanest pattern?
2. **Bun N-API edge cases:** Any recent quirks with `tokio::task::spawn_blocking`
   inside Bun's runtime?
3. **Batch size tuning:** Optimal chunk size for the fused walk (256? 512? 1024?)
   — needs empirical benchmarking.
4. **Serde→JS bridge for JSON:** Is native `serde_json` → N-API object creation
   actually faster than V8/JSC `JSON.parse()` for small files? Likely not —
   reserve native codecs for TOML/YAML/MessagePack.

---

## File Conventions

All knowledge files use YAML frontmatter:

```yaml
---
title: "Human-readable title"
domain: architecture | features | competitive | implementation | meta
status: decided | proposed | pending | deprecated
created: YYYY-MM-DD
updated: YYYY-MM-DD
source: conversation | web-search | benchmark
depends_on:
  - domain/filename       # without .md extension
tags: [tag1, tag2]
---
```

Navigate via [INDEX.md](./INDEX.md).
