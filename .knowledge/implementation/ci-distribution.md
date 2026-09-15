---
title: "NAPI-RS Targets, Platform Matrix, Bun CI, bun:ffi Rejection"
domain: implementation
status: decided
created: 2025-07-11
updated: 2025-07-11
source: conversation
depends_on:
  - architecture/napi-boundary
  - implementation/repo-structure
  - competitive/verified-data
tags: [ci, napi-rs, distribution, platform, bun, targets, github-actions]
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

### Explicitly NOT Supported

| Target | Reason |
|--------|--------|
| WASM/WASI | Filesystem needs real OS access; see [napi-boundary.md](../architecture/napi-boundary.md) |
| FreeBSD | Low demand; can be added if community requests |
| 32-bit (x86, armv7) | Node.js dropped 32-bit support |

---

## Runtime Support Matrix

### Tested Runtimes

| Runtime | Versions | Support Level |
|---------|----------|---------------|
| Node.js | 22 LTS, 24 Current | **Full** |
| Bun | Latest stable | **Full** (verified via CI) |

### Best-Effort (Not Blocking)

| Runtime | Notes |
|---------|-------|
| Deno | NAPI-RS supports Deno, but we don't CI-test it initially |
| Node 20 | Should work (Node-API ABI stable) but not CI-tested |

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
        runtime: [node, bun]
        include:
          - os: ubuntu-latest
            runtime: node
            node-version: "22"
          - os: ubuntu-latest
            runtime: node
            node-version: "24"
          - os: ubuntu-latest
            runtime: bun
            bun-version: "latest"
          - os: macos-latest
            runtime: node
            node-version: "22"
          - os: macos-latest
            runtime: bun
            bun-version: "latest"
          - os: windows-latest
            runtime: node
            node-version: "22"
          - os: windows-latest
            runtime: bun
            bun-version: "latest"

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
```

### Release Workflow (`.github/workflows/release.yml`)

```yaml
name: Release

on:
  push:
    tags: ["v*"]

jobs:
  build:
    strategy:
      matrix:
        target:
          - os: ubuntu-latest
            triple: x86_64-unknown-linux-gnu
          - os: ubuntu-latest
            triple: x86_64-unknown-linux-musl
          - os: ubuntu-latest
            triple: aarch64-unknown-linux-gnu
          - os: macos-latest
            triple: x86_64-apple-darwin
          - os: macos-latest
            triple: aarch64-apple-darwin
          - os: windows-latest
            triple: x86_64-pc-windows-msvc

    runs-on: ${{ matrix.target.os }}

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-node@v4
        with:
          node-version: "22"

      - name: Build
        run: npx napi build --platform --release --target ${{ matrix.target.triple }}

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.target.triple }}
          path: "*.node"

  publish:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: "22"
          registry-url: "https://registry.npmjs.org"

      - name: Download all artifacts
        uses: actions/download-artifact@v4

      - name: Publish
        run: npx napi publish
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

---

## Bun CI: Why It's Blocking

### The Problem

NAPI-RS's own upstream CI currently treats Bun as **best-effort**. The
NAPI-RS documentation explicitly states that Bun compatibility is not
guaranteed and must be verified by the package author.

### Our Stance

> **Bun is a first-class runtime. Our CI proves it.**

We do not rely on:
> "Bun supports Node-API, therefore it works."

We run the **exact same test suite** on Bun as on Node.js, across all
three operating systems. If a test passes on Node but fails on Bun, that
is a blocking CI failure.

### Known Bun Edge Cases to Watch

1. **`tokio::task::spawn_blocking`** — May behave differently under Bun's
   event loop. Test with high-concurrency walks.
2. **N-API `ThreadSafeFunction`** — Bun's implementation may have subtle
   differences in callback scheduling. Test with streaming iterators.
3. **`Buffer` handling** — Bun's `Buffer` is not identical to Node's.
   Test binary read/write paths.
4. **`process.exit()` behavior** — Bun may not trigger N-API finalizers
   the same way Node does. Test temp dir cleanup on exit.

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
  Works on: Node.js ✅ | Bun ✅ | Deno (best-effort)
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
| 6 launch targets | Covers 99% of Node/Bun users; expand later |
| Node 22 + 24 + Bun latest | Current LTS + current + emerging runtime |
| Bun is blocking CI | NAPI-RS upstream is best-effort; we earn the support contract |
| Same test suite on all runtimes | No "works on my machine" gaps |
| NAPI-RS v3 distribution | Proven model (SWC, lightningcss, node-rs) |
| No `bun:ffi` | Experimental; Node-API is stable and cross-runtime |
| No WASM | Wrong model for filesystem operations |
| musl for Alpine | Docker users on Alpine need musl binaries |
