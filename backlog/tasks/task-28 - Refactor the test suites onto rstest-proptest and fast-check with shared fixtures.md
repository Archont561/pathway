---
id: task-28
title: >-
  Refactor the test suites onto rstest/proptest and fast-check with shared
  fixtures
status: To Do
assignee: []
created_date: '2026-10-02'
updated_date: '2026-10-03 10:43'
labels:
  - rust
  - typescript
  - testing
  - tooling
  - dx
milestone: m-0
dependencies:
  - TASK-2
references:
  - backlog/docs/test-refactor-plan.md
priority: high
type: chore
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The suite that landed with task-2 is 54 unit tests and 2 doctests, all of them
example-based and all of their setup hand-rolled. That was the right shape for
proving the scanner works; it is the wrong shape for proving it works *for every
input*, and the seams are already visible:

- **`crates/core/src/walk/scanner.rs`** carries four private helpers — `tree()`,
  `collect()`, `paths()`, `scan()` — that are a fixture kit written by hand.
  They are only reachable from that one `mod tests`, so `matcher.rs` grew its
  own `globs()` and `accepts()` doing the same job differently, and a future
  `fs/temp.rs` or `fs/lock.rs` suite will grow a third set.
- **The glob matrix is five near-identical `#[test]` functions.** Each builds a
  `Matcher`, asserts a handful of paths, and differs only in the pattern and the
  expected verdict. That is a table, written out longhand, and it is exactly
  what `rstest`'s `#[case]` exists for.
- **Two tests are properties pretending to be examples.**
  `a_digest_does_not_depend_on_the_chunk_boundaries` asserts a universal
  invariant against one hand-picked payload size (`CHUNK_SIZE * 2 + 1337`), and
  `every_algorithm_round_trips_through_its_name` loops a three-element array.
  Both are stronger as generated properties.
- **The TypeScript side has one test file and no fixtures at all.**
  `packages/path/test/path.test.ts` reads and parses the workspace `Cargo.toml`
  inline inside the suite.

This task adopts the convention Archont561/geoquery settled on, which splits the
two jobs cleanly: **`rstest` builds the values and parameterises the examples,
`proptest` generates the inputs for the invariants that cannot be checked one
example at a time** — and, on the TypeScript side, **`fast-check` for the
properties plus a shared `createFixture` helper for the setup/teardown
lifecycle**.

The goal is not coverage for its own sake — the workspace is already at 95.51%
region / 94.81% line. It is that the *classes* of input nobody thought to write
down get exercised, and that adding the next suite (task-12 temp dirs, task-16
locking, task-18 symlinks) costs a fixture import rather than a fresh copy of
the same tree-building code.

---

## ⚠️ Prerequisite: the dependencies are declared, the transport is not

**Read this before picking the task up in a restored sandbox.** Originally
verified 2026-10-02, when none of the three existed anywhere; re-verified
2026-10-03 on a connected machine, where steps 1–3 have since landed:

| Dependency | Resolved | Declared at |
| --- | --- | --- |
| `rstest` | `0.27.0` | root `Cargo.toml` `[workspace.dependencies]`, `crates/core` + `crates/path` `[dev-dependencies]` |
| `proptest` | `1.11.0` | same two manifests, same sections |
| `fast-check` | `4.10.2` | `packages/path` `devDependencies` |

Both lockfiles carry them (`Cargo.lock`, `bun.lock`), and
`cargo deny check bans licenses sources` is green with them in the graph — all
three are MIT/Apache-2.0, so `deny.toml` needed no edit.

An airlocked machine still cannot use them. `pixi add` and `cargo add` both solve
before they write, and prefix.dev and the crates.io index are precisely what the
airlock cannot reach, so this was a **connected-side** step and it is only
partly done:

1. ~~Add `rstest` and `proptest` to `[workspace.dependencies]` in the root
   `Cargo.toml` and `fast-check` to the consuming package's `devDependencies`.~~
   **Done.**
2. ~~Let the connected side refresh `Cargo.lock` and `bun.lock`.~~ **Done.**
3. ~~Check the new licences against `deny.toml`.~~ **Done** — green, no edits
   needed.
4. **Repack and republish the sandbox transport**
   (`sandbox-pack` → `sandbox-doctor` → `sandbox-publish`). A merged lockfile
   does not make a crate usable in the airlock; only a transport carrying it
   does. **Still outstanding.**

Until step 4 lands, `cargo build --offline` in a restored environment will fail
on the new crates. Bounds, as declared: `rstest >=0.27,<0.28`, `proptest >=1.6,<2`,
`fast-check ^4.10.2`.

