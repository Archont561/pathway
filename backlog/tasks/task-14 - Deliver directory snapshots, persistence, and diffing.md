---
id: task-14
title: "Deliver directory snapshots, persistence, and diffing"
status: In Progress
priority: High
assignee: []
created_date: '2026-09-30'
updated_date: '2026-10-07'
labels:
  - rust
  - typescript
  - snapshot
dependencies:
  - TASK-2
  - TASK-13
milestone: m-1
type: feature
---
# Deliver directory snapshots, persistence, and diffing

## Description

This task is derived from the project knowledge base and scheduled in m-1. It is the executable backlog representation of the referenced design and roadmap material.

## Acceptance Criteria

- [ ] 100k-file snapshot and diff meets the documented performance target.
- [x] Nanosecond mtime precision and sorted-path folds make persisted snapshots deterministic.

## References

- [.knowledge/features/killer-features.md](../../.knowledge/features/killer-features.md)

## Definition of Done

- [ ] Implementation, tests, and documentation are complete.
- [x] Related knowledge-base design remains accurate after implementation.

## Implementation Notes

Three layers, one mechanism, and the JSON document is the transport.

`crates/core/src/snapshot/mod.rs` owns everything that decides what a snapshot
*is*: `Snapshot::capture` (one `NativeScanner` pass with `files_only` and
`with_metadata` forced on), a `BTreeMap<String, SnapshotEntry>` keyed by the
slash-normalised relative path, `diff`, `to_json`/`from_json`, and
`save`/`load` (atomic write — a half-written cache key that still parses is
worse than none). `crates/engine/src/snapshot.rs` is glue:
`snapshotCaptureNative` on the libuv pool, `snapshotDiffNative` and
`snapshotValidateNative` synchronous. `packages/path/src/snapshot.ts` is a
view: it keeps the document as its canonical form and parses it for reads, so
the fold and the comparison rule exist once.

Decisions worth keeping:

- **The document crosses the boundary, not a map of entries.** One crossing
  per snapshot and one per diff (D2), and — the real reason — a format whose
  two implementations can disagree is worse than no format. The TypeScript
  class never sorts, compares or re-times anything.
- **Nanoseconds are decimal strings on the wire and `bigint` on the surface.**
  A JSON number is an IEEE-754 double in every mainstream parser; a 2026
  nanosecond timestamp needs 61 bits. `Date` was rejected for the same reason.
- **Hashes decide only when both sides have one**, otherwise `size` plus
  `modifiedNanos`. A hash-to-nothing comparison has no defensible answer.
- **A foreign `format` tag is refused** (`SnapshotFormatError`) rather than
  best-effort decoded, which would diff as "the whole tree changed".
- `crates/engine/src/walk.rs` grew one extracted, behaviour-preserving
  `scan_options()` so a snapshot and a walk share one default table.

Two Windows facts the Linux sandbox could not show, both found by the CI
matrix after the first push and both now pinned:

- `std::fs::canonicalize` returns an extended-length path (`\\?\C:\project`).
  The root is the one field a snapshot always exposes — document,
  `Snapshot#root`, and every `Path` a diff returns — so the core strips the
  prefix (`UNC/` included) when recording it, pinned by a pure-string test
  that runs everywhere.
- The first suite asserted Linux in three ways: expectations built with
  `node:path.join` compare backslashes against the POSIX spelling
  `Path.value` carries; a fixture root cannot be compared against a canonical
  root that expands 8.3 short names (`RUNNER~1`); and creating a symlink is
  privileged. Assertions now use the snapshot's own root plus the file
  contents behind it, and the symlink test states its premise.

A test the filesystem refused to let us write honestly: "two writes inside one
millisecond are distinguished" was measured handing *identical* nanosecond
stamps to two back-to-back writes on this host (linux-64, overlayfs,
2026-10-07). That test would have proven the filesystem's clock tick, not the
format, so the precision claim is pinned two ways instead — the captured mtime
equals the kernel's own `stat` to the nanosecond, and two documents whose
mtimes share a millisecond diff as `modified`.

## Final Summary

Landed: directory snapshots, the `pathway-snapshot-v1` persisted format, and
diffing, across core, engine and the TypeScript surface. Suite 209 passing / 0
skipped (was 168): core 86 nextest + 50 doctests, engine 8, path 1 + 2, Bun 62
across 8 files. `pixi run --frozen gates` green locally, and PR #18 green on
all 19 checks — including the Bun and Node suites on ubuntu, macOS and
Windows, which is where the two cross-platform defects above were caught.

AC#1 (100k-file performance target) is left **unchecked and unverified**: no
benchmark for `snapshot()` exists — task-4's harness measures the walk — so
there is no evidence for "<500ms on 100k files". The knowledge base now says
so explicitly in `.knowledge/features/killer-features.md` §2 ("Verification
status: NOT VERIFIED for `snapshot()`"), and `backlog/docs/phase-plan.md`
carries the same note against the success criterion. Closing it needs a
snapshot case in `benches/walk` plus a CI sweep; the task stays In Progress
until then.
