---
type: Architecture Decision
title: "3-Layer Architecture, Path vs FileSystem, Rust Trait"
description: "The 3-layer stack — TypeScript surface, NAPI-RS bridge, Rust engine — the Path vs FileSystem split, the Rust-internal FileSystem trait, and the D7 dual-surface core."
tags: [architecture, layers, path, filesystem, rust, trait, dual-surface]
status: stable
generated:
  by: pathway_kb/1.0
  at: 2026-09-30T00:00:00Z
verified:
  - by: human:archont561
    at: 2025-07-11T00:00:00Z
domain: architecture
decision: decided  # legacy KB status (decided|proposed|deprecated)
created: 2025-07-11
source: conversation
depends_on:
  - CONTEXT
---

# 3-Layer Architecture

## The Stack

```
┌─────────────────────────────────────────┐
│             @archont561/pathway                 │
│                                         │
│  Path • walk • glob • serializers       │
│  ergonomic TypeScript API               │
└────────────────────┬────────────────────┘
                     │
                  napi-rs
                     │
┌────────────────────▼────────────────────┐
│              Rust core                  │
│                                         │
│  PathBuf                                │
│  filesystem traversal                   │
│  glob / regex filtering                 │
│  metadata                               │
│  hashing                                │
│  serialization codecs                   │
└─────────────────────────────────────────┘
```

### Layer 1: TypeScript API (Ergonomic Surface)

The public API should feel **almost boring**. No surprises, no cleverness,
no learning curve for anyone who has used `pathlib` in Python or `Path` in
Java/Kotlin.

```ts
const project = Path.cwd();
const config = project.join("config.toml");
const value = await config.read(toml);

for await (const file of project.walkFiles({
  regex: /\.tsx?$/,
  exclude: ["node_modules", ".git"],
})) {
  console.log(file);
}
```

**Key constraint:** All path string manipulation (`join`, `resolve`, `relative`,
`dirname`, `basename`, `extname`) stays in TypeScript using `pathe`. These are
pure string operations with zero I/O — crossing the N-API boundary for them
would be pure overhead. See [napi-boundary.md](/architecture/napi-boundary.md) for the
full rationale.

### Layer 2: NAPI-RS Bridge (Coarse-Grained FFI)

The bridge crosses the JS↔Rust boundary **only** for operations where native
code provides measurable value: bulk traversal, content hashing, parallel I/O,
native codec parsing. See [napi-boundary.md](/architecture/napi-boundary.md).

### Layer 3: Rust Engine (Performance Core)

Owns all heavy filesystem operations. Uses the Rust `std::fs` ecosystem plus
battle-tested crates (`ignore`, `globset`, `regex`, `blake3`, `serde`).
Internally structured around a `FileSystem` trait, but this trait is an
**internal architecture boundary** — it is never exposed through N-API.

---

## One Core, Two Surfaces (D7, added 2026-09-30)

Layer 3 is packaged as an **rlib crate** (`crates/core`, zero napi
deps), consumed by two independent Layer-1 surfaces:

```
   TypeScript surface (@archont561/pathway)        Rust surface (pathway-fs crate)
        Path • walk • Serializer<T>            Path • walk() • read_toml()
                 │                                       │
         napi-rs (crates/engine,                 plain Rust dependency
          thin cdylib wrapper)                    (crates/path → rlib)
                 │                                       │
                 └──────────────┬────────────────────────┘
                                ▼
                     crates/core (rlib)
          scanner • hashers • atomic fs • serde codecs
```

- The N-API bridge (Layer 2) is a **JS-surface concern only**; Rust
  consumers link core directly, with no FFI in the path.
- The internal `FileSystem` trait stays internal on **both** surfaces —
  `crates/path` exposes a curated pathlib-like API, not the trait.
- Path semantics differ by design: pathe/POSIX on the TS side,
  platform-native `std::path` on the Rust side; core is platform-native
  and glob semantics are defined once in core. Full spec:
  [rust-crate-surface.md](/architecture/rust-crate-surface.md).

---

## Path vs FileSystem

### The Simple API (Default)

For most users, `Path` is the only class they interact with:

```ts
const project = Path.cwd();
const file = project.join("package.json");
```

Internally, `Path` holds a reference to a default filesystem backend:

```
Path
 │
 └── filesystem backend (default: local)
          │
          └── native Rust engine
```

### The Explicit API (Advanced)

For testing, sandboxing, and multi-backend scenarios, a `FileSystem` concept
is introduced as a **second, optional layer**:

```ts
const fs = FileSystem.local();
const project = fs.cwd();
const file = project.join("package.json");
```

This is optional initially. The simple `Path.cwd()` API remains the default
entry point. But the separation gives us a clean extension path:

```ts
FileSystem.local()     // Real OS filesystem (default)
FileSystem.memory()    // In-memory filesystem for tests
FileSystem.sandbox()   // Confined to a root directory
FileSystem.overlay()   // Union mount (e.g., virtual + real)
```

The implemented local seam is `FileSystem.create().sandbox(root)`. It returns
`SandboxPath` values that preserve containment through `resolve`, `join`,
`parent`, and I/O. The TypeScript view performs lexical plus existing-ancestor
`realpath` checks and documents its remaining TOCTOU limitation. The Rust core
also provides a Unix descriptor-anchored `Sandbox::open_read` primitive using
`openat(O_NOFOLLOW)` for operations that require race-resistant containment.

**None of these change the `Path` API.** A `Path` object behaves identically
regardless of which `FileSystem` backend produced it.

### Why Not Make Path Do Everything?

Tempting but wrong. If `Path` owns both identity (the path string) and
behavior (the I/O operations), you cannot:

1. Swap backends for testing without mocking the entire `Path` class.
2. Create sandboxed views that prevent escape via `../`.
3. Overlay virtual files on top of real directories.
4. Run the same path logic against an in-memory tree in CI.

The `Path` + `FileSystem` split follows the same pattern as Rust's own
`Path` (pure data) vs `std::fs` (operations) separation.

---

## The Rust FileSystem Trait (Internal)

Defined inside the Rust engine as an **internal architecture boundary**:

```rust
trait FileSystem {
    fn metadata(&self, path: &Path) -> Result<Metadata>;
    fn read(&self, path: &Path) -> Result<Vec<u8>>;
    fn write(&self, path: &Path, data: &[u8]) -> Result<()>;

    fn mkdir(&self, path: &Path, recursive: bool) -> Result<()>;
    fn remove(&self, path: &Path) -> Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> Result<()>;

    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>>;
}
```

The default implementation uses `std::fs`. Future implementations could use:
- `tokio::fs` for async I/O
- A virtual filesystem for testing
- An overlay filesystem for build caches

**Critical rule:** This trait is never exposed through N-API. The N-API layer
exposes coarse-grained operations (see [napi-boundary.md](/architecture/napi-boundary.md)),
not individual trait methods. The trait exists to keep the Rust codebase
testable and extensible internally.

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| 3 layers, not 2 | Clean separation of concerns; TS ergonomics independent of Rust internals |
| `pathe` for string ops | Zero-cost path manipulation; no FFI overhead for `join()` |
| `Path` as default entry | Boring, familiar API; lowest barrier to adoption |
| `FileSystem` as optional | Enables testing, sandboxing, overlays without breaking simple usage |
| Rust trait is internal | Architecture boundary, not API surface; keeps N-API coarse-grained |
| Core as rlib with two surfaces (D7) | Rust projects consume the same engine via `pathway-fs`; napi stays a JS-boundary detail |
