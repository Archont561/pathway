---
id: TASK-30
title: Expose native traversal errors through the TypeScript walk API
status: Done
assignee: []
created_date: '2026-10-04 18:16'
updated_date: '2026-10-07'
labels:
  - rust
  - typescript
  - api
  - testing
dependencies:
  - TASK-3
priority: medium
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust scanner collects traversal failures and the N-API Walker exposes errors(), but packages/path/src/walk.ts never reads or surfaces that data. Define the public TypeScript error-reporting contract, wire the boundary, and add an integration regression test so unreadable or otherwise failed traversal entries are not silently lost.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The public TypeScript walk contract documents how aggregate traversal errors are surfaced.
- [x] #2 Native Walker.errors() is consumed or replaced by an equivalent boundary-safe mechanism, with no traversal error silently discarded.
- [x] #3 A test exercises the public TypeScript walk seam and verifies the error behavior.
- [x] #4 The native and TypeScript behavior is documented consistently.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 Implementation and public-seam tests are complete.
- [x] #2 The TDD and refactor skills were followed, with behavior changes separated from refactoring.
<!-- DOD:END -->

## Implementation Notes

`drive()` in `packages/path/src/walk.ts` now reads `walker.errors()` once,
after the last batch, and throws the exported `WalkError` (mirroring
`ContainmentError`: `name` plus verbatim `errors: readonly string[]`) when the
native walk collected failures. One synchronous bulk crossing per walk, never a
crossing per failure. Entries that fail `stat`/hashing are still yielded with
`PathEntry.error` mid-stream; failures that produce no entry at all (an
unreadable directory) are reported only by this throw. `signal.reason` still
wins over a collected failure, and an early consumer `break` stays silent (a
cancelled walk is not an error). Side effect, in the direction of correctness:
TS `hashTree` now fails loudly on a partial tree instead of digesting it
silently, matching core's `hash_tree`, which already treats aggregate errors
as fatal. No Rust logic changed (engine `errors()` rustdoc only).

Two regression tests at the public TS seam (`walkFiles` via `src/index.js`),
mirroring core's `an_unreadable_file_is_reported_without_aborting_the_walk`:
chmod-000 file plus `{ hash: "blake3" }`, premise-guarded (readable means root
or an inapplicable mode bit, so the test returns early instead of failing).
The premise-check block is duplicated across the two tests; task-28's TS
fixture kit should absorb it.

Evidence: `pixi run --frozen test` 144 passing / 0 skipped (was 142/0);
`pixi run --frozen gates` (lint, typecheck, test) green. TDD red → green in
two slices (rejection + error content, then stream-then-throw ordering); no
refactoring was performed, so behavior/refactor separation holds vacuously.

## Final Summary

TASK-30 is complete. Native traversal failures reach TypeScript callers as a
`WalkError` at the end of iteration instead of being silently discarded, with
the contract documented in `walk()`/`WalkError` TSDoc, the `PathEntry.error`
tolerance wording, the engine `errors()` rustdoc (same 1,000 cap on both
sides), and the quick-start guide.
