---
id: task-2
title: Implement the fused-walk scanner in crates/core
status: To Do
priority: high
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30'
labels:
  - rust
  - core
dependencies: []
---

## Description

Phase 1 Step 1.2: the whole traversal engine in `crates/core` — napi-free by construction (D7), so the entire suite runs under `cargo nextest` with no Node. The module tree (`walk/entry.rs`, `walk/matcher.rs`, `walk/scanner.rs`) and the error type already exist; this task fills the scanner in. All semantics come from `.knowledge/implementation/code-rust-walker.md` and `.knowledge/features/walk-traversal.md`.

## Acceptance Criteria

- [ ] `NativeScanner` is implemented over the `ignore` crate.
- [ ] Glob matching uses `globset` with a **Vec of patterns, AND logic, matched against root-relative paths** (never `Vec<String>` re-parsing per entry).
- [ ] The glob-semantics matrix is unit-tested: nested and root-level `**/*.ts`, `*.ts`, and Windows separators.
- [ ] Regex filtering matches the full absolute path.
- [ ] Directory exclusion happens pre-descent (pruning): an excluded directory is never descended into.
- [ ] `dot` (default false → `hidden(true)`) and `gitignore` options are implemented.
- [ ] `absolute` option implemented; default yields root-relative paths.
- [ ] Results are yielded in batches, default batch size 512.
- [ ] `withMetadata` surfaces stat info via `DirEntry::metadata`.
- [ ] Content hashing (BLAKE3, xxhash, SHA-256) reads in **chunked 64 KB blocks — never whole-file loads**.
- [ ] Traversal and hash errors are collected (`errors()` plus per-entry `error`) instead of aborting the walk.
- [ ] `cancel()` is an `AtomicBool` check, ready to be wired to a JS `AbortSignal` in task-3.
- [ ] `crates/core` gains no `napi`/`napi-derive` dependency: `cargo test -p pathway-fs-core` passes in an environment with no Node.

## Definition of Done

- [ ] `pixi run test-rs` is green with the new suite.
- [ ] `pixi run clippy` and `pixi run fmt-check-rs` are clean.
- [ ] `pixi run deny` stays green (any new dependency with a new licence updates `deny.toml`).
