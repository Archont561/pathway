---
id: doc-knowledge-base-feature-map
title: "Knowledge Base to Backlog Feature Map"
type: document
status: active
created_date: '2026-09-30'
updated_date: '2026-09-30'
---

# Knowledge Base to Backlog Feature Map

This is the traceability index for the pathway knowledge base. Every feature
family is represented by one or more backlog tasks and assigned to the
implementation milestone that owns it.

## Milestones

| Milestone | Scope | Backlog coverage |
| --- | --- | --- |
| m-0 Phase 1 Foundation v0.1 | Pre-flight, fused walk, TypeScript Path, JSON serializer, benchmark proof, CI | Tasks 1–11 |
| m-1 Phase 2 Build System Features v0.2 | Temp directories, hashers, snapshots and diffing | Tasks 12–14 |
| m-2 Phase 3 Infrastructure Features v0.3 | Sandbox, locking, parallel I/O, symlinks, Rust crate, transformers | Tasks 15–20 |
| m-3 Phase 4 Power User Features v0.4-v1.0 | Transactions, codecs, detectors, validators, resolver, watching, CAS, release hardening | Tasks 21–27 |

## Knowledge-base traceability

| Knowledge-base area | Backlog tasks |
| --- | --- |
| `implementation/phase-plan.md` | Tasks 1–6, 9, 11–12, 14, 17–19, 25, 27 |
| `features/walk-traversal.md` | Tasks 1–4, 9, 11, 17–19, 25 |
| `features/serializers.md` | Tasks 10, 21, 22, 23 |
| `features/killer-features.md` | Tasks 12, 14–18, 21 |
| `features/pluggable-patterns.md` | Tasks 13, 20, 23, 24, 26 |
| `architecture/napi-boundary.md` | Tasks 1, 3, 10, 17, 20 |
| `architecture/fused-walk.md` | Tasks 2–4, 13–14, 17, 26 |
| `architecture/rust-crate-surface.md` | Tasks 9, 19, 27 |
| `implementation/ci-distribution.md` | Tasks 5, 6, 11, 27 |
| `competitive/landscape.md` and `positioning.md` | Tasks 4, 11, 24–27 |

## Dependency spine

`TASK-1` and `TASK-2` unblock `TASK-3`; the TypeScript surface and native
engine unblock `TASK-10`; fused traversal and hashing unblock `TASK-14` and
`TASK-26`; locking and atomic writes unblock `TASK-21`; all feature families
converge on `TASK-27` for v1.0 release hardening.

The mapping is intentionally traceable rather than duplicating full design
specifications. Task acceptance criteria are the delivery contract; the linked
knowledge-base files remain the source of design truth.
