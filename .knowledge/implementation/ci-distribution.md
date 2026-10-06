---
type: Implementation Spec
title: "NAPI-RS Targets, Platform Matrix, Bun CI, Distribution Hardening"
description: "NAPI-RS target matrix, GitHub Actions CI, Bun 1.3/1.4 and optional Deno legs, provenance and distribution hardening."
tags: [ci, napi-rs, distribution, platform, bun, targets, github-actions, provenance]
status: stable
generated:
  by: pathway_kb/1.0
  at: 2026-10-06T00:00:00Z
verified:
  - by: human:archont561
    at: 2025-07-11T00:00:00Z
  - by: process:gap-analysis-2026-09
    at: 2026-09-16T00:00:00Z
  - by: process:runtime-matrix-task-11
    at: 2026-10-05T22:33:54Z
domain: implementation
decision: decided  # legacy KB status (decided|proposed|deprecated)
created: 2025-07-11
source: conversation
depends_on:
  - architecture/napi-boundary
  - architecture/rust-crate-surface
  - implementation/repo-structure
  - competitive/verified-data
---

# CI & Distribution

## NAPI-RS Distribution Model

**Current Phase 1 model:** `@archont561/pathway` is one npm package. The
TypeScript build writes `dist/index.js` and the NAPI build writes the
platform-specific `.node` file into the same `packages/path/dist/` directory.
`package.json` publishes only `dist/`, so `npm pack ./packages/path` and a
clean install exercise the same artifact that the loader consumes. There is
no checked-in platform-package list and no generated optional-dependency graph
yet.

The hand-written loader in `packages/path/src/binding.ts` calculates the
NAPI-RS filename from `process.platform`, `process.arch`, and glibc/musl
availability, then validates the engine and Node-API versions. This is the
source-build contract; it is tested with Node smoke tests and the Bun suite.

**Release-time direction:** `napi publish` may split artifacts into
platform-specific optional-dependency packages once the release workflow is
implemented. That future packaging plan must not be documented as current
support or used as a substitute for the packed-install smoke test below.

### Published Crates (crates.io — D7, verified 2026-10-05)

Per [rust-crate-surface.md](/architecture/rust-crate-surface.md), the Rust
surface is distributed through crates.io, on an **independent semver cadence**
from the npm package. The names `pathway-fs` and `pathway-fs-core` are reserved
and their manifests have `publish = true`; `pathway-fs-engine` remains glue
only with `publish = false`.

```
pathway-fs-core                      ← rlib engine core (napi-free)
pathway-fs                           ← ergonomic pathlib-like Rust API (preview)
pathway-fs-engine                    ← NEVER published (cdylib napi glue)
```

`cargo publish --dry-run` for the two crates is a separate release gate. The
crates are named and publishable, but no first release is claimed here.

> **Install smoke requirement:** npm pack metadata, the packed tarball, and a
> clean install must be checked on Linux, macOS, and Windows. The runtime
> workflow performs this check with Node 24 and `npm publish --dry-run
> --provenance`; it does not claim a registry release.

---

## Initial Platform Support (v0.1)

### Tier 1 (Launch Targets)

| Target | Triple | Use Case |
|--------|--------|----------|
| Linux x64 glibc | `x86_64-unknown-linux-gnu` | CI servers, cloud, Docker |
| Linux arm64 glibc | `aarch64-unknown-linux-gnu` | AWS Graviton, Raspberry Pi |
| Linux x64 musl | `x86_64-unknown-linux-musl` | Alpine Linux, minimal Docker |
| macOS arm64 | `aarch64-apple-darwin` | Apple Silicon (M1/M2/M3/M4) |
| macOS x64 | `x86_64-apple-darwin` | Intel Macs (declining but present) |
| Windows x64 | `x86_64-pc-windows-msvc` | Windows development |

### Tier 2 (v0.4+)

| Target | Triple | Use Case |
|--------|--------|----------|
| Windows arm64 | `aarch64-pc-windows-msvc` | Surface Pro X, ARM Windows |
| Linux arm64 musl | `aarch64-unknown-linux-musl` | Alpine on ARM |

> **Cross-compilation (added Sept 2026):** the 2025 release workflow built
> every Linux target on x64 `ubuntu-latest` runners, which **cannot**
> produce `aarch64-unknown-linux-gnu` or musl artifacts. The release job
> must use a cross toolchain (`cross`/`zigbuild` for gnu-arm64,
> `musl-cross`/zig for musl targets) or dedicated runners. Windows arm64
> has no GitHub Actions runner — cross-build from Windows x64 or ship
> Tier 2.

### Explicitly NOT Supported

| Target | Reason |
|--------|--------|
| WASM/WASI | Filesystem needs real OS access; see [napi-boundary.md](/architecture/napi-boundary.md) |
| FreeBSD | Low demand; can be added if community requests |
| 32-bit (x86, armv7) | Node.js dropped 32-bit support |

---

## Runtime Support Matrix (updated 2026-10-05)

### Required Phase 1 matrix

The executable workflow is
[`.github/workflows/runtime-matrix.yml`](../../.github/workflows/runtime-matrix.yml).
It deliberately uses native GitHub-hosted runners rather than treating the
knowledge-base table as evidence:

| Operating system | Node | Bun | Checks |
|------------------|------|-----|--------|
| Ubuntu | 24, 26 | 1.3.11, 1.4.2 | build; Node smoke or Bun suite |
| macOS | 24, 26 | 1.3.11, 1.4.2 | build; Node smoke or Bun suite |
| Windows | 24, 26 | 1.3.11, 1.4.2 | build; Node smoke or Bun suite |

