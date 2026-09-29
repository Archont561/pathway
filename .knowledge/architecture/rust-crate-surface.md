---
type: Architecture Decision
title: "Rust Crate Surface: Publishable Core + Ergonomic pathlib API for Rust"
description: "D7 — split the engine into an rlib core and publish `myorg-path` to crates.io as a pathlib-like Rust API over the same core the N-API engine uses."
tags: [architecture, rust, crate, crates-io, core, pathlib, dual-surface]
status: stable
generated:
  by: pathway_kb/1.0
  at: 2026-09-30T00:00:00Z
verified:
  - by: human:archont561
    at: 2026-09-30T00:00:00Z
domain: architecture
decision: decided  # legacy KB status (decided|proposed|deprecated)
created: 2026-09-30
source: conversation
depends_on:
  - CONTEXT
  - architecture/core-layers
  - architecture/napi-boundary
  - implementation/repo-structure
---

# Rust Crate Surface (D7)

## Decision

The Rust engine is **not** a JS-only implementation detail. It is split so
that the same core ships as a **published Rust crate** giving Rust projects
the same pathlib-like ergonomics the TypeScript surface gives JS:

```
crates/core     myorg-path-core     rlib    crates.io   low-level ops (walk, hash, fs, codecs)
crates/path     myorg-path          rlib    crates.io   ergonomic pathlib-like Rust API
crates/engine   myorg-path-engine   cdylib  npm only    thin NAPI-RS wrapper over core
```

- `crates/core` contains **all** engine logic (scanner, hashers, atomic
  write, sandbox, lock, temp, serde codecs). Zero `napi` dependencies.
  `crate-type = ["rlib"]`. Fully testable with plain `cargo test` — no
  Node involved.
- `crates/engine` shrinks to N-API glue: `#[napi]` bindings, JS type
  conversion, async-iterator/AsyncTask plumbing. It re-exports nothing.
- `crates/path` is the **Rust Layer 1**: a curated, pathlib-flavoured
  API over core, published to crates.io and documented on docs.rs.

This makes the architecture **one core, two ergonomic surfaces**
(TypeScript `Path` and Rust `Path`), with the N-API bridge relevant only
to the JS surface.

---

## Rust API Sketch

The Rust surface mirrors the TS surface where it makes sense, and defers
to `std`/idiomatic Rust where it doesn't:

```rust
use myorg_path::{Path, Hash};

let project = Path::cwd()?;

// pathlib-style navigation (thin sugar over std::path)
let config = project.join("config.toml");

// typed reads via serde — the Serializer<T> analogue
let cfg: Config = config.read_toml()?;
config.with_extension("json").write_json_atomic(&cfg)?;

// the fused walk — same killer feature, same core code path
for entry in project
    .walk()
    .glob("**/*.rs")
    .exclude(["target", ".git"])
    .hash(Hash::Blake3)
{
    let entry = entry?;
    println!("{}  {}", entry.hash().unwrap(), entry.path().display());
}

// temp dirs, locking, sandbox — same primitives as the TS surface
project.join("build.lock").with_lock(|| { /* ... */ })?;
```

**Honest positioning:** `std::path` + `std::fs` already covers most of
pathlib. The crate's value is what Rust has no single coherent crate for:
the fused walk builder, glob-on-a-path, integrated hashing, typed serde
read/write with atomic semantics, and temp/lock/sandbox sugar — i.e. the
[killer features](/features/killer-features.md), not `join()`.
Pitch: **"pathlib's convenience + ripgrep's walker, one crate."**

---

## Path Semantics Rule

The two surfaces intentionally differ in path-string semantics, and this
is documented rather than papered over:

| Surface | Path type | Semantics |
|---------|-----------|-----------|
| TypeScript | `string` via `pathe` | POSIX-normalized (`/` everywhere) |
| Rust | `std::path::PathBuf` | Platform-native (Windows `\`, UNC, non-UTF-8) |
| `crates/core` | `std::path::Path` | Platform-native — core never assumes pathe normalization |

- Core accepts platform-native paths; the **engine** (N-API layer) owns
  any pathe↔native conversion needed at the JS boundary.
- The Rust surface may add an optional `camino` feature for UTF-8 paths;
  it never forces it.
- Glob matching semantics (root-relative, `**` behavior, separators) are
  defined **once in core** and shared by both surfaces — the
  glob-semantics test matrix from
  [code-rust-walker.md](/implementation/code-rust-walker.md) is a core test suite.

---

## Versioning, MSRV, Scope Discipline

- `myorg-path` / `myorg-path-core` follow **independent semver** from the
  npm package. Feature parity is a goal, lockstep version numbers are not.
- MSRV policy: latest stable minus 2, declared via `rust-version` in
  `crates/core` and CI-enforced.
- `crates/engine` is **never published to crates.io** (it is a cdylib
  napi artifact, useless as a dependency).
- The Rust surface is **deliberately thinner** than the TS surface:
  anything `std` already does well (`parent()`, `extension()`,
  `exists()`) is inherited or thinly delegated, not re-invented.
- API parity pressure flows one way: TS Layer 1 remains the primary
  product; a feature may ship TS-first and reach the Rust surface later.
  A Rust-surface gap never blocks an npm release.

---

## Phasing

| Phase | Rust-surface deliverable |
|-------|--------------------------|
| **v0.1 (Phase 1)** | `crates/core` exists as rlib from day one; `crates/path` is a stub. Core is `cargo test`-covered. Names reserved on crates.io (Step 0). |
| **v0.3 (Phase 3)** | `myorg-path` **preview** published to crates.io (walk + hash + serde read/write + temp/lock). docs.rs docs. |
| **v1.0 (Phase 4)** | Rust surface stabilized (1.0 on crates.io), full killer-feature coverage where applicable. |

Deferring the ergonomic surface to v0.3 keeps Phase 1 focused on the
benchmark gate, while the core/engine split (which must happen in Phase 1
scaffolding) makes the later crate release low-risk.

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Core as `rlib`, engine as thin `cdylib` wrapper | Enables Rust consumers; also gives Node-free `cargo test` for all engine logic |
| Separate `crates/path` for the ergonomic Rust API | Keeps core low-level and stable; sugar can iterate without breaking core semver |
| Engine never on crates.io | cdylib napi glue is not a usable Rust dependency |
| Platform-native path semantics in core/Rust surface | Matches Rust ecosystem expectations; pathe normalization is a JS-boundary concern |
| Independent semver + MSRV policy | Two ecosystems, two release cadences; no lockstep coupling |
| Rust surface deferred to v0.3 preview | Phase 1 stays focused on the ≥5x benchmark proof; split is cheap now, crate is cheap later |
