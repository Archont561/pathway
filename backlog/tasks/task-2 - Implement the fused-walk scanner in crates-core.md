---
id: task-2
title: Implement the fused-walk scanner in crates/core
status: Done
assignee: []
created_date: '2026-09-30'
updated_date: '2026-10-02'
labels:
  - rust
  - core
milestone: m-0
dependencies: []
priority: high
---

## Description

Phase 1 Step 1.2: the whole traversal engine in `crates/core` — napi-free by construction (D7), so the entire suite runs under `cargo nextest` with no Node. The module tree (`walk/entry.rs`, `walk/matcher.rs`, `walk/scanner.rs`) and the error type already exist; this task fills the scanner in. All semantics come from `.knowledge/implementation/code-rust-walker.md` and `.knowledge/features/walk-traversal.md`.

## Acceptance Criteria

- [x] `NativeScanner` is implemented over the `ignore` crate.
- [x] Glob matching uses `globset` with a **Vec of patterns, AND logic, matched against root-relative paths** (never `Vec<String>` re-parsing per entry).
- [x] The glob-semantics matrix is unit-tested: nested and root-level `**/*.ts`, `*.ts`, and Windows separators.
- [x] Regex filtering matches the full absolute path.
- [x] Directory exclusion happens pre-descent (pruning): an excluded directory is never descended into.
- [x] `dot` (default false → `hidden(true)`) and `gitignore` options are implemented.
- [x] `absolute` option implemented; default yields root-relative paths.
- [x] Results are yielded in batches, default batch size 512.
- [x] `withMetadata` surfaces stat info via `DirEntry::metadata`.
- [x] Content hashing (BLAKE3, xxhash, SHA-256) reads in **chunked 64 KB blocks — never whole-file loads**.
- [x] Traversal and hash errors are collected (`errors()` plus per-entry `error`) instead of aborting the walk.
- [x] `cancel()` is an `AtomicBool` check, ready to be wired to a JS `AbortSignal` in task-3.
- [x] `crates/core` gains no `napi`/`napi-derive` dependency: `cargo test -p pathway-fs-core` passes in an environment with no Node.

## Definition of Done

- [x] `pixi run test-rs` is green with the new suite.
- [x] `pixi run clippy` and `pixi run fmt-check-rs` are clean.
- [x] `pixi run deny` stays green (any new dependency with a new licence updates `deny.toml`).

> The Definition of Done names `test-rs`, `clippy`, `fmt-check-rs` and `deny`,
> which were folded into the repo-wide verbs by task-7. The equivalents were
> run: `pixi run gates` (lint + typecheck + test) is green, and
> `cargo deny check bans licenses sources` reports `bans ok, licenses ok,
> sources ok`. No new dependency was added — every crate used was already
> declared in `[workspace.dependencies]` and already in the vendor tree.

## Implementation Notes

Landed 2026-10-02. Everything is in `crates/core`; nothing in `crates/engine`
was touched, so the N-API bridge is still task-3.

**Three defects in the reference implementation were found and not copied.**

1. **`GlobSet::is_match` is OR, and the specified semantics are AND.** The
   reference compiles every pattern into one `GlobSet`, so `["**/*.ts",
   "src/**"]` accepted any `.ts` anywhere *or* anything under `src`.
   `walk-traversal.md` says an entry must match **all** patterns. Positives are
   now individual matchers combined with `all`; a `GlobSet` is kept for
   `!`-prefixed negations, where OR is the correct semantics.
2. **`globset` does not bound `*` at `/` by default.** `literal_separator(true)`
   has to be opted into, and without it `*.ts` matches `src/a.ts` — the
   distinction between `*.ts` and `**/*.ts` does not exist. This was caught by
   the glob matrix the task asked for, which is the matrix earning its place.
3. **The cursor and the drain contradict each other.** The reference does
   `*cursor = end; results.drain(start..end)`. Draining shifts every later
   index down, so the second call reads from `512` in a vector whose element
   `512` is what used to be `1024`: half the walk is silently skipped. Fixed by
   dropping the cursor — `FusedEntry` is deliberately not `Clone`, so batches
   must be moved out and there is nothing for the reference's `reset()` to do.
   `reset()` is therefore not part of the core API.

**Two deliberate deviations.**

- **The walk is parallel.** The reference calls `WalkBuilder::build()`, the
  single-threaded iterator, while its own notes claim the builder spawns
  workers. Only `build_parallel()` does, and the fused-walk claim depends on it
  because hashing runs per entry. Consequence: **yield order is unspecified**,
  which the streaming target (task-1/task-3) could not have honoured anyway.
  Per-worker buffers flush on `Drop` so the shared mutex is locked once per
  thread rather than once per entry.
- **`require_git(false)`.** Without it `ignore` applies `.gitignore` only inside
  a git repository, so `gitignore: true` would silently do nothing when walking
  an extracted tarball.

**Verification.** 54 unit tests plus 2 doctests, all green. `cargo tree -p
pathway-fs-core` contains **zero** napi crates (AC #13). Coverage over the
workspace moved from 75.68% region / 80.00% line to **95.51% / 94.81%**;
`walk/scanner.rs` is at 97.38% and `walk/matcher.rs` at 98.01%.

**Side effect on task-8.** The doctest gate reported `0 passed; 0 failed` for
both crates and so could not fail. It now runs 2 doctests, and a deliberately
broken example was confirmed to fail the gate. Task-8's AC #2 and #3 are
satisfied for the surface that exists; its AC #1 (every public item documented
or excused) still needs a pass over the whole crate.