`rstest` was bumped off the `>=0.24,<0.26` bound this task originally suggested:
0.26 and 0.27 are additive (0.27 raises MSRV to 1.85.0, still under the
workspace's 1.98), and 0.26 drops the default `async-std` dependency and adds
folder support to `#[files(...)]`, both of which this task benefits from.

All three are **dev-only**. Nothing in `src/` may depend on a test framework: a
crate that needs `proptest` to build is a crate whose consumers need `proptest`
too. In Rust they go under `[dev-dependencies]` of the crates that use them,
with the version stated once in `[workspace.dependencies]`.

---
<!-- SECTION:DESCRIPTION:END -->

# Refactor the test suites onto rstest/proptest and fast-check with shared fixtures

## Acceptance Criteria

### Shared fixtures — Rust

- [ ] #1 A `crates/core/tests/support/mod.rs` (or an equivalent `#[cfg(test)]`
      module reachable from every suite) owns tree construction, replacing the
      private `tree()`/`collect()`/`paths()`/`scan()` helpers in `scanner.rs`
      and `globs()`/`accepts()` in `matcher.rs`.
- [ ] #2 Building a tree is one expression — a fixture takes a description of
      the tree (paths, and where it matters contents, sizes and modes) and
      returns a live `TempDir`, so a new suite writes no `create_dir_all` loop.
- [ ] #3 `#[fixture]` is used for the values a suite sets up per test, so a
      `TempDir`'s lifetime is managed by rstest rather than by a `let dir =`
      the test has to remember to keep alive.
- [ ] #4 The glob-semantics matrix in `matcher.rs` becomes `#[rstest]` with
      named `#[case]`s. A failing case must name the pattern and the path in
      its test name, so the failure reads without opening the file.

### Properties — Rust (`proptest`)

- [ ] #5 **Hashing is independent of chunk boundaries.** Generate payload sizes
      spanning the `CHUNK_SIZE` boundary (0, 1, `CHUNK_SIZE - 1`, `CHUNK_SIZE`,
      `CHUNK_SIZE + 1`, multiples, and arbitrary) and assert the file digest
      equals the in-memory digest for all three algorithms. This replaces the
      single hand-picked size.
- [ ] #6 **`Algorithm::from_name(a.name()) == a`** for every algorithm, and any
      string that is not one of the three is rejected.
- [ ] #7 **`*` never crosses a separator and `**` always does**, over generated
      path components rather than the four written out today.
- [ ] #8 **Glob AND semantics are conjunction:** for generated pattern sets and
      paths, `Matcher::new(&[a, b]).accepts(p)` iff
      `Matcher::new(&[a]).accepts(p) && Matcher::new(&[b]).accepts(p)`. This is
      the invariant the `GlobSet::is_match` OR bug violated.
- [ ] #9 **Negation is complement:** `!p` accepts exactly what `p` rejects,
      modulo the other patterns in the set.
- [ ] #10 **Batching loses and duplicates nothing.** For a generated tree and a
      generated `batch_size`, the concatenation of every batch equals the full
      result set exactly once. This is the invariant the reference's
      cursor/drain bug violated, and an example test only catches it at the
      sizes someone wrote down.
- [ ] #11 **Concurrency does not change the result set.** For a generated tree,
      the sorted output is identical for 1, 2, 4 and 8 threads.
- [ ] #12 **`absolute` and relative outputs are related by the root:** every
      relative result `r` satisfies `root.join(r) == a` for the corresponding
      absolute result.
- [ ] #13 **Pruning is exact:** for a generated tree and a generated exclusion
      set, the results are precisely the files whose path contains no excluded
      directory component.
- [ ] #14 `proptest-regressions/` seed files are **committed to version
      control**, so a counterexample found once is re-run by everyone
      afterwards. Add the path to `.gitignore` exceptions if a glob would
      otherwise drop it.
- [ ] #15 Property runs are bounded so the suite stays fast — state the case
      count where it differs from the default, and keep `pixi run test` under a
      wall-clock budget recorded in the task's notes.

### Edge cases — must be covered by name

The brief is "every edge case handled", so these are enumerated rather than
left to judgement. Each is either a `#[case]` or a generator constraint:

- [ ] #16 **Empty inputs:** empty directory, zero-byte file, empty glob list,
      empty exclusion list, `batch_size` of 0 and 1, `max_depth` of 0.
- [ ] #17 **Names:** Unicode and emoji filenames, names with spaces, a name
      containing a newline, a name containing `[`/`]`/`{`/`}`/`*` (glob
      metacharacters as literals), a leading-dot file, and a **non-UTF-8**
      filename — which `scanner.rs` documents as walked rather than pruned, and
      nothing currently asserts.
- [ ] #18 **Sizes:** a file larger than one chunk, exactly one chunk, and one
      byte either side of a chunk.
- [ ] #19 **Shape:** deeply nested trees (depth > 32), a very wide directory, a
      root that is itself excluded by name, a root that is a file rather than a
      directory, and a root that does not exist.
- [ ] #20 **Links and permissions:** a symlink to a file, a symlink to a
      directory, a **broken** symlink, a symlink cycle, an unreadable file and
      an unreadable directory. Permission cases must skip rather than fail when
      the suite runs as root — the two existing tests already do this and the
      pattern should move into the fixture kit.
- [ ] #21 **Cancellation:** cancelled before `scan()`, and cancelled during a
      walk large enough that the flag is observed mid-traversal.

### TypeScript — `@repo/test-utils` and `fast-check`

