---
id: TASK-8
title: 'Make the Rust doctest gate real: the workspace has zero doctests'
status: To Do
assignee: []
created_date: '2026-09-30 13:01'
updated_date: '2026-09-30 20:24'
labels:
  - rust
  - testing
  - tooling
milestone: m-0
dependencies: []
priority: medium
type: chore
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Rust doctest step (`cargo test --doc -p pathway-fs-core -p pathway-fs`, the `test:doc` script of `@repo/rust`) reports `0 passed; 0 failed; 0 ignored` for **both** crates. There is not a single doctest in the workspace, so the gate runs in CI and in `pixi run gates` but cannot fail.

This was measured on 2026-09-30 while verifying task-5 on a restored full toolchain.

The task is not dead weight — it is a gate waiting for its subject. `pixi.toml` justifies it on the grounds that "a public Rust API (D7, two crates.io artefacts) is only as documented as its examples compile", and D7 does put `pathway-fs-core` and `pathway-fs` on crates.io. A published crate whose rustdoc examples have never been compiled is exactly the failure this is meant to catch; today nothing is being caught.

Related signal from the same session: `pixi run coverage` reports 75.68% region / 80.00% line coverage, with `core/src/fs/atomic.rs` and `engine/src/lib.rs` at 0%. Doctests on the public surface would lift both numbers and the documentation at once.

Two ways to close this, and the choice is the point of the task: write doctests for the public API as it stands, or accept that the API is too small to document yet and drop the task until task-2/task-3 land a real surface. Do not simply delete the gate — if it goes, say in `pixi.toml` when it comes back.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every public item in `pathway-fs-core` and `pathway-fs` either carries a rustdoc example or has a recorded reason it does not
- [ ] #2 `bun run --cwd crates test:doc` reports a non-zero number of doctests run
- [ ] #3 A deliberately broken example in a doc comment makes `pixi run test` fail (verify by temporarily breaking one)
- [ ] #4 If the decision is instead to remove the gate, `crates/package.json` drops `test:doc` from its `test` script and records in a comment why and when it should return
<!-- AC:END -->
