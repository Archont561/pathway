---
title: "3-Layer Architecture, Path vs FileSystem, Rust Trait"
domain: architecture
status: decided
created: 2025-07-11
updated: 2025-07-11
source: conversation
depends_on:
  - meta/CONTEXT
tags: [architecture, layers, path, filesystem, rust, trait]
---

# 3-Layer Architecture

## The Stack

```
┌─────────────────────────────────────────┐
│             @myorg/path                 │
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
would be pure overhead. See [napi-boundary.md](./napi-boundary.md) for the
full rationale.

### Layer 2: NAPI-RS Bridge (Coarse-Grained FFI)

The bridge crosses the JS↔Rust boundary **only** for operations where native
code provides measurable value: bulk traversal, content hashing, parallel I/O,
native codec parsing. See [napi-boundary.md](./napi-boundary.md).

### Layer 3: Rust Engine (Performance Core)

Owns all heavy filesystem operations. Uses the Rust `std::fs` ecosystem plus
battle-tested crates (`ignore`, `globset`, `regex`, `blake3`, `serde`).
Internally structured around a `FileSystem` trait, but this trait is an
**internal architecture boundary** — it is never exposed through N-API.

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
exposes coarse-grained operations (see [napi-boundary.md](./napi-boundary.md)),
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
