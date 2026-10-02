---
okf_version: "0.2"
---

# @archont561/pathway — Knowledge Base

Native, pathlib-inspired filesystem API for TypeScript, Bun, and Node,
built on a 3-layer stack: ergonomic TS surface → NAPI-RS bridge → Rust
engine. One file = one concept (OKF v0.2 bundle).

Read [Project Context](/CONTEXT.md) first for current state and the
foundational decisions. Change history: [update log](/log.md).

# Context

* [Project Context & Decision Log](/CONTEXT.md) - Identity, current state, decisions D1–D7, market snapshot, roadmap, bundle conventions.

# Architecture

* [3-Layer Architecture](/architecture/core-layers.md) - TS surface, NAPI-RS bridge, Rust engine; Path vs FileSystem; the Rust-internal trait; D7 dual-surface core.
* [Coarse-Grained FFI Boundary](/architecture/napi-boundary.md) - What stays in TypeScript, what crosses to Rust; per-call boundary hops banned.
* [Fused Walk](/architecture/fused-walk.md) - Single-pass stat+hash+filter in Rust — the defensible performance moat.
* [Rust Crate Surface](/architecture/rust-crate-surface.md) - D7: rlib core + `pathway-fs` crate on crates.io — pathlib-like ergonomics for Rust projects.

# Features

* [Walk & Traversal](/features/walk-traversal.md) - Glob/regex matching, excludes, pruning, predicates, async iteration, batching.
* [Serializers](/features/serializers.md) - Serializer&lt;T&gt;, Serde codecs, per-FileSystem registry, typed read/write.
* [Killer Features](/features/killer-features.md) - Temp dirs, snapshots/diff, sandbox, transactions, locking, parallel ops.
* [Pluggable Patterns](/features/pluggable-patterns.md) - Transformers, hashers, detectors, validators, resolvers, CAS.

# Competitive Intelligence

* [Landscape](/competitive/landscape.md) - Tiers 1–5 library map and the gap matrix @archont561/pathway targets.
* [Verified Market Data](/competitive/verified-data.md) - Node 24/26, stable `node:fs.glob`, Bun 1.3/1.4, Deno 2, NAPI-RS state; Round-1 corrections.
* [Positioning](/competitive/positioning.md) - Target audience, one-line pitch, Bun-as-ally posture, messaging guardrails.

# Implementation

* [Repo Structure](/implementation/repo-structure.md) - Cargo workspace, npm/TypeScript package tree, directory layout.
* [CI & Distribution](/implementation/ci-distribution.md) - NAPI-RS targets, platform matrix, Bun CI, provenance hardening.
* [Rust Walker (Reference)](/implementation/code-rust-walker.md) - NativeScanner over the `ignore` crate; chunked fused batches.
* [TypeScript Path (Reference)](/implementation/code-ts-path.md) - `Path` class, `Serializer<T>`, `WalkIterator`, `FileSystem`.

# Delivery Tracking

Not part of this bundle. Plans, checkboxes and release gates live in `backlog/`,
because they are tracking rather than design truth and they change on a
different clock to the concepts above.

* [Phase Plan](../backlog/docs/phase-plan.md) - Phase 1–4 steps and benchmark gates (was `/implementation/phase-plan.md`).
* [Release Roadmap](../backlog/docs/release-roadmap.md) - v0.1–v1.0 scope per release (was a section of `/CONTEXT.md`).
* [Success Metrics](../backlog/docs/success-metrics.md) - v0.1/v0.5/v1.0 release gates (was a section of `/competitive/positioning.md`).
* [KB → Backlog Feature Map](../backlog/docs/knowledge-base-feature-map.md) - which tasks implement which concept.
