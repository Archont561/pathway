---
title: "Strategic Conclusions, Target Audience, Pitch, Bun-as-Ally"
domain: competitive
status: decided
created: 2025-07-11
updated: 2026-09-16
source: conversation
depends_on:
  - competitive/landscape
  - competitive/verified-data
  - architecture/fused-walk
  - features/killer-features
tags: [positioning, strategy, pitch, audience, bun, marketing]
---

# Strategic Positioning

## The Pitch

### What NOT to Say

> ❌ "pathlib for JavaScript."

This is useful as a mental hook but insufficient as a product pitch. `pathlib`
implies path manipulation, which is a commodity (see `pathe`). It undersells
the native performance story and the compositional API.

### What to Say

> ✅ **"A native, pathlib-inspired filesystem API for TypeScript, Bun, and Node."**

Then the features:
- Ergonomic `Path` objects with TypeScript generics
- Native recursive traversal (Rust `ignore` crate, ripgrep's engine)
- Fused walk: stat + hash + filter in a single native pass
- Glob and regex filtering with directory pruning
- Pluggable serialization (JSON, TOML, YAML, CBOR, MessagePack)
- Atomic writes with fsync
- Native content hashing (BLAKE3, xxHash, SHA-256)
- Scoped temp directories with OS-level cleanup
- Directory snapshots and diffing for incremental builds
- Safe path sandboxing (anti-traversal)
- Native file locking (`flock` / `LockFileEx`)
- Parallel bulk operations
- Transactional filesystem updates
- Async iteration with chunked batching

Now there's a legitimate reason to install it.

**Benchmark framing (added Sept 2026):** Since `node:fs.glob` is stable
and C++-native in Node core, we never claim to beat Node at globbing. The
claim is: *"Node can glob natively. Only @myorg/path fuses glob + stat +
hash + filter into a single native pass — 5–20× faster than
`fs.glob` + `fs.stat` + `crypto` on 100k+ file trees."* The moat is the
fusion, not the glob.

### The One-Liner for Build Tool Authors

> **"The filesystem library that build tools wish they had."**

---

## Target Audience (Priority Order)

### 1. Build Tool Authors (Primary)

**Who:** Vite, esbuild plugins, Turbopack, Rspack, Biome, Oxc, Rolldown,
Parcel, webpack plugin authors.

**Pain:** They currently stitch together `tinyglobby` + `node:fs` +
`node:crypto` + custom caching logic. Every build tool reimplements file
hashing, change detection, and incremental rebuild logic from scratch.

**What we give them:**
- Fused walk with BLAKE3 hashing in a single pass
- Directory snapshots and diffing for incremental builds
- Parallel file transformation and copying
- Content-addressable caching primitives

**Adoption trigger:** "Replace your 500-line file-walking-and-hashing utility
with `const diff = (await project.snapshot()).diff(cached)`."

### 2. Monorepo Infrastructure Teams (Primary)

**Who:** Nx, Turborepo, Lerna, Rush, Bazel JS rules, custom monorepo tooling.

**Pain:** Monorepo tools need to hash entire directory trees to determine
cache keys. This is currently the slowest part of the build pipeline.

**What we give them:**
- `project.hashTree({ glob, exclude, hasher })` — single call, native speed
- Snapshot persistence for cross-build caching
- Parallel file operations for task orchestration

**Adoption trigger:** "Your cache key computation goes from 2s to 150ms."

### 3. CLI Framework Authors (Secondary)

**Who:** Commander, Yargs, oclif, Clack, Ink, Effect CLI.

**Pain:** CLI tools need temp directories, config file read/write, file
locking for concurrent invocations.

**What we give them:**
- `Path.temp()` with OS-level cleanup
- `file.read(toml)` / `file.write(toml, config, { atomic: true })`
- `file.withLock()` for concurrent CLI safety

**Adoption trigger:** "Stop writing try/finally temp dir cleanup."

### 4. Server Framework Authors (Secondary)

**Who:** Express, Fastify, Hono, Elysia, Nitro, Fresh.

**Pain:** Static file serving has path traversal vulnerabilities. Config
loading is boilerplate. File uploads need temp handling.

**What we give them:**
- `project.sandbox("public")` — zero-cost anti-traversal
- `file.read(json, { validate: zodValidator(ConfigSchema) })`
- `Path.temp()` for upload staging

**Adoption trigger:** "Eliminate path traversal vulnerabilities by construction."

### 5. Application Developers (Tertiary)

**Who:** Anyone writing Node/Bun applications that touch the filesystem.

**Pain:** `fs-extra` is untyped. `node:fs` is verbose. Glob + read + parse
is boilerplate.

**What we give them:**
- A single import that replaces `fs-extra` + `glob` + `pathe` + `tmp`
- Type-safe config reading: `const config = await file.read<Config>(toml)`
- Async iterators instead of callback soup

**Adoption trigger:** "npm uninstall fs-extra glob fast-glob tmp proper-lockfile."

---

## Bun Strategy: Ally, Not Competitor

### The Reality

Bun ships with fast native filesystem primitives:
- `Bun.file()` — mmap-based reads
- `Bun.write()` — fast writes
- `Bun.Glob.scan()` — native Zig-based traversal
- `Bun.hash()` — native hashing (wyhash, murmur, city)

These are **fast** and **well-implemented**. We should not pretend they
don't exist or try to beat them at their own game.

### What Bun Lacks

Bun's primitives are **uncomposed**. They are building blocks, not a
framework:

| Need | Bun Primitive | Missing |
|------|--------------|---------|
| Typed config read | `Bun.file("x.json").json()` | No generics, no TOML/YAML, no validation |
| Fused walk+hash | `Bun.Glob.scan()` + `Bun.hash()` | Separate calls, no single-pass |
| Atomic write | `Bun.write()` | No temp+rename+fsync |
| Safe serving | `Bun.file()` | No sandbox/containment |
| Temp cleanup | `fs.mkdtemp()` | No RAII, no OS-level guarantee |
| File locking | — | Not available |
| Incremental builds | — | No snapshots/diff |

### Our Position

> **"We use Bun's speed. We add the composition."**

Our library runs on Bun via NAPI-RS (Node-API ABI). The Rust engine provides
the fused walk, serialization, and orchestration that Bun intentionally
doesn't ship. Bun's primitives are the floor; we're the ceiling.

**Update (Sept 2026):** Bun 1.4 (Aug 20, 2026) rewrote the runtime in Rust
and shipped a 2× faster `Bun.Glob.scan()`. The ally stance holds —
`scan()` still returns strings only — but the raw-speed gap has narrowed.
All "Bun floor" claims must be re-benchmarked on Bun 1.3.x **and** 1.4.x
before publication, and CI validates N-API on both (a runtime rewritten in
a new language can change addon behavior).

### Implementation

```ts
// Internal: detect Bun and use its fast primitives where appropriate
const isBun = typeof Bun !== "undefined";

class Path {
  async readText(): Promise<string> {
    if (isBun) {
      return Bun.file(this.value).text();  // Use Bun's mmap read
    }
    return fs.readFile(this.value, "utf-8");  // Node fallback
  }
}
```

But the **fused walk always uses Rust**, even on Bun, because `Bun.Glob.scan()`
doesn't provide stat+hash in a single pass.

---

## Competitive Moats

### Moat 1: The Fused Walk (Technical)

No JS library can match single-pass traverse+stat+hash because the JS
event loop requires a separate libuv round-trip for each `fs.stat()` and
`fs.readFile()` call. This is an **architectural limitation** of the
Node.js runtime, not an implementation detail that can be optimized away.

**Updated scope (Sept 2026):** this also applies to `node:fs.glob` —
Node core's native C++ glob is fast at *finding* files but still returns
paths/Dirents only; fusing stat+hash into the pass remains something only
an out-of-process engine (ours) can do. The moat is against the best
native baseline, not just pure JS.

Our Rust engine bypasses this by performing all I/O in OS worker threads
and returning fully populated batches across a single N-API boundary.

**Defensibility:** High. Requires native code. Cannot be replicated in JS.

### Moat 2: The Compositional API (Ergonomic)

No existing library combines path objects + traversal + serialization +
hashing + locking + temp dirs + sandboxing into a single, type-safe API.
Users currently need 5–7 dependencies to get this functionality.

**Defensibility:** Medium. Could be replicated by a motivated competitor,
but the design space is large and the integration work is significant.

### Moat 3: The Serializer Ecosystem (Network Effect)

Once we establish the `Serializer<T>` interface as a standard, third-party
codecs can plug in. The ecosystem grows without our direct involvement.

**Defensibility:** Medium-high. First-mover advantage in defining the
interface. Similar to how `express` middleware became a standard.

### Moat 4: The Benchmark Story (Marketing)

If our benchmarks show 10–20x speedup on real-world pipelines (walk+stat+hash
on 100k+ files), the numbers speak for themselves. Build tool authors are
performance-sensitive and will switch for measurable gains.

**Defensibility:** Low (anyone can benchmark), but the **results** are
defensible because they flow from the architectural advantage.

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| NAPI-RS distribution pain | Medium | High | Use `napi-rs/cli` tooling; proven by SWC/lightningcss |
| Bun N-API breakage | Medium | Medium | CI matrix includes Bun; **test both 1.3.x and 1.4.x (Rust-rewrite release, Aug 2026)** |
| Fused walk not fast enough vs `node:fs.glob` | Low | Critical | Benchmark before committing; ≥5x threshold measured against the native C++ baseline, not just JS |
| `tinyglobby` adds native backend | Low | High | Unlikely (JS ecosystem); our composition layer remains |
| Node core ships stat/hash fusion in `fs.glob` | Low | High | Watch Node tracking issues; composition layer + ecosystem remain the fallback |
| Bun ships compositional FS API | Low | Medium | Would take years; we establish ecosystem first |
| Serde→JS bridge too slow | Low | Medium | Fall back to JS parsing for JSON; native only for TOML/YAML |
| Legacy npm fails platform `optionalDependencies` | Low | Medium | `napi-postinstall` fallback + oldest-supported-npm install smoke test in CI |
| Effect Platform becomes the default typed-FS layer for Effect users | Medium | Medium | Fused-walk moat unaffected; ship an Effect adapter if adoption data supports it |
| Community doesn't adopt | Medium | Critical | Target build tool authors first; they influence the ecosystem |

---

## Success Metrics

### v0.1 (Proof of Concept)
- [ ] Fused walk benchmark ≥5x faster than `node:fs.glob` + `fs.stat` +
      `crypto` (and all JS incumbents)
- [ ] Works on Node 24 (LTS), Node 26 (Current), Bun 1.3.x, Bun 1.4.x
- [ ] CI passes on Linux, macOS, Windows
- [ ] `Path`, `walkFiles`, `read(json)`, `write(json)` all functional

### v0.5 (Early Adoption)
- [ ] 1,000+ GitHub stars
- [ ] 100k+ weekly npm downloads
- [ ] Adopted by at least 2 build tools or monorepo frameworks
- [ ] TOML and YAML native serializers published

### v1.0 (Ecosystem Replacement)
- [ ] 10,000+ GitHub stars
- [ ] 1M+ weekly npm downloads
- [ ] Recognized as the default filesystem library for new TypeScript projects
- [ ] `fs-extra` downloads declining in our favor

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| "Native pathlib-inspired FS API" pitch | Accurate, differentiated, not just "pathlib for JS" |
| Build tools as primary audience | Most performance-sensitive; most pain from current fragmentation |
| Bun as ally, not competitor | Bun has fast primitives but no composition; we add the layer |
| Fused walk as primary moat | Architectural advantage that cannot be replicated in JS |
| Benchmark-first development | Prove the performance story before expanding features |
| Serializer ecosystem as network effect | Third-party codecs grow the platform without our effort |
| Target fs-extra replacement | Largest addressable market; clear migration path |
