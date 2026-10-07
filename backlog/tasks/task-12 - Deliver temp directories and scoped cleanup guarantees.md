---
id: task-12
title: "Deliver temp directories and scoped cleanup guarantees"
status: Done
priority: High
assignee: []
created_date: '2026-09-30'
updated_date: '2026-10-07'
labels:
  - rust
  - typescript
  - temp
dependencies:
  - TASK-2
milestone: m-1
type: feature
---
# Deliver temp directories and scoped cleanup guarantees

## Description

This task is derived from the project knowledge base and scheduled in m-1. It is the executable backlog representation of the referenced design and roadmap material.

## Acceptance Criteria

- [x] Normal completion, thrown callbacks, process exit, and documented SIGKILL behavior are tested.
- [x] Tier guarantees and unsupported-platform behavior are documented.

## References

- [.knowledge/features/killer-features.md](../../.knowledge/features/killer-features.md)

## Definition of Done

- [x] Implementation, tests, and documentation are complete.
- [x] Related knowledge-base design remains accurate after implementation.

## Implementation Notes

Three surfaces, one mechanism. `crates/core/src/fs/temp.rs` owns the logic:
`TempOptions { prefix, suffix, parent }`, `TempDir` (RAII guard, `close`,
`keep`, `path`), a process-wide registry of live directories, and
`cleanup_live_temp_dirs()` — a `Drop` cannot cover `process.exit()`, which runs
no destructors, so tier 1 needs a list that does not depend on unwinding. The
registry holds paths, not guards: it has to be reachable from a plain
`extern "C"` callback, and the single owner of the tree stays the `TempDir`.
`crates/engine/src/temp.rs` is the bridge (`TempDir`, `flushTempDirs()`), and
`packages/path/src/temp.ts` is the surface (`tempDir()` handle with
`remove`/`keep`, `Path.temp(options?, callback)` and `FileSystem#temp` as the
scoped sugar). The exit flush is a `process.on("exit")` listener installed on
first use — installed lazily so a process that never asks for a temp directory
never gets the listener, and owned by the host's process rather than by a
`SIGINT`/`SIGTERM` handler this library would otherwise have to install.

The tier story was corrected by measurement, not by rereading `tempfile`:

- **A temp directory is never tier 2.** `open(dir, O_TMPFILE | O_DIRECTORY | O_RDWR)`
  returns an unnamed *regular* file — the kernel ignores `O_DIRECTORY` — and
  `openat` inside it fails `ENOTDIR`. The hard SIGKILL guarantee is a property
  of unnamed temp *files*, which moved to TASK-31. The regression test
  `linux_cannot_make_a_temp_directory_anonymous` pins it so the day a release
  claims a SIGKILL-surviving temp directory, the suite says why not.
- **`tempfile` has no atexit hook.** Its guarantee is the destructor, and
  `process.exit()` runs none. Measured on Node 22.22.3 and Bun 1.3.11: an
  `exit` hook runs for `process.exit(0)` and for a `SIGINT` whose handler exits
  cleanly, and runs for neither a default-disposition `SIGINT`/`SIGTERM` nor a
  `SIGKILL`. That is exactly what the corrected
  `.knowledge/features/killer-features.md` §1 table and the `phase-plan.md`
  Phase 2 criteria now say.

Evidence: `pixi run --frozen gates` green. Suite 168 passing / 0 skipped (was
144/0). Rust: 75 nextest + 34 doctests in core (was 64 + 33), 4 in engine (was
1), 3 in path; Bun 52 across 7 files (was 43/6). TDD cycles in order: name and
liveness, `close`/`keep`, the child-process probes with the flush, the Linux
anonymous-directory pin, then a failed-removal retry (`close` unregistered
before removing, so a failed removal lost its exit-flush retry — the test
caught it). Windows behavior is documented rather than executed: this sandbox
publishes `linux-64` only, so the Windows row of the tier table is design, not
proof.

## Final Summary

TASK-12 is complete. `Path.temp(options?, callback)` scopes a directory to a
callback and removes it on return and on throw; `tempDir()` hands over the
handle for a directory that must outlive one callback, with `remove()` and
`keep()`; `FileSystem#temp` is the view-scoped form. Cleanup is tiered and
documented per platform: tier 1 covers return, throw, `process.exit()` and GC,
and tier 3 is documented *and tested as observed* for `SIGKILL` and
default-disposition signals. The tier-2 SIGKILL promise is corrected to
unnamed temp files and filed as TASK-31 rather than claimed for directories,
which the kernel does not support.
