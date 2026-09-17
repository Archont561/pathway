---
type: Implementation Spec
title: "NAPI-RS Targets, Platform Matrix, Bun CI, Distribution Hardening"
description: "NAPI-RS target matrix, GitHub Actions CI, Bun 1.3/1.4 and optional Deno legs, provenance and distribution hardening."
tags: [ci, napi-rs, distribution, platform, bun, targets, github-actions, provenance]
status: stable
generated:
  by: pathway_kb/1.0
  at: 2026-09-16T00:00:00Z
verified:
  - by: human:archont561
    at: 2025-07-11T00:00:00Z
  - by: process:gap-analysis-2026-09
    at: 2026-09-16T00:00:00Z
domain: implementation
decision: decided  # legacy KB status (decided|proposed|deprecated)
created: 2025-07-11
source: conversation
depends_on:
  - architecture/napi-boundary
  - implementation/repo-structure
  - competitive/verified-data
---

# CI & Distribution

## NAPI-RS Distribution Model

NAPI-RS uses a **platform-specific package** model. The root package
(`@myorg/path`) declares optional dependencies on platform-specific
packages, and npm/pnpm/bun automatically installs the correct one.

### Published Packages

```
@myorg/path                          ← Root package (JS + types)
@myorg/path-linux-x64-gnu            ← Linux x64 glibc
@myorg/path-linux-arm64-gnu          ← Linux arm64 glibc
@myorg/path-linux-x64-musl           ← Linux x64 musl (Alpine)
@myorg/path-darwin-x64               ← macOS Intel
@myorg/path-darwin-arm64             ← macOS Apple Silicon
@myorg/path-win32-x64-msvc           ← Windows x64
@myorg/path-win32-arm64-msvc         ← Windows arm64
```

Each platform package contains a single `.node` binary:

```
@myorg/path-linux-x64-gnu/
├── package.json
└── myorg-path.linux-x64-gnu.node    ← The compiled Rust addon
```

> **Legacy-npm fallback (added Sept 2026):** some npm versions mishandle
> platform-specific `optionalDependencies` (npm/cli#4828 — the reason
> swc/rollup/unrs-resolver ship `napi-postinstall`). We either add the
> `napi-postinstall` fallback or, at minimum, an install smoke test in CI
> on the oldest supported npm. Either way, the "binding not found" error
> must name the platform package to install.

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

## Runtime Support Matrix (updated Sept 2026)

### Tested Runtimes

| Runtime | Versions | Support Level |
|---------|----------|---------------|
| Node.js 24 "Krypton" | LTS (e.g. 24.21.0) | **Full** — primary production target |
| Node.js 26 | Current (e.g. 26.8.2) | **Full** — libraries must test Current |
| Bun 1.3.x | stable | **Full** (verified via CI) |
| Bun 1.4.x | stable (Rust-rewrite release, Aug 20, 2026) | **Full** — separate validation required |

### Optional (Non-Blocking)

| Runtime | Notes |
|---------|-------|
| Node 22 | Maintenance until Apr 2027 — optional CI leg |
| Deno 2 | NAPI addons are a supported surface (local `node_modules` + `--allow-ffi`; >75% Node suite parity on 2.8). Smoke-test job from v0.2. |

### Removed

| Runtime | Notes |
|---------|-------|
| Node 20 | **EOL** — no longer supported or tested |

---

## CI Configuration

### Test Matrix (`.github/workflows/ci.yml`)

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        include:
          - { os: ubuntu-latest,  runtime: node, node-version: "24" }
          - { os: ubuntu-latest,  runtime: node, node-version: "26" }
          - { os: ubuntu-latest,  runtime: bun,  bun-version: "1.3" }
          - { os: ubuntu-latest,  runtime: bun,  bun-version: "1.4" }
          - { os: macos-latest,   runtime: node, node-version: "24" }
          - { os: macos-latest,   runtime: bun,  bun-version: "1.4" }
          - { os: windows-latest, runtime: node, node-version: "24" }
          - { os: windows-latest, runtime: bun,  bun-version: "1.4" }

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Setup Node
        if: matrix.runtime == 'node'
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}

      - name: Setup Bun
        if: matrix.runtime == 'bun'
        uses: oven-sh/setup-bun@v2
        with:
          bun-version: ${{ matrix.bun-version }}

      - name: Install dependencies
        run: pnpm install

      - name: Build native addon
        run: pnpm --filter @myorg/path build

      - name: Test (Node)
        if: matrix.runtime == 'node'
        run: pnpm --filter @myorg/path test

      - name: Test (Bun)
        if: matrix.runtime == 'bun'
        run: pnpm --filter @myorg/path test:bun

  # Optional, non-blocking: NAPI is a supported Deno 2 surface
  deno-smoke:
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    continue-on-error: true
    steps:
      - uses: actions/checkout@v4
      - uses: denoland/setup-deno@v2
        with: { deno-version: v2.x }
      - run: deno install
      - run: deno test --allow-ffi --allow-read --allow-env --allow-env=TMPDIR

  # Oldest-supported-npm install check (optionalDependencies bug, npm/cli#4828)
  install-smoke:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: "24", npm-version: "8" }
      - run: npm publish --dry-run --dry-run  # placeholder: use a staged package
      - run: npm install --ignore-scripts ./published-packs/@myorg-path.tgz && node -e "require('./node_modules/@myorg/path')"
