---
type: Architecture Decision
title: "Coarse-Grained FFI Boundary: What Stays JS, What Goes to Rust"
description: "Coarse-grained FFI policy: what stays in TypeScript, what crosses to Rust, and why per-call boundary hops are banned."
tags: [napi-rs, ffi, boundary, performance, wasm, bun]
status: stable
generated:
  by: pathway_kb/1.0
  at: 2026-09-16T00:00:00Z
verified:
  - by: human:archont561
    at: 2025-07-11T00:00:00Z
  - by: process:gap-analysis-2026-09
    at: 2026-09-16T00:00:00Z
domain: architecture
decision: decided  # legacy KB status (decided|proposed|deprecated)
created: 2025-07-11
source: conversation
depends_on:
  - architecture/core-layers
  - competitive/verified-data
---

# N-API Boundary Design

## The Cardinal Rule

> **Rust is not there to make `join()` faster.**
> It is there to make **large-scale filesystem operations** fast and
> memory-efficient.

This distinction is what makes the native dependency technically justified
rather than "Rust because Rust is faster."

---

## The Anti-Pattern: Per-Operation FFI Hops

**Do NOT do this:**

```ts
path.join("src")        // → Rust FFI call
  .join("components")   // → Rust FFI call
  .join("Button.tsx")   // → Rust FFI call
```

Each N-API crossing involves:
1. JS value marshaling (string → N-API string handle)
2. Thread-safe function dispatch
3. Rust-side `napi::Env` interaction
4. Return value unmarshaling (N-API string → JS string)

For a pure string concatenation that `pathe` handles in ~50ns of JS, the
N-API round-trip costs ~500–2000ns. You've made the operation **10–40x slower**
by "optimizing" it with Rust.

### The Same Applies To:

| Operation | JS Cost | N-API Overhead | Verdict |
|-----------|---------|----------------|---------|
| `join()` | ~50ns | ~1000ns | ❌ Stay in JS |
| `resolve()` | ~100ns | ~1000ns | ❌ Stay in JS |
| `relative()` | ~80ns | ~1000ns | ❌ Stay in JS |
| `dirname()` | ~30ns | ~1000ns | ❌ Stay in JS |
| `basename()` | ~30ns | ~1000ns | ❌ Stay in JS |
| `extname()` | ~20ns | ~1000ns | ❌ Stay in JS |
| `isAbsolute()` | ~10ns | ~1000ns | ❌ Stay in JS |
| `normalize()` | ~60ns | ~1000ns | ❌ Stay in JS |

All path string manipulation stays in TypeScript via `pathe`.

---

## What Crosses the Boundary

The native boundary is used **exclusively** for operations where Rust provides
measurable, significant value:

### Tier A: Bulk Operations (Primary Justification)

| Operation | Why Rust |
|-----------|----------|
| `walk()` / `walkFiles()` | Directory traversal with pruning, glob, regex — all in one pass |
| `hashTree()` | Parallel content hashing across thousands of files |
| `copyTo()` | Parallel recursive copy with filtering |
| `snapshot()` | Stat + hash entire directory trees |

These are the operations that justify the native dependency. A single
`walkFiles()` call with regex filtering and directory pruning on a 100k-file
tree would require 100k+ JS↔libuv round-trips in pure JS. The Rust engine
does it in a single N-API call with internal parallelism.

### Tier B: I/O-Intensive Single-File Operations

| Operation | Why Rust |
|-----------|----------|
| `read()` with native codec | Serde TOML/YAML/CBOR/MessagePack parsing |
| `write()` atomic | `write → fsync → rename` in a single syscall sequence |
| `hash()` | BLAKE3/xxHash on large files (memory-mapped I/O) |

These cross the boundary once per file, which is acceptable because the
bottleneck is the I/O and computation, not the FFI overhead.

### Tier C: OS-Level Primitives

| Operation | Why Rust |
|-----------|----------|
| `withLock()` | Direct `flock()` / `LockFileEx()` — no polling, no PID files |
| `temp()` | `O_TMPFILE` / `FILE_FLAG_DELETE_ON_CLOSE` guarantees |
| `watch()` | Native `inotify` / `FSEvents` / `ReadDirectoryChangesW` |

These require OS-level access that JS simply cannot provide without a native
addon.

---

## The Chunked Batch Pattern

For Tier A operations, results are returned in **batches**, not one-at-a-time:

```
Rust Engine
   ↓ (traverses 100k files in parallel)
   ↓ (filters, hashes, stats — all native)
   ↓
Yields batch of 512 PathEntry objects
   ↓ (single N-API boundary crossing)
TypeScript AsyncGenerator
   ↓ (unrolls batch into individual yields)
Consumer: for await (const file of ...)
```

