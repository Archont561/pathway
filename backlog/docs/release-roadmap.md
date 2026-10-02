---
id: doc-release-roadmap
title: "Release Roadmap v0.1-v1.0"
type: document
status: active
created_date: '2025-07-11'
updated_date: '2026-10-02'
tags: [roadmap, release, milestones]
---

> Extracted from `.knowledge/CONTEXT.md` on 2026-10-02. The per-release scope
> table is delivery tracking and belongs with the milestones it drives
> (`m-0`-`m-3`); CONTEXT.md keeps identity, state and decisions D1-D7.
> Step-level detail lives in [phase-plan.md](phase-plan.md).

# Release Roadmap

| Phase | Scope | Key Deliverable |
|-------|-------|-----------------|
| **v0.1** | `Path`, `walk`, `read/write`, `json` serializer, `exclude` | Prove architecture. Benchmark vs fdir/Bun. |
| **v0.2** | `temp`, `hash`, `snapshot/diff`, `blake3`/`xxhash` | Build system adoption. |
| **v0.3** | `sandbox` (openat-based), `withLock`, `copyTo`/`moveTo`, `transform`, symlink controls, **`pathway-fs` Rust crate preview on crates.io (D7)** | Server & infra adoption; first Rust consumers. |
| **v0.4** | `transaction`, `detect`, `watch` (notify), CAS, native TOML/YAML serializers | Power users, monorepos. |
| **v1.0** | `resolve` (unrs-resolver adapter), streaming transforms, full CI matrix, **Rust crate stabilized (1.0 on crates.io)** | Ecosystem replacement. |
