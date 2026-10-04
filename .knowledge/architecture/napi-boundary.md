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
| `copyTo()` | Planned native optimization; current API uses a bounded TypeScript worker pool with filtering |
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

**Decision (frozen 2026-10-03, task-1): chunked paging over `AsyncTask`.**
The spike ran; the record below is the evidence. The iterator was not
disqualified — it passed every checklist item on every runtime available —
it was *unnecessary*: at the default batch size the two transports are
within 8% of each other, and paging maps 1:1 onto the core's pull model
(`scan` / `next_batch` / `cancel`) without taking a dependency on a surface
napi-rs itself labels "experimental … not yet stable" in its rustdoc.

Spike design constraints (from the docs, confirmed in practice): `Yield`
must be `Send + 'static` owned values (our `FusedEntry` qualifies — no
scoped JS values); overlapping `next()` calls are **not** serialized for us,
so the cursor state machine stays.

---

## Phase 1 Step 0 Spike Record (2026-10-03, task-1)

A throwaway cdylib (napi 3.13.0 / napi-derive 3.6.9, the locked workspace
versions) exposed the same payload — batches of `{value, isDir, size}`
objects, the realistic marshaling shape — over both transports:
`#[napi(async_iterator)]` (`AsyncGenerator` with `next`/`complete`/`catch`)
and a `BatchPager` class paging through `AsyncTask`, mirroring
`NativeScanner`. The crate was deleted after the measurements; this section
is its surviving output.

### Runtime matrix

| Runtime | Checklist | Worker shutdown | Source |
|---------|-----------|-----------------|--------|
| Node 22.22.3 | 11/11 pass | survives | pixi env (conda-forge) |
| Bun 1.3.11 | 11/11 pass | **segfault** (see below) | pixi env |
| Bun 1.4.2 | 11/11 pass | survives | npm (`bun@1.4.2`) |
| Node 24 / Node 26 | **not run** | **not run** | unavailable: the sandbox egress proxy blocks `nodejs.org`, and the conda env pins Node 22 |

The Node 24/26 legs are a recorded gap, not a waiver: they must run in CI
(`ci-distribution.md` matrix) before the first publish. The gap is also an
argument *for* the frozen decision — `AsyncTask` is the decade-old Node-API
mechanism, so the untested legs carry less unknown risk than they would
under an experimental surface.

### The napi.rs iterator checklist (all three runtimes)

`next()` with and without its argument (undefined arrives as `None`, a value
as `Some`); natural completion and `next()` after completion (stays
`{done: true}`); early `break` (invokes `complete()` exactly once); explicit
`return(value)`; cleanup failure (`complete()` erroring rejects the
`return()` without crashing); default `throw(error)` (rejects with the
thrown value) and recovered `throw` (a `catch()` that yields resumes
iteration); two overlapping `next()` calls (both resolve, no duplicated
cursor); dropping the original class while retaining only its iterator
(napi-rs pins the instance via `[[InstanceRef]]`, napi-rs#3119); forced GC
mid-iteration. 11/11 on Node 22.22.3, Bun 1.3.11 and Bun 1.4.2.

### Worker-environment shutdown: the Bun 1.3 segfault

Terminating a `worker_threads` Worker crashes **Bun 1.3.11** with
`panic: Segmentation fault` — and the controls show it is not the
iterator's fault: the same crash reproduces with the `AsyncTask` pager
in flight, and with the addon merely `dlopen`ed and idle. Any napi addon
loaded in a terminated worker brings the Bun 1.3.11 process down. Bun 1.4.2
and Node 22 survive all three variants.

Consequence: this is a **platform note, not a transport discriminator** —
it changes nothing about the decision but it does mean "Bun 1.3/1.4
supported through the same ABI" (README) needs a caveat for addon-in-worker
use on 1.3, and the CI matrix must keep the worker-shutdown case.

### Measurements (100 000 entries, median of 5)

| Transport | Batch | Node 22.22.3 | Bun 1.3.11 | Bun 1.4.2 |
|-----------|------:|-------------:|-----------:|----------:|
| `async_iterator` | 64 | 184.6 ms | 161.3 ms | 152.6 ms |
| `AsyncTask` page | 64 | 207.4 ms | 161.3 ms | 170.3 ms |
| `async_iterator` | 512 | 105.3 ms | 92.3 ms | 79.0 ms |
| `AsyncTask` page | 512 | 110.0 ms | 90.2 ms | 82.9 ms |
| `async_iterator` | 4096 | 84.7 ms | 80.6 ms | 65.0 ms |
| `AsyncTask` page | 4096 | 89.3 ms | 78.8 ms | 75.8 ms |

What the numbers say: **batch size dominates, transport does not.** Going
from 64 to 4096 entries per crossing roughly halves wall time on every
runtime; swapping transport at a fixed batch size moves it by -2% to +12%,
and at the default 512 the gap is ≤4.5% everywhere. The iterator's only
consistent win (Bun 1.4.2 at 4096, 14%) is at a batch size large enough
that the crossing count (26) stopped mattering.

### `ThreadsafeFunction` scheduling, Bun 1.3 vs 1.4

50 000 nonblocking calls fired from a plain OS thread, drained on the JS
thread (the streaming-iterator path's push mechanism):

| Runtime | Drain | Rate | Out-of-order | Max 1k-gap |
|---------|------:|-----:|-------------:|-----------:|
| Node 22.22.3 | 34.2 ms | 1.46 M/s | 0 | 1.4 ms |
| Bun 1.3.11 | 26.5 ms | 1.88 M/s | 0 | 1.1 ms |
| Bun 1.4.2 | 24.1 ms | 2.07 M/s | 0 | 1.7 ms |

No reordering, no starvation, no coalescing anomalies on either Bun line;
the 1.4 Rust rewrite is slightly faster with marginally burstier gaps.
Nothing here blocks a TSFN-based path, but nothing demands one either.

### The frozen decision

**Chunked paging over `AsyncTask`**, because:

1. **The performance difference does not pay for the risk.** ≤4.5% at the
   batch size that ships; the knob that matters (batch size) exists in both
   designs.
2. **Stability asymmetry.** napi-rs's own rustdoc calls `AsyncGenerator`
   "experimental … not yet stable"; `AsyncTask` is the mechanism Node-API
   has had for years, and the untested Node 24/26 legs inherit that
   asymmetry.
3. **Shape match.** `NativeScanner` already is a pager (`scan`,
   `next_batch`, `cancel`); paging keeps `crates/engine` glue-only (D7),
   while the iterator would add a second state machine (`complete`/`catch`
   semantics) the TypeScript surface never exposes — `walk()` yields
   `WalkBatch`es either way, which is exactly why the API was frozen before
   the spike.

**Revisit when** napi-rs stabilizes `async_iterator` (drops the
experimental label) *and* a measured workload shows per-entry streaming —
not batch paging — is the bottleneck. The checklist above is the regression
suite for that future spike.

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
| Batched yields (512) over `AsyncTask` chunked paging | Spike-measured (2026-10-03): transport moves ≤4.5% at batch 512 while batch size dominates; `async_iterator` stays experimental upstream — revisit on stabilization |
| Blocking work via `AsyncTask` | Per NAPI-RS decision table: libuv pool for blocking/CPU; avoids occupying a Tokio worker |
| No WASM | Filesystem needs real OS access; WASM sandbox is wrong model |
| No `bun:ffi` | Experimental; Node-API is stable and works on Bun |
| NAPI-RS v3 | Industry standard; proven by SWC, Turbo, oxc; iterator APIs experimental as of Sept 2026 |
