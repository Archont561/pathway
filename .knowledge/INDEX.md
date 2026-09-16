---
title: "Knowledge Base Index"
domain: meta
status: active
created: 2025-07-11
updated: 2026-09-16
source: conversation
depends_on: []
tags: [index, navigation]
---

# @myorg/path — Knowledge Base Index

Master link graph for all project knowledge.
Read [CONTEXT.md](./CONTEXT.md) first for project state and decisions.

---

## Architecture

| File | Title | Status |
|------|-------|--------|
| [core-layers.md](./architecture/core-layers.md) | 3-Layer Model, Path vs FileSystem, Rust Trait | `pending` |
| [napi-boundary.md](./architecture/napi-boundary.md) | Coarse-Grained FFI Boundary, JS vs Rust Split | `pending` |
| [fused-walk.md](./architecture/fused-walk.md) | Fused Walk: Single-Pass Stat+Hash+Filter | `pending` |

## Features

| File | Title | Status |
|------|-------|--------|
| [walk-traversal.md](./features/walk-traversal.md) | Walk Engine, Glob/Regex/Exclude, Pruning, Predicates | `pending` |
| [serializers.md](./features/serializers.md) | Serializer Pattern, Serde Codecs, Registry | `pending` |
| [killer-features.md](./features/killer-features.md) | Temp Dirs, Snapshots, Sandbox, Transactions, Locking | `pending` |
| [pluggable-patterns.md](./features/pluggable-patterns.md) | Transformers, Hashers, Detectors, Validators, Resolvers | `pending` |

## Competitive Intelligence

| File | Title | Status |
|------|-------|--------|
| [landscape.md](./competitive/landscape.md) | Tiers 1–5: Full Library Map and Gap Matrix | `pending` |
| [verified-data.md](./competitive/verified-data.md) | Web Search Corrections: tinyglobby, Bun.Glob, fdir | `pending` |
| [positioning.md](./competitive/positioning.md) | Strategic Conclusions, Target Audience, Pitch | `pending` |

## Implementation

| File | Title | Status |
|------|-------|--------|
| [repo-structure.md](./implementation/repo-structure.md) | Workspace Layout, Cargo/TS Tree | `pending` |
| [phase-plan.md](./implementation/phase-plan.md) | Phase 1–4 Steps, v0.1–v1.0 Roadmap, Benchmarks | `pending` |
| [ci-distribution.md](./implementation/ci-distribution.md) | NAPI-RS Targets, Platform Matrix, Bun CI | `pending` |
| [code-rust-walker.md](./implementation/code-rust-walker.md) | Rust NativeScanner + ignore Crate | `pending` |
| [code-ts-path.md](./implementation/code-ts-path.md) | TypeScript Path Class + Serializer<T> | `pending` |