Every runtime row installs with Bun, builds the native addon into `dist/`,
and then runs the suite for the selected runtime. The three OS-specific
install-smoke jobs additionally pack `packages/path`, run
`npm publish --dry-run --provenance`, install that tarball into a clean
prefix, and import it with Node. This is intentionally a package-level smoke
test until platform-specific `napi publish` packaging is implemented.

**Evidence status (2026-10-05, task-11):** GitHub runtime matrix run
[37382716742](https://github.com/Archont561/pathway/actions/runs/37382716742)
on main commit `70bb89d` passed every required job: Node 24 and Node 26 on
Ubuntu, macOS, and Windows; Bun 1.3.11 and Bun 1.4.2 on Ubuntu, macOS, and
Windows; and the three OS-specific packed-install smoke jobs. Each smoke job
builds the addon and TypeScript package, packs `packages/path`, runs
`npm publish --dry-run --provenance`, installs the tarball into a clean
prefix, imports it with Node 24, verifies `engineAvailable()`, and exercises
the built package. The existing `.github/workflows/ci.yml` remains the unified
Ubuntu/Pixi repository gate; it is not this runtime matrix.

### Optional runtimes

Node 22 and Deno 2 are not part of the Phase 1 support claim. Add them only
when a separate compatibility decision and executable smoke tests exist.

### Release workflow direction

A future release workflow may cross-build the Tier 1 NAPI targets and publish
platform packages with provenance. Until that workflow exists, the checked-in
single-package source-build layout and the packed-install smoke test are the
only supported distribution path. Do not copy an illustrative release YAML
into CI as if it were executable.

---

## Bun CI: Why It's Blocking

### The Problem

NAPI-RS's own upstream CI currently treats Bun as **best-effort**. The
NAPI-RS documentation explicitly states that Bun compatibility is not
guaranteed and must be verified by the package author.

### Our Stance

> **Bun is a first-class runtime. The executable workflow is designed to
> prove it — on both 1.3 and 1.4 — rather than infer it from Node-API claims.**
>
> The matrix runs the **same package test suite** on Bun as the Node smoke
> contract across all three operating systems. If a test passes on Node but
> fails on Bun, that is a blocking CI failure. The configuration is committed,
> and main run
> [37382716742](https://github.com/Archont561/pathway/actions/runs/37382716742)
> is the current green evidence for the Phase 1 runtime contract.
> **Bun 1.4 is validated separately from 1.3 because a runtime implementation
> change can alter addon behavior even when the ABI is unchanged.**

### Known Bun Edge Cases to Watch

1. **`tokio::task::spawn_blocking`** — May behave differently under Bun's
   event loop. Test with high-concurrency walks. (Mitigated by the
   `AsyncTask`/libuv-pool design — see napi-boundary.md.)
2. **N-API `ThreadSafeFunction`** — Bun's implementation may have subtle
   differences in callback scheduling. Test with streaming iterators.
3. **`Buffer` handling** — Bun's `Buffer` is not identical to Node's.
   Test binary read/write paths.
4. **`process.exit()` behavior** — Bun may not trigger N-API finalizers
   the same way Node does. Test temp dir cleanup on exit.
5. **Bun 1.4 (Rust runtime):** re-run the entire matrix; the Zig-era
   assumptions in this file's 2025 revision are void.

### Iterator-API Test Checklist (from napi.rs docs, added Sept 2026)

Applies once the `#[napi(async_iterator)]` spike lands:
- `next()` with and without its argument
- Natural completion and calls after completion
- Early break, explicit `return(value)`, and cleanup failure
- Default and recovered `throw(error)` behavior
- Two overlapping async `next()` calls
- Dropping the original async class while retaining only its iterator
- Forced garbage collection and worker-environment shutdown
- Runtimes both with and without the global Iterator helper API

---

## Rejected: `bun:ffi`

### Decision: Rejected

**Rationale:**
1. Bun describes `bun:ffi` as **experimental** in its own documentation.
2. Node-API is the **recommended stable route** for production native code.
3. Using `bun:ffi` would lock us to Bun, defeating the Node + Bun goal.
4. NAPI-RS already provides near-native performance on Bun via Node-API.

### Architecture Remains

```
@archont561/pathway
    ↓
  napi-rs
    ↓
  Node-API (stable ABI)
    ↓
  Target matrix: Node 24/26 + Bun 1.3/1.4
  (configured for Linux/macOS/Windows; verification is a GitHub CI result)
```

---

## Rejected: WASM / WASI

### Decision: Rejected for v0.1–v1.0

**Rationale:**
1. A filesystem package needs **real OS filesystem access**.
2. WASM sandbox is the wrong model for filesystem operations.
3. NAPI-RS docs warn against claiming Bun/Deno WASI support without testing.
4. Known compatibility gaps in WASI filesystem APIs.
5. Target audience runs on real OSes, not browsers.

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| 6 launch targets (+2 Tier 2) | Release target; package publication is not yet implemented |
| Node 24 LTS + 26 Current + Bun 1.3/1.4 | Phase 1 executable matrix; Bun 1.3.11 and 1.4.2 get separate validation; run 37382716742 is green |
| No Deno claim in Phase 1 | No executable Deno smoke job is committed |
| Cross toolchains for musl/arm64 | Required before platform-package publication |
| npm provenance + packed-install smoke test | Supply-chain hygiene; the current single-package path is exercised before release |
| Bun is blocking CI, same suite | NAPI-RS upstream is best-effort; the project must earn the support contract |
| NAPI-RS v3 distribution | The native addon boundary; future platform publication remains release work |
| No `bun:ffi` | Experimental; Node-API is stable and cross-runtime |
| No WASM | Wrong model for filesystem operations |
| musl for Alpine | Release target; no published musl artifact is claimed yet |