This minimizes N-API crossings while providing a clean per-file API to the
consumer. The batch size (256–1024) is tunable and should be benchmarked.

See [fused-walk.md](/architecture/fused-walk.md) for the full traversal architecture.

### 2026 Update: Native Async Iterators (Experimental)

NAPI-RS now ships first-class iteration support (experimental, verified
Sept 2026): `#[napi(async_iterator)]` makes a Rust class implement JS
`Symbol.asyncIterator` natively, and `#[napi(iterator)]` covers sync
iteration. The pull-based model is "easier to cancel and bound than pushing
every item through an unbounded ThreadsafeFunction queue" (napi.rs docs),
and `return()` — invoked on early `break` — is the cancellation hook.

**Decision:** Phase 1 Step 0 runs a 1–2 day spike on `#[napi(async_iterator)]`
+ `AsyncTask` across Node 24/26 + Bun 1.3/1.4 before freezing the walker
architecture. If it passes the napi.rs test checklist (forced GC, early
break, overlapping `next()`, worker shutdown), the fused walk yields batches
lazily as Rust traverses — eliminating eager full-scan latency, bounding
memory to a prefetch window, and making `AbortSignal` cancellation native.
Chunked paging (above) remains the fallback if the spike fails.

Spike design constraints (from the docs): `Yield` must be `Send + 'static`
owned values (our `FusedEntry` qualifies — no scoped JS values); overlapping
`next()` calls are **not** serialized for us, so keep the cursor state
machine.

---

## Rejected Alternatives

### WASM / WASI Fallback

**Decision: Rejected for v0.1–v1.0.**

Rationale:
1. A filesystem package needs **real OS filesystem access**, not sandboxed
   WASM trying to reach the host filesystem through capability handles.
2. NAPI-RS supports WASI builds, but its own documentation explicitly warns
   against claiming Bun/Deno WASI support without runtime testing, and there
   are known compatibility gaps.
3. The performance profile is wrong: WASM adds overhead for the exact
   operations (syscalls, I/O) where we need native speed.
4. The target audience (build tools, server frameworks) runs on real OSes,
   not browsers.

**Revisit if:** A compelling edge-compute or browser-based use case emerges
with mature WASI filesystem support.

### `bun:ffi`

**Decision: Rejected.**

Rationale:
1. Bun itself currently describes `bun:ffi` as **experimental** and recommends
   Node-API as the more stable production route for native code.
2. Using `bun:ffi` would lock us to Bun, defeating the goal of a unified
   Node + Bun library.
3. NAPI-RS already provides near-native performance on Bun via the standard
   Node-API ABI.

**Architecture remains:**
```
Bun-specific package
      ↓
    napi-rs
      ↓
    Node-API (stable ABI)
```

### Bun 1.4 (Rust runtime rewrite, Aug 2026)

**Note, not a rejection:** Bun 1.4 (released Aug 20, 2026) rewrote Bun's
runtime in Rust and shipped a 2× faster `Bun.Glob.scan()` (plus Windows
ARM64 builds and Node 26.3 compatibility). The 2025 "Zig-based" assumptions
in this file are void. N-API behavior is re-verified on **both** Bun 1.3.x
and 1.4.x in CI (see
[ci-distribution.md](/implementation/ci-distribution.md)); all
Bun-comparison benchmarks must run on both lines.

### Neon (Alternative Rust Binding)

**Decision: Not evaluated, NAPI-RS chosen.**

NAPI-RS is the dominant Rust→Node binding in 2025. It powers SWC, Turbo,
oxc, lightningcss, and the `node-rs` ecosystem. Its tooling for multi-platform
binary distribution is mature. No reason to evaluate alternatives.

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| `pathe` for all string ops | 10–40x faster than FFI round-trip for pure string work |
| Coarse-grained FFI only | Bulk ops, native codecs, OS primitives — nothing else |
| Batched yields (512) / native async iterator (spike) | Minimize N-API crossings; pull-based native iterators (experimental, Sept 2026) enable true streaming + cancellation if the Phase-1 spike passes |
| Blocking work via `AsyncTask` | Per NAPI-RS decision table: libuv pool for blocking/CPU; avoids occupying a Tokio worker |
| No WASM | Filesystem needs real OS access; WASM sandbox is wrong model |
| No `bun:ffi` | Experimental; Node-API is stable and works on Bun |
| NAPI-RS v3 | Industry standard; proven by SWC, Turbo, oxc; iterator APIs experimental as of Sept 2026 |
