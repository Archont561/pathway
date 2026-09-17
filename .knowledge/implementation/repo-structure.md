---
type: Implementation Spec
title: "Workspace Layout: Cargo Workspace, TypeScript Package, Directory Tree"
description: "Workspace layout: Cargo workspace, npm/TypeScript package tree, and target directory structure."
tags: [repo, workspace, cargo, npm, directory, structure]
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
  - architecture/core-layers
  - architecture/napi-boundary
---

# Repository Structure

## Monorepo Layout

The project uses a **Cargo workspace** for Rust crates and a **packages/**
directory for the published npm package. This keeps the native engine and
the TypeScript API in a single repository with unified CI.

```
@myorg/path/
│
├── Cargo.toml                    # Cargo workspace root
├── package.json                  # Root package.json (scripts, devDeps)
├── pnpm-workspace.yaml           # pnpm workspace config
├── tsconfig.base.json            # Shared TS config
├── .cargo/
│   └── config.toml               # Rust build config (linker, target dirs)
│
├── crates/
│   └── engine/                   # NAPI-RS Rust core
│       ├── Cargo.toml
│       ├── build.rs              # napi-rs build script
│       └── src/
│           ├── lib.rs            # N-API module registration
│           ├── walk/
│           │   ├── mod.rs        # Walk module exports
│           │   ├── scanner.rs    # NativeScanner (ignore + globset + regex)
│           │   ├── matcher.rs    # Matcher enum (Glob, Regex, Both)
│           │   └── entry.rs      # FusedEntry struct (path, stat, hash)
│           ├── hash/
│           │   ├── mod.rs
│           │   ├── blake3.rs     # BLAKE3 via blake3 crate
│           │   ├── xxhash.rs     # xxHash via xxhash-rust
│           │   └── sha256.rs     # SHA-256 via sha2 crate
│           ├── fs/
│           │   ├── mod.rs
│           │   ├── atomic.rs     # Atomic write (O_EXCL temp + fsync + rename + dir fsync)
│           │   ├── sandbox.rs    # Path containment (canonicalize + openat(O_NOFOLLOW))
│           │   ├── lock.rs       # File locking (flock / LockFileEx, sidecar option)
│           │   └── temp.rs       # Temp dirs (tempfile + O_TMPFILE / DELETE_ON_CLOSE)
│           ├── serializers/
│           │   ├── mod.rs
│           │   ├── toml.rs       # TOML via toml-rs + serde
│           │   └── yaml.rs       # YAML via serde_yaml
│           ├── watch/            # v0.4 — native filesystem watching (notify crate)
│           │   └── mod.rs
│           ├── resolve/          # v1.0 — adapter over the unrs_resolver crate
│           │   └── mod.rs        #   (NOT a from-scratch resolver — Sept 2026)
│           └── error.rs          # Unified error types → N-API errors
│
├── packages/
│   └── path/                     # Published npm package: @myorg/path
│       ├── package.json
│       ├── tsconfig.json
│       ├── src/
│       │   ├── index.ts          # Public exports
│       │   ├── path.ts           # Path class
│       │   ├── fs.ts             # FileSystem abstraction
│       │   ├── walk.ts           # WalkIterator (async generator)
│       │   ├── entry.ts          # PathEntry class
│       │   ├── serializers/
│       │   │   ├── index.ts
│       │   │   ├── types.ts      # Serializer<T> interface
│       │   │   └── json.ts       # Built-in JSON serializer
│       │   ├── types.ts          # Core type definitions
│       │   └── binding.ts        # NAPI-RS binding loader
│       └── test/
│           ├── path.test.ts
│           ├── walk.test.ts
│           ├── serializer.test.ts
│           └── fixtures/         # Test fixtures (small file trees)
│
├── packages/
│   ├── path-toml/                # Extension: @myorg/path-toml
│   │   ├── package.json
│   │   └── src/index.ts
│   └── path-yaml/                # Extension: @myorg/path-yaml
│       ├── package.json
│       └── src/index.ts
│
├── benches/                      # Benchmark suite
│   ├── package.json
│   ├── fdir-vs-native.bench.ts
│   ├── tinyglobby-vs-native.bench.ts
│   ├── bun-glob-vs-native.bench.ts
│   └── fixtures/
│       └── generate.ts           # Generate 100k/500k/1M file trees
│
├── .github/
│   └── workflows/
│       ├── ci.yml                # Test matrix (Node + Bun × OS)
│       ├── release.yml           # NAPI-RS multi-platform build + publish
│       └── bench.yml             # Benchmark regression detection
│
├── .gitignore
├── LICENSE
└── README.md
```

---

## Cargo Workspace (`Cargo.toml`)

```toml
[workspace]
members = ["crates/engine"]
resolver = "2"

[workspace.dependencies]
# Sept 2026: feature set re-verified against current napi-rs docs.
# `async` (+ `tokio_rt`) for async fns / AsyncGenerator; `serde-json` for
# the serde_json::Value → JsUnknown conversion used by the TOML/YAML path.
# `web_stream` is optional (Web Streams consumers) — add if needed.
# The iterator APIs are experimental; the Phase-1 spike pins the exact
# napi-rs version that ships in v0.1.
napi = { version = "3", features = ["async", "tokio_rt", "serde-json"] }
napi-derive = "3"
tokio = { version = "1", features = ["rt-multi-thread"] }
ignore = "0.4"
globset = "0.4"
regex = "1"
blake3 = "1"
xxhash-rust = { version = "0.8", features = ["xxh3"] }
sha2 = "0.10"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
serde_yaml = "0.9"
tempfile = "3"
fs2 = "0.4"
rayon = "1"
thiserror = "2"
```

---

## Engine Crate (`crates/engine/Cargo.toml`)

```toml
[package]
name = "myorg-path-engine"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
napi = { workspace = true }
napi-derive = { workspace = true }
tokio = { workspace = true }
ignore = { workspace = true }
globset = { workspace = true }
regex = { workspace = true }
blake3 = { workspace = true }
xxhash-rust = { workspace = true }
sha2 = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
toml = { workspace = true }
serde_yaml = { workspace = true }
tempfile = { workspace = true }
fs2 = { workspace = true }
rayon = { workspace = true }
thiserror = { workspace = true }

[build-dependencies]
napi-build = "2"

[profile.release]
lto = true
codegen-units = 1
opt-level = 3
strip = "symbols"
```

---

## Package Configuration (`packages/path/package.json`)

```json
{
  "name": "@myorg/path",
  "version": "0.1.0",
  "type": "module",
  "main": "./dist/index.js",
  "types": "./dist/index.d.ts",
  "exports": {
    ".": {
      "import": "./dist/index.js",
      "types": "./dist/index.d.ts"
    }
  },
  "engines": { "node": ">=24" },
  "packageManager": "pnpm@10.0.0",
  "sideEffects": false,
  "files": ["dist", "index.js", "index.d.ts", "myorg-path.*.node"],
  "publishConfig": { "access": "public" },
  "repository": { "type": "git", "url": "TODO: public repo URL (see naming/license decision)" },
  "napi": {
    "name": "myorg-path",
    "triples": {
      "defaults": true,
      "additional": [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "aarch64-pc-windows-msvc"
      ]
    }
  },
  "scripts": {
    "build": "tsc && napi build --platform --release",
    "test": "vitest run",
    "test:bun": "bun test"
  },
  "dependencies": {
    "pathe": "^2.0.3"
  },
  "devDependencies": {
    "@napi-rs/cli": "^3.0.0",
    "typescript": "^5.7.0",
    "vitest": "^3.0.0"
  },
  "optionalDependencies": {
    "@myorg/path-linux-x64-gnu": "0.1.0",
    "@myorg/path-linux-x64-musl": "0.1.0",
    "@myorg/path-linux-arm64-gnu": "0.1.0",
    "@myorg/path-darwin-x64": "0.1.0",
    "@myorg/path-darwin-arm64": "0.1.0",
    "@myorg/path-win32-x64-msvc": "0.1.0",
    "@myorg/path-win32-arm64-msvc": "0.1.0"
  }
}
```

---

## Binding Loader (added Sept 2026)

The JS entry that loads the `.node` binary is **generated by `napi build`**
(platform detection + readable error), per
[code-ts-path.md](/implementation/code-ts-path.md). The 2025 draft had a hand-written
`require("@myorg/path")` (a circular self-require): the binary lives in the
**platform** packages, not the root package. Hand-written platform require
lists are forbidden in this repo.

## v1.0 Resolver Dependency (added Sept 2026)

The `resolve/` module depends on the **`unrs_resolver` Rust crate**
(MIT — enhanced-resolve + tsconfig-paths + tsconfck + PnP, LRU-cached).
It is added to the workspace dependencies when Phase 4 starts; the engine
wraps it behind the TS `Resolver` interface. No from-scratch resolver code
is written.

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Cargo workspace | Single Rust dependency tree; shared crate versions |
| `crates/engine` separate from `packages/path` | Clean native/JS boundary; engine is not published to npm directly |
| `pathe` as only runtime dependency | Minimal footprint; all heavy lifting is native |
| Platform-specific optional deps | NAPI-RS distribution model; only the right binary is installed |
| **Generated binding loader** | Hand-written self-require was a circular-load bug (2026 audit) |
| **`engines`/`packageManager`/`sideEffects`/`files`/`publishConfig`** | Publishing best practices; missing from the 2025 draft (2026 audit) |
| Extension packages (`path-toml`, etc.) | Tree-shakeable; users install only what they need |
| `benches/` as separate package | Isolated benchmark deps; doesn't bloat the main package |
| `pnpm-workspace.yaml` | pnpm is the standard for NAPI-RS monorepos |

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Monorepo (Cargo + pnpm) | Unified CI, single source of truth |
| Engine as cdylib | Required by NAPI-RS for native addon output |
| Release profile with LTO | Maximum performance for the native binary |
| Optional platform deps | Users only download their platform's binary |
| Extension packages separate | Tree-shakeable; keeps core lean |
