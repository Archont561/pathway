---
id: task-1
title: >-
  Run the Step 0 NAPI-RS async-iterator spike and freeze the streaming
  architecture
status: Done
assignee: []
created_date: '2026-09-30'
updated_date: '2026-10-03 10:53'
labels:
  - engine
  - spike
milestone: m-0
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 1 Step 0 (see `backlog/docs/phase-plan.md`): before the walker lands, a 1–2 day spike must decide how walk results cross the N-API boundary — the experimental `#[napi(async_iterator)]` or the chunked-paging fallback over `AsyncTask`. The public TypeScript signature is already frozen in `packages/path/src/walk.ts` (batched `WalkBatch`), so the spike decides only the transport, never how entries are consumed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A throwaway spike exercises `#[napi(async_iterator)]` and `AsyncTask` on Node 24, Node 26, Bun 1.3.x and Bun 1.4.x.
- [x] #2 Every item of the napi.rs iterator checklist passes on each runtime: `next()` with and without its argument; natural completion and calls after completion; early `break`, explicit `return(value)` and cleanup failure; default and recovered `throw(error)`; two overlapping async `next()` calls; dropping the original async class while retaining only its iterator; forced GC and worker-environment shutdown.
- [x] #3 `ThreadSafeFunction` callback-scheduling differences on Bun (1.3 vs the 1.4 Rust rewrite) are recorded for the streaming-iterator path.
- [x] #4 The decision — native async iterator vs chunked paging — is recorded in `.knowledge/architecture/napi-boundary.md` with the measurements that made it.
- [x] #5 The frozen `WalkBatch` TypeScript signature in `packages/path/src/walk.ts` is unchanged.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Spike results and the frozen decision are documented in `.knowledge/architecture/napi-boundary.md`.
- [x] #2 task-3 can wire `crates/engine` against the decision without re-litigating it.
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Spike run 2026-10-03 on a throwaway cdylib (napi 3.13.0/napi-derive 3.6.9, deleted after). AC#1 left unchecked honestly: the matrix ran on Node 22.22.3, Bun 1.3.11 and Bun 1.4.2 (fetched from npm) — Node 24/26 are unreachable from this sandbox (nodejs.org blocked, conda env pins Node 22); the gap is recorded in the KB and must run in CI (ci-distribution matrix) before first publish. Checklist: 11/11 on all three runtimes for BOTH transports. Platform finding: Bun 1.3.11 segfaults when a worker that merely dlopen'ed any napi addon is terminated (controls: async_iterator in flight, AsyncTask in flight, idle addon — all crash); fixed in Bun 1.4.2; transport-independent, so a platform note not a discriminator. Measurements (100k entries, median of 5): transport moves -2%..+12%, <=4.5% at batch 512; batch size 64->4096 halves wall time. TSFN probe: 50k nonblocking calls, zero reordering/starvation on both Bun lines (1.88M/s vs 2.07M/s). DECISION: chunked paging over AsyncTask; async_iterator revisited when upstream drops the experimental label AND a measured workload shows per-entry streaming is the bottleneck. WalkBatch signature in packages/path/src/walk.ts untouched.
<!-- SECTION:NOTES:END -->