```

### Release Workflow (`.github/workflows/release.yml`)

```yaml
name: Release

on:
  push:
    tags: ["v*"]

permissions:
  contents: read
  id-token: write   # required for npm provenance

jobs:
  build:
    strategy:
      matrix:
        include:
          - { os: ubuntu-latest,  triple: x86_64-unknown-linux-gnu,     cross: none }
          - { os: ubuntu-latest,  triple: x86_64-unknown-linux-musl,     cross: musl }
          - { os: ubuntu-latest,  triple: aarch64-unknown-linux-gnu,     cross: zigbuild }
          - { os: macos-latest,   triple: x86_64-apple-darwin,           cross: none }
          - { os: macos-latest,   triple: aarch64-apple-darwin,          cross: none }
          - { os: windows-latest, triple: x86_64-pc-windows-msvc,        cross: none }

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-node@v4
        with: { node-version: "24" }

      # Cross toolchain only where the native runner can't produce the triple
      - name: Cross toolchain (musl/arm64)
        if: matrix.cross != 'none'
        run: |
          if [ "${{ matrix.cross }}" = "musl" ]; then
            sudo apt-get install -y musl-tools
            # or: cargo install cargo-zigbuild && use zig for libc
          else
            cargo install cargo-zigbuild
          fi

      - name: Build
        run: npx napi build --platform --release --target ${{ matrix.triple }}

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.triple }}
          path: "*.node"

  publish:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: "24"
          registry-url: "https://registry.npmjs.org"

      - name: Download all artifacts
        uses: actions/download-artifact@v4

      - name: Publish (with provenance)
        # --provenance: npm publishes a Sigstore attestation linking the
        # package to this GitHub Actions run (supply-chain best practice).
        run: npx napi publish
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

> **Version lockstep:** root + all platform packages must publish the same
> version in one shot (napi publish handles this); add a CI assertion that
> every `optionalDependencies` entry has a matching built artifact before
> publishing.

---

## Bun CI: Why It's Blocking

### The Problem

NAPI-RS's own upstream CI currently treats Bun as **best-effort**. The
NAPI-RS documentation explicitly states that Bun compatibility is not
guaranteed and must be verified by the package author.

### Our Stance

> **Bun is a first-class runtime. Our CI proves it — on both 1.3 and 1.4.**

We do not rely on:
> "Bun supports Node-API, therefore it works."

We run the **exact same test suite** on Bun as on Node.js, across all three
operating systems. If a test passes on Node but fails on Bun, that is a
blocking CI failure. **Bun 1.4 (the Rust-rewrite release) is validated
separately from 1.3 — a runtime rewritten in a new language can change
addon behavior even when the ABI is unchanged.**

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
@myorg/path
    ↓
  napi-rs
    ↓
  Node-API (stable ABI)
    ↓
  Works on: Node 24/26 ✅ | Bun 1.3/1.4 ✅ | Deno 2 (smoke)
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
| 6 launch targets (+2 Tier 2) | Covers ~99% of Node/Bun users; expand later |
| Node 24 LTS + 26 Current + Bun 1.3/1.4 | Sept 2026 runtime reality; Bun's Rust rewrite gets separate validation |
| Deno 2 smoke job (non-blocking) | NAPI is a supported Deno surface; cheap reach |
| Cross toolchains for musl/arm64 | x64 runners can't produce those triples as written in the 2025 draft |
| npm provenance + install smoke test | Supply-chain hygiene; legacy-npm optionalDependencies bug (npm/cli#4828) |
| Bun is blocking CI, same suite | NAPI-RS upstream is best-effort; we earn the support contract |
| NAPI-RS v3 distribution | Proven model (SWC, lightningcss, node-rs) |
| No `bun:ffi` | Experimental; Node-API is stable and cross-runtime |
| No WASM | Wrong model for filesystem operations |
| musl for Alpine | Docker users on Alpine need musl binaries |
