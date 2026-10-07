---
id: TASK-31
title: Deliver unnamed temp files for the tier-2 SIGKILL guarantee
status: To Do
assignee: []
created_date: '2026-10-07 13:27'
labels:
  - rust
  - typescript
  - temp
milestone: m-1
dependencies:
  - TASK-2
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The tier-2 row of the cleanup guarantee is a property of *unnamed files*: an
inode with no directory entry that the kernel reclaims when the last handle
closes, even after a `SIGKILL`. TASK-12 measured that it cannot be a property
of a directory — `open(dir, O_TMPFILE | O_DIRECTORY | O_RDWR)` ignores the
`O_DIRECTORY` bit and returns a regular file, and `openat` inside it fails
`ENOTDIR` — so the directory half of the guarantee stops at tier 1 and the
tier-2 promise moved here.

This task delivers the file primitive the tier table actually describes:
`TempFile`, backed by `tempfile::tempfile()` (which already selects
`O_TMPFILE` on Linux and `FILE_FLAG_DELETE_ON_CLOSE` on Windows, falling back
to create-then-unlink where the flags are unsupported), exposed as
`{ prefix, suffix, dir }` on both surfaces, plus the rules for the cases that
follow from having no path: no `keep()` to a visible path, reads and writes go
through the handle, and a caller that needs a name must not use this type.

The measurement to pin: a child process that creates an unnamed file, writes
contents and is `SIGKILL`ed leaves **no directory entry** behind, which is the
observable difference from `TempDir` and the reason this type exists. The
crates.io-facing `pathway-fs` Rust surface ships it in the same task; the
TypeScript wrapper mirrors `tempDir()` in shape.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Unnamed temp files are created with O_TMPFILE on Linux and FILE_FLAG_DELETE_ON_CLOSE on Windows, with the named-file fallback documented where the flags are unsupported.
- [ ] #2 The SIGKILL guarantee is tested on Linux: a killed process leaves no directory entry behind, while the data itself remains reachable to the OS until the last handle closes.
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 Implementation, tests, and documentation are complete, and the killer-features tier-2 row is narrowed or confirmed against the measurement.
- [ ] #2 Related knowledge-base design remains accurate after implementation.
<!-- DOD:END -->
