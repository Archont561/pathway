---
okf_version: "0.2"
---

# @myorg/path — Knowledge Base

Native, pathlib-inspired filesystem API for TypeScript, Bun, and Node,
built on a 3-layer stack: ergonomic TS surface → NAPI-RS bridge → Rust
engine. One file = one concept (OKF v0.2 bundle).

Read [Project Context](/CONTEXT.md) first for current state and the
foundational decisions. Change history: [update log](/log.md).

# Context

* [Project Context & Decision Log](/CONTEXT.md) - Identity, current state, decisions D1–D6, market snapshot, roadmap, bundle conventions.

# Architecture

* [3-Layer Architecture](/architecture/core-layers.md) - TS surface, NAPI-RS bridge, Rust engine; Path vs FileSystem; the Rust-internal trait.
* [Coarse-Grained FFI Boundary](/architecture/napi-boundary.md) - What stays in TypeScript, what crosses to Rust; per-call boundary hops banned.
* [Fused Walk](/architecture/fused-walk.md) - Single-pass stat+hash+filter in Rust — the defensible performance moat.

# Features

* [Walk & Traversal](/features/walk-traversal.md) - Glob/regex matching, excludes, pruning, predicates, async iteration, batching.
* [Serializers](/features/serializers.md) - Serializer&lt;T&gt;, Serde codecs, per-FileSystem registry, typed read/write.
* [Killer Features](/features/killer-features.md) - Temp dirs, snapshots/diff, sandbox, transactions, locking, parallel ops.
* [Pluggable Patterns](/features/pluggable-patterns.md) - Transformers, hashers, detectors, validators, resolvers, CAS.

# Competitive Intelligence

* [Landscape](/competitive/landscape.md) - Tiers 1–5 library map and the gap matrix @myorg/path targets.
* [Verified Market Data](/competitive/verified-data.md) - Node 24/26, stable `node:fs.glob`, Bun 1.3/1.4, Deno 2, NAPI-RS state; Round-1 corrections.
* [Positioning](/competitive/positioning.md) - Target audience, one-line pitch, Bun-as-ally posture, messaging guardrails.

# Implementation

* [Repo Structure](/implementation/repo-structure.md) - Cargo workspace, npm/TypeScript package tree, directory layout.
* [Phase Plan](/implementation/phase-plan.md) - Phase 1–4 steps, v0.1–v1.0 roadmap, benchmark gates.
* [CI & Distribution](/implementation/ci-distribution.md) - NAPI-RS targets, platform matrix, Bun CI, provenance hardening.
* [Rust Walker (Reference)](/implementation/code-rust-walker.md) - NativeScanner over the `ignore` crate; chunked fused batches.
* [TypeScript Path (Reference)](/implementation/code-ts-path.md) - `Path` class, `Serializer<T>`, `WalkIterator`, `FileSystem`.
