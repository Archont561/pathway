---
type: Implementation Spec
title: "Workspace Layout: Cargo Workspace, TypeScript Package, Directory Tree"
description: "Workspace layout: three-crate Cargo workspace (core rlib, path crate, napi engine), npm/TypeScript package tree, and target directory structure."
tags: [repo, workspace, cargo, npm, directory, structure, crates-io]
status: stable
generated:
  by: pathway_kb/1.0
  at: 2026-09-30T00:00:00Z
verified:
  - by: human:archont561
    at: 2025-07-11T00:00:00Z
  - by: process:gap-analysis-2026-09
    at: 2026-09-16T00:00:00Z
  - by: process:orchestration-refactor-2026-09-30
    at: 2026-09-30T00:00:00Z
domain: implementation
decision: decided  # legacy KB status (decided|proposed|deprecated)
created: 2025-07-11
source: conversation
depends_on:
  - architecture/core-layers
  - architecture/napi-boundary
  - architecture/rust-crate-surface
---

# Repository Structure

## Monorepo Layout

The project uses a **Cargo workspace** for Rust crates and a **packages/**
directory for the published npm package. This keeps the native engine and
the TypeScript API in a single repository with unified CI.

Per **D7** ([rust-crate-surface.md](/architecture/rust-crate-surface.md)),
the workspace has **three crates** (added 2026-09-30): all engine logic
lives in `crates/core` (an `rlib` with zero napi deps, published to
crates.io as `pathway-fs-core`); `crates/engine` is a thin `cdylib`
NAPI-RS wrapper for npm distribution; and `crates/path` is the ergonomic
pathlib-like Rust API published to crates.io as `pathway-fs`.

```
@archont561/pathway/
│
├── Cargo.toml                    # Cargo workspace root
├── package.json                  # Bun workspace root: workspaces, scripts, toolchain devDeps
├── bun.lock                      # The single JS dependency resolution record
├── pixi.toml / pixi.lock         # pixi environments + tasks; the lockfile is what CI verifies
├── biome.json / turbo.json       # JS lint config; turbo task graph + cache inputs
├── lefthook.yml / .versionrc     # Hooks delegate to pixi tasks; convco changelog config
├── .pixi-sandbox.toml            # The reviewed sandbox publish plan
├── .cargo/
│   └── config.toml               # gitignored — written by `pixi-sandbox restore` (crates.io → vendored tree)
│
├── crates/
│   ├── core/                     # pathway-fs-core — rlib, ALL engine logic, zero napi deps (D7)
│   │   ├── Cargo.toml            # crates.io-publishable; `cargo test` needs no Node
│   │   └── src/
│   │       ├── lib.rs            # Public Rust API of the core (curated, low-level)
│   │       ├── walk/
│   │       │   ├── mod.rs        # Walk module exports
│   │       │   ├── scanner.rs    # NativeScanner (ignore + globset + regex)
│   │       │   ├── matcher.rs    # Matcher enum (Glob, Regex, Both)
│   │       │   └── entry.rs      # FusedEntry struct (path, stat, hash)
│   │       ├── hash/
│   │       │   ├── mod.rs
│   │       │   ├── blake3.rs     # BLAKE3 via blake3 crate
│   │       │   ├── xxhash.rs     # xxHash via xxhash-rust
│   │       │   └── sha256.rs     # SHA-256 via sha2 crate
│   │       ├── fs/
│   │       │   ├── mod.rs
│   │       │   ├── atomic.rs     # Atomic write (O_EXCL temp + fsync + rename + dir fsync)
│   │       │   ├── sandbox.rs    # Path containment (canonicalize + openat(O_NOFOLLOW))
│   │       │   ├── lock.rs       # File locking (flock / LockFileEx, sidecar option)
│   │       │   └── temp.rs       # Temp dirs (tempfile + O_TMPFILE / DELETE_ON_CLOSE)
│   │       ├── serializers/
│   │       │   ├── mod.rs
│   │       │   ├── toml.rs       # TOML via toml-rs + serde
│   │       │   └── yaml.rs       # YAML via serde_yaml
│   │       ├── watch/            # v0.4 — native filesystem watching (notify crate)
│   │       │   └── mod.rs
│   │       ├── resolve/          # v1.0 — adapter over the unrs_resolver crate
│   │       │   └── mod.rs        #   (NOT a from-scratch resolver — Sept 2026)
│   │       └── error.rs          # Unified error types (thiserror)
│   │
│   ├── path/                     # pathway-fs — ergonomic pathlib-like Rust API (D7, v0.3 preview)
│   │   ├── Cargo.toml            # rlib over core; published to crates.io
│   │   └── src/
│   │       ├── lib.rs            # Public exports (Path, WalkBuilder, Hash, ...)
│   │       ├── path.rs           # Path: join/cwd sugar + read_/write_ helpers over core
│   │       ├── walk.rs           # Fluent walk builder → core scanner iterator
│   │       └── serde_ext.rs      # read_json/read_toml/write_*_atomic via serde
│   │
│   └── engine/                   # pathway-fs-engine — thin NAPI-RS wrapper, npm only
│       ├── Cargo.toml            # cdylib; never published to crates.io
│       ├── build.rs              # napi-rs build script
│       └── src/
│           ├── lib.rs            # N-API module registration
│           ├── walk.rs           # #[napi] bindings → core walk (batches, AbortSignal)
│           ├── fs.rs             # #[napi] bindings → core fs ops
│           ├── serializers.rs    # serde_json::Value → JsUnknown conversion
│           └── error.rs          # core errors → N-API errors
│
├── packages/
│   ├── typescript-config/        # @repo/typescript-config — internal package: shared tsconfig
│   │   ├── package.json          #   bases (base.json language+resolution, library.json
│   │   ├── base.json             #   + declaration emit). Internal (@repo/*), never published;
│   │   └── library.json          #   consumers extend it via a real dependency edge.
│   └── path/                     # Published npm package: @archont561/pathway
│       ├── package.json
│       ├── tsconfig.json         # noEmit; includes src + test
│       ├── tsconfig.build.json   # declaration emit into dist/
│       ├── src/
│       │   ├── index.ts          # Public exports
│       │   ├── path.ts           # Path class
│       │   ├── walk.ts           # WalkIterator (async generator; stub)
│       │   ├── serializers/
│       │   │   ├── index.ts
│       │   │   └── json.ts       # Built-in JSON serializer
│       │   ├── types.ts          # Core type definitions
│       │   └── binding.ts        # NAPI-RS binding loader (diagnostic front end)
│       └── test/
│           └── path.test.ts      # includes the npm/Cargo version-parity test
│
├── apps/
│   └── docs/                     # Astro + Starlight documentation site
│       ├── package.json          # dev / build / preview / typecheck (astro check)
│       ├── turbo.json            # Package Configuration: declares the root
│       │                         #   Cargo.toml and scripts/version.ts as build
│       │                         #   inputs, so a version bump invalidates the
│       │                         #   cached site instead of serving a stale one
│       ├── astro.config.mjs      # aliases @workspace/version → scripts/version.ts
│       └── src/
│           ├── version.ts        # reads [workspace.package] version; throws if absent
│           ├── content.config.ts
│           └── content/docs/     # .mdx pages
│
├── scripts/
│   ├── version.ts                # the one version reader (importable + CLI)
│   ├── restore.sh                # offline sandbox reconstruction
│   └── restore.ps1
│
├── .github/
│   └── workflows/
│       ├── ci.yml                # one job; every step is `pixi run <task>`
│       ├── docs.yml              # docs build + astro check; Pages deploy from main
│       └── publish-sandbox.yml   # publishes the offline sandbox transport
│
├── .gitignore
├── LICENSE
└── README.md
```

### Planned, not yet in the tree

These are designed but unbuilt. They are listed separately on purpose: a
directory in the tree above is one that exists.

| Planned | Gate |
| --- | --- |
| `benches/*` — the benchmark harness (mitata) | backlog task-4. The `benches/*` workspace glob, the turbo `bench` task and the pixi `bench` task were **removed** until it lands: a task no package implements exits 0 having run nothing, and a benchmark that silently passes is the worst possible failure mode for the claim it exists to prove. Re-add all four together. |
| `packages/path-toml`, `packages/path-yaml` — tree-shakeable codec extensions | v0.2+ |
| A multi-platform npm release workflow (`napi publish`) | see [ci-distribution.md](/implementation/ci-distribution.md) |

---

## Cargo Workspace (`Cargo.toml`)

```toml
[workspace]
members = ["crates/core", "crates/path", "crates/engine"]
resolver = "2"

# Shared metadata, inherited by every member with `<key>.workspace = true`, so
# the version, the MSRV and the licence are each one edit.
[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.98"
license = "MIT"

# Profiles must live at the workspace root — Cargo ignores [profile.*]
# in member manifests (moved here from crates/engine, 2026-09-30).
[profile.release]
lto = true
codegen-units = 1
opt-level = 3
strip = "symbols"

[workspace.dependencies]
pathway-fs-core = { path = "crates/core", version = "0.1.0" }
# Sept 2026: feature set re-verified against current napi-rs docs.
# `async` for async fns / AsyncGenerator (it pulls in `tokio_rt`, so naming
# that separately is redundant); `serde-json` for the serde_json::Value →
# JsUnknown conversion used by the TOML/YAML path. `web_stream` is optional
# (Web Streams consumers) — add if needed. The iterator APIs are experimental;
# the Phase-1 spike pins the exact napi-rs version that ships in v0.1.
napi = { version = "3", features = ["async", "serde-json"] }
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

## Core Crate (`crates/core/Cargo.toml`) — added 2026-09-30 (D7)

All filesystem/traversal/hash/codec logic lives here. **No napi
dependencies** — this crate compiles and tests without Node, and is
published to crates.io.

```toml
[package]
name = "pathway-fs-core"
description = "Native filesystem core: fused walk, hashing, atomic ops, serde codecs"
publish = false                # until the crates.io names are reserved
version.workspace = true       # 0.1.0, inherited — never restated per crate
edition.workspace = true       # 2021
rust-version.workspace = true  # 1.98 — conda-forge pins rust 1.98.x; napi 3.13
                               #   declares MSRV 1.88, so the channel is binding
# license is inherited too: MIT, settled at Step-0 sign-off (2026-09-30).
# One LICENSE at the root covers every crate and the npm package.

[lib]
crate-type = ["rlib"]

[dependencies]
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
```

---

## Rust API Crate (`crates/path/Cargo.toml`) — added 2026-09-30 (D7)

The ergonomic pathlib-like surface for Rust consumers
([rust-crate-surface.md](/architecture/rust-crate-surface.md)). Stub in
Phase 1; published as a preview at v0.3.

```toml
[package]
name = "pathway-fs"
description = "pathlib's convenience + ripgrep's walker: ergonomic paths, fused walk, hashing, typed serde I/O"
publish = false                # until the crates.io names are reserved
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[lib]
crate-type = ["rlib"]

[dependencies]
# Through the workspace, so there is one place to bump the path/version pair.
pathway-fs-core = { workspace = true }
serde = { workspace = true }
thiserror = { workspace = true }
```

An optional `camino` feature (UTF-8 path type) is sketched in the D7 notes but
is **not** in the manifest yet; add it with the v0.3 preview, not before.

---

## Engine Crate (`crates/engine/Cargo.toml`)

Thin NAPI-RS wrapper over `crates/core` — bindings, JS type conversion,
async plumbing only. **Never published to crates.io.**

```toml
[package]
name = "pathway-fs-engine"
publish = false                # npm-only artifact (D7)
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
pathway-fs-core = { workspace = true }
napi = { workspace = true }
napi-derive = { workspace = true }
tokio = { workspace = true }
serde_json = { workspace = true }   # Value → JsUnknown conversion

[build-dependencies]
napi-build = { workspace = true }
```

(Release profile lives in the workspace root `Cargo.toml` — see above.)

---

## Package Configuration (`packages/path/package.json`)

The actual manifest (verified 2026-09-30 — the 2025 draft sketched pnpm +
vitest; the repo standardized on bun + `bun test` when it scaffolded):

```json
{
  "name": "@archont561/pathway",
  "version": "0.1.0",
  "license": "MIT",
  "type": "module",
  "main": "./dist/index.js",
  "types": "./dist/index.d.ts",
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "import": "./dist/index.js",
      "default": "./dist/index.js"
    }
  },
  "engines": { "node": ">=24" },
  "sideEffects": false,
  "files": ["dist", "pathway.*.node"],
  "publishConfig": { "access": "public" },
  "napi": { "binaryName": "pathway" },
  "scripts": {
    "build": "tsc -p tsconfig.build.json",
    "build:native": "napi build --platform --no-js --dts native-engine.d.ts --manifest-path ../../crates/engine/Cargo.toml --output-dir .",
    "build:native:release": "napi build --platform --release --no-js --dts native-engine.d.ts --manifest-path ../../crates/engine/Cargo.toml --output-dir .",
    "typecheck": "tsc -p tsconfig.json --noEmit",
    "test": "bun test"
  },
  "dependencies": {
    "pathe": "^2.0.3"
  },
  "devDependencies": {
    "@napi-rs/cli": "^3.0.0",
    "@repo/typescript-config": "workspace:*"
  }
}
```

Notes:

- `napi.binaryName` names the addon `pathway.<platform>.node`. `--no-js` keeps
  the hand-written loader front-end (`src/binding.ts`) in charge until the
  generated loader replaces it with the task-3 engine bridge.
- `build:native` is the debug build (the dev loop, ~40s cold); the release
  variant (LTO) is for benchmarks and the publish pipeline. turbo runs these
  same scripts as its `build:native` / `build:native:release` tasks with
  `$TURBO_ROOT$/crates/**` inputs, so a tsc-only change never pays for a cargo
  rebuild. `pixi run build-native` reaches them **through** turbo
  (`--filter=@archont561/pathway`) rather than with `cwd`, so the pixi task and
  the turbo task share one cache entry.
- `packageManager` is deliberately **absent**: it is a corepack field that
  says which package manager runs *the repository*, so it belongs to the root
  `package.json` only. Restating it here was a second bun version to bump and
  is meaningless in a published tarball.
- The version is asserted against `[workspace.package] version` in the root
  `Cargo.toml` by a test in `packages/path/test` — never bumped here alone.
- The platform-specific `optionalDependencies` matrix
  (`@archont561/pathway-linux-x64-gnu` and friends) is **added by the release
  pipeline** (`napi publish`, per
  [ci-distribution.md](/implementation/ci-distribution.md)), never
  hand-maintained in this manifest.

---

## Binding Loader (added Sept 2026)

The JS entry that loads the `.node` binary is **generated by `napi build`**
(platform detection + readable error), per
[code-ts-path.md](/implementation/code-ts-path.md). The 2025 draft had a hand-written
`require("@archont561/pathway")` (a circular self-require): the binary lives in the
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
| **Three-crate split: `core` (rlib) / `path` (Rust API) / `engine` (cdylib)** | D7: Rust consumers get a published crate; core is `cargo test`-able without Node; napi glue stays thin |
| `crates/engine` separate from `packages/path` | Clean native/JS boundary; engine is not published to npm directly |
| `pathe` as only runtime dependency | Minimal footprint; all heavy lifting is native |
| Platform-specific optional deps | NAPI-RS distribution model; only the right binary is installed |
| **Generated binding loader** | Hand-written self-require was a circular-load bug (2026 audit) |
| **`engines`/`sideEffects`/`files`/`publishConfig`** | Publishing best practices; missing from the 2025 draft (2026 audit). `packageManager` is root-only — it describes the repo, not the tarball |
| Extension packages (`path-toml`, etc.) — *planned* | Tree-shakeable; users install only what they need |
| Bun workspaces in the root `package.json` (no workspace yaml) | bun is the repo's only JS runtime — no Node anywhere; one root `bun.lock` resolves the whole JS toolchain |
| **Workspace globs name only directories that exist** | A glob matching nothing makes `turbo run <task>` exit 0 having run nothing. `benches/*` was such a glob and was removed until task-4 |
| **No pixi task uses `cwd = "<package>"`** | Package-scoped work goes through `turbo run <task> --filter=<package>`. A task that shells into a package bypasses the cache and diverges from the turbo task for the same verb — which is how the docs site came to be built twice in `pixi run ci` |
| **`test` depends on `build:native`** | The addon must exist before the Bun suite runs. Encoding it in `turbo.json` rather than as `napi build && bun test` keeps the edge visible to the task graph and to the cache |
| **`apps/docs/turbo.json` names `Cargo.toml` a build input** | The site quotes `[workspace.package] version`; without that input a version bump would serve a cached site with the old number |

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Monorepo (Cargo + bun) | Unified CI, single source of truth |
| Engine as cdylib | Required by NAPI-RS for native addon output |
| Core as rlib on crates.io (`pathway-fs-core`) | D7: reusable from Rust; Node-free unit tests for all engine logic |
| `pathway-fs` crate as the Rust Layer 1 | D7: pathlib-like ergonomics for Rust projects, same core as the TS surface |
| `publish = false` on engine | cdylib napi glue is not a usable Rust dependency |
| Release profile with LTO (workspace root) | Maximum performance for the native binary |
| Optional platform deps | Users only download their platform's binary |
| Extension packages separate | Tree-shakeable; keeps core lean |
