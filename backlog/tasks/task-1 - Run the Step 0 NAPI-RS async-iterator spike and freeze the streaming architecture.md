---
id: task-1
title: >-
  Run the Step 0 NAPI-RS async-iterator spike and freeze the streaming
  architecture
status: To Do
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30 20:24'
labels:
  - engine
  - spike
milestone: m-0
dependencies: []
priority: high
---

## Description

Phase 1 Step 0 (see `.knowledge/implementation/phase-plan.md`): before the walker lands, a 1–2 day spike must decide how walk results cross the N-API boundary — the experimental `#[napi(async_iterator)]` or the chunked-paging fallback over `AsyncTask`. The public TypeScript signature is already frozen in `packages/path/src/walk.ts` (batched `WalkBatch`), so the spike decides only the transport, never how entries are consumed.

## Acceptance Criteria

- [ ] A throwaway spike exercises `#[napi(async_iterator)]` and `AsyncTask` on Node 24, Node 26, Bun 1.3.x and Bun 1.4.x.
- [ ] Every item of the napi.rs iterator checklist passes on each runtime: `next()` with and without its argument; natural completion and calls after completion; early `break`, explicit `return(value)` and cleanup failure; default and recovered `throw(error)`; two overlapping async `next()` calls; dropping the original async class while retaining only its iterator; forced GC and worker-environment shutdown.
- [ ] `ThreadSafeFunction` callback-scheduling differences on Bun (1.3 vs the 1.4 Rust rewrite) are recorded for the streaming-iterator path.
- [ ] The decision — native async iterator vs chunked paging — is recorded in `.knowledge/architecture/napi-boundary.md` with the measurements that made it.
- [ ] The frozen `WalkBatch` TypeScript signature in `packages/path/src/walk.ts` is unchanged.

## Definition of Done

- [ ] Spike results and the frozen decision are documented in `.knowledge/architecture/napi-boundary.md`.
- [ ] task-3 can wire `crates/engine` against the decision without re-litigating it.
