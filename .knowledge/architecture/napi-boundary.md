---
title: "Coarse-Grained FFI Boundary: What Stays JS, What Goes to Rust"
domain: architecture
status: decided
created: 2025-07-11
updated: 2025-07-11
source: conversation
depends_on:
  - architecture/core-layers
  - competitive/verified-data
tags: [napi-rs, ffi, boundary, performance, wasm, bun]
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

See [fused-walk.md](./fused-walk.md) for the full traversal architecture.

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
| Batched yields (512) | Minimize N-API crossings while keeping clean async iterator API |
| No WASM | Filesystem needs real OS access; WASM sandbox is wrong model |
| No `bun:ffi` | Experimental; Node-API is stable and works on Bun |
| NAPI-RS v3 | Industry standard; proven by SWC, Turbo, oxc |