- [ ] #22 A new private workspace package provides `createFixture(setup,
      teardown?, scope?)` with `"test"` (per-test, the default) and `"file"`
      (per-file) scopes, modelled on `@geoquery/utils`. It is `private: true`,
      `main` points at `src/`, and nothing in a published package imports it.
      Name it under the existing `@repo/*` internal convention
      (`@repo/test-utils`) rather than inventing a second scope — the repo
      already uses `@repo/` for `typescript-config` and `rust`.
- [ ] #23 The accessor **throws** when read before its scope's setup has run,
      naming the scope, so a fixture wired into the wrong scope fails at the
      call site instead of making every assertion fail on `undefined`.
- [ ] #24 Teardown is skipped when setup did not finish, so a half-built fixture
      is never torn down as if it were whole.
- [ ] #25 A `tempTree` fixture mirrors the Rust one, so a TypeScript walk test
      describes its tree the same way a Rust one does.
- [ ] #26 `packages/path/test/path.test.ts` uses the fixtures, and its inline
      `Cargo.toml` parsing moves into one.
- [ ] #27 `fast-check` properties cover the `Path` string surface, which is pure
      and therefore the easiest place for properties to pay: `normalise` is
      idempotent, `join` then `parent` round-trips, `new Path(p).value` is
      stable under repeated construction, `stem + ext` reconstructs `name`, and
      POSIX separators are preserved on every platform.
- [ ] #28 Property tests live in a separate file or directory from the
      example tests (geoquery uses `test/property/`), because the two fail
      differently: an example names the behaviour it documents, a property
      hands back a shrunk counterexample.

### Conventions, written down

- [ ] #29 `AGENTS.md`'s Testing section states the rule: **`#[case]`/examples
      for finite, enumerable inputs; properties for universal contracts;
      fixtures for anything set up more than once** — and that
      `proptest-regressions` is committed.
- [ ] #30 The same three sentences appear in the README's
      "Development & Quality Gates" section, where a contributor looks first.

---

## Non-goals

- Raising the coverage number. It is already 95.51% region; this task is about
  the inputs the existing lines are exercised with, and the percentage may
  barely move.
- Rewriting tests that are already in the right shape. The error-type tests in
  `crates/core/src/error.rs` and the `FusedEntry` test in `walk/entry.rs` are
  examples documenting a decision, which is what examples are for.
- Benchmarks. Performance measurement is task-4 and must not be smuggled in
  behind a property test's case count.

## References

- [backlog/docs/phase-plan.md](../docs/phase-plan.md) — Phase 1 Step 1.2
- [.knowledge/implementation/code-rust-walker.md](../../.knowledge/implementation/code-rust-walker.md) — the glob matrix this task parameterises
- [.knowledge/features/walk-traversal.md](../../.knowledge/features/walk-traversal.md) — AND semantics and pattern rules
- Prior art: `Archont561/geoquery` — `packages/utils/src/fixtures.ts`
  (`createFixture`), `packages/client/test/property/index.property.test.ts`
  (fast-check), `crates/protocol/tests/lib.rs` (`#[fixture]` + `prop_recursive`),
  and its committed `crates/protocol/proptest-regressions/lib.txt`

## Definition of Done

- [ ] `pixi run gates` is green, and `pixi run test` stays within the recorded
      wall-clock budget.
- [ ] `cargo deny check bans licenses sources` is green with the new
      dev-dependencies, and `deny.toml` is updated if any licence is new.
- [ ] `cargo tree -p pathway-fs-core -e normal` still contains neither
      `proptest` nor `rstest` — they are dev-only, and a normal-edges tree is
      how that is proven rather than asserted.
- [ ] The sandbox transport has been repacked and republished, and
      `sh scripts/restore.sh && pixi run -e default cargo build --offline` works
      from a clean restore.
- [ ] Implementation, tests, and documentation are complete.
- [ ] Related knowledge-base design remains accurate after implementation.

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Sequenced in backlog/docs/test-refactor-plan.md (2026-10-03). Summary:

- Phase A (no new deps, offline-safe): A1 Rust fixture kit extraction (AC #1-#3 groundwork, #20 skip pattern); A2 @repo/test-utils with createFixture + tempTree (AC #22-#25); A3 path.test.ts onto fixtures (AC #26); A4 conventions + agent rule with task-29 (AC #29-#30).
- Phase B (new dev-deps): B1 fast-check properties (AC #27-#28) — npm IS reachable from the current restored sandbox, verified 2026-10-03; B2 rstest adoption (AC #3-#4, #16-#19); B3 proptest invariants (AC #5-#15, #16-#21).
- Phase C: budgets, cargo-deny, dev-only proof via cargo tree, connected-side transport repack.

Constraint update (2026-10-03, restored sandbox with partial egress): registry.npmjs.org and index.crates.io reachable; static.crates.io blocked, so cargo can resolve but not download rstest/proptest — the connected-side prerequisite stands for the Rust half only. Decision gate on the Rust route (prescribed connected-side vs interim fastrand-based properties) is section 6 of the plan.
<!-- SECTION:PLAN:END -->
