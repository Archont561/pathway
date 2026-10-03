# Test-suite refactor plan — shared fixtures and property-based testing

Status: **proposed** · Owner: task-28 (implementation), task-29 (agent rule)
Method: `.agents/skills/refactor` (behaviour-preserving, small steps, gates
green between every step) · Test discipline: `.agents/skills/tdd`

This plan operationalises **task-28** across the whole repo and records the
constraint report that decides *how much of it* this environment can execute.
It does not change the task's acceptance criteria; it sequences them.

---

## 1. Purpose

The refactor skill's first rule is that refactoring needs a clear purpose, and
task-28 states it: the suite that landed with task-2 proves the scanner works,
not that it works *for every input*. Setup is hand-rolled twice, tables are
written out longhand, and two universal invariants are asserted against one
hand-picked example each. The next three suites (task-12 temp dirs, task-16
locking, task-18 symlinks) will each copy the same tree-building code unless a
fixture kit lands first.

Behaviour under refactor here is **what the tests verify**, not what the
engine does: every step below must leave the set of verified behaviours equal
or strictly larger, never smaller, and `pixi run gates` green.

## 2. Current state — the smell inventory

Mapped to the refactor skill's catalogue:

| Smell | Where | Evidence |
| --- | --- | --- |
| **Duplicated code** (two hand-rolled fixture kits) | `crates/core/src/walk/scanner.rs` (`tree()`, `collect()`, `paths()`, `scan()`), `crates/core/src/walk/matcher.rs` (`globs()`, `accepts()`) | Both build setup only their own `mod tests` can reach; a third suite will grow a third kit |
| **A table written out longhand** | `matcher.rs` glob matrix | Five near-identical `#[test]`s differing only in pattern and verdict — `rstest` `#[case]` material |
| **Properties pretending to be examples** | `scanner.rs` / `hash` tests | `a_digest_does_not_depend_on_the_chunk_boundaries` picks one size (`CHUNK_SIZE * 2 + 1337`); `every_algorithm_round_trips_through_its_name` loops a 3-element array |
| **Inline setup, no seam** | `packages/path/test/path.test.ts` | Parses the workspace `Cargo.toml` inside the suite; no fixture kit exists on the TS side at all |
| **Lifetime managed by convention** | `scanner.rs` tests | `TempDir` stays alive only because every test remembers `let dir = …` |

Baseline (verified in this workspace, 2026-10-03): 54 Rust unit tests + 2
doctests + the Bun suite, all green; coverage 95.51% region / 94.81% line.
The percentage is **not** a goal of this plan (task-28 non-goal).

## 3. Target state

- **Rust:** one shared fixture kit reachable from every suite; the glob matrix
  as named `#[case]`s; the nine invariants of task-28 AC #5–#13 as generated
  properties; regression seeds committed.
- **TypeScript:** a private `@repo/test-utils` package owning
  `createFixture(setup, teardown?, scope?)` and a `tempTree` fixture mirroring
  the Rust one; `path.test.ts` on fixtures; `fast-check` properties for the
  pure `Path` string surface under `test/property/`.
- **Conventions written down** (task-28 AC #29–#30): examples/`#[case]` for
  finite enumerable inputs, properties for universal contracts, fixtures for
  anything set up more than once; `proptest-regressions` committed.
- **Agent rule (task-29):** `AGENTS.md` directs any agent implementing changes
  to consult the vendored `.agents/skills/tdd` skill for the red → green loop
  and `.agents/skills/refactor` for behaviour-preserving restructuring, before
  and during the work, not after.

## 4. Constraint report — what this environment can reach

Verified 2026-10-03 from the restored sandbox (this updates the table in
task-28, which was verified 2026-10-02 from a fully airlocked machine):

| Endpoint | State | Consequence |
| --- | --- | --- |
| `registry.npmjs.org` | ✅ reachable (metadata + tarballs) | `fast-check` can be added and `bun.lock` refreshed **here** |
| `index.crates.io` (sparse index) | ✅ reachable | cargo can **resolve** new crates and write `Cargo.lock` |
| `static.crates.io` (crate downloads) | ❌ blocked (TLS reset by egress proxy) | cargo cannot **download** new crates; `cargo vendor`/`fetch` fail |
| `crates.io`, docs.rs, S3 bucket, mirror sites | ❌ blocked | no alternative official artefact source |
| `github.com` / `codeload.github.com` | ✅ reachable | git dependencies and source tarballs are fetchable |

So the TypeScript half of task-28 is fully executable in this environment,
and the Rust half splits: everything that needs **no new crate** is executable
now; `rstest`/`proptest` themselves need either a properly connected machine
(the task's prescribed route) or a workaround (§6).

One more hard limit: this session can push only its own working branch, so
**`sandbox-publish` (transport repack, task-28 DoD) cannot run from here**
regardless of connectivity. That step stays connected-side.

## 5. Step sequence

Rules carried from the refactor skill: one step at a time, each step is a
commit, `pixi run gates` green at every boundary, no step mixes refactoring
with new verification (a step either moves setup without changing what is
verified, or adds verification without moving setup).

### Phase A — no new dependencies (executable now, fully offline-safe)

- **A1. Rust fixture kit, extraction only.** Add a `#[cfg(test)]` support
  module reachable from every core suite. Move `tree`/`collect`/`paths`/`scan`
  and `globs`/`accepts` into it; building a tree becomes one expression taking
  a tree description (paths, contents, sizes, modes — AC #1, #2). The
  permission-case "skip under root" pattern moves into the kit (part of
  AC #20). No test's verified behaviour changes.
- **A2. TS fixture kit.** New private package `@repo/test-utils`
  (`private: true`, `main` → `src/`, nothing published imports it — AC #22):
  `createFixture` with `"test"`/`"file"` scopes, an accessor that **throws by
  scope name** before setup (AC #23), teardown skipped when setup did not
  finish (AC #24), and a `tempTree` fixture mirroring A1's tree description
  (AC #25). Needs only `bun:test` hooks — zero new external dependencies.
- **A3. Refactor `path.test.ts` onto the kit.** The inline `Cargo.toml`
  parsing becomes a fixture (AC #26). Same assertions, new seams.
- **A4. Agent rule + conventions (task-29, AC #29–#30 of task-28).**
  `AGENTS.md` gains the skills-consultation rule and the three-sentence
  testing convention; the README's "Development & Quality Gates" section gets
  the same three sentences.

### Phase B — new dev-dependencies (gated on the §6 decision)

- **B1. `fast-check` properties (executable now — npm is reachable).** Add
  `fast-check ^4.10.2` to `packages/path` devDependencies, refresh `bun.lock`,
  add `test/property/` with the five `Path` properties of AC #27, kept apart
  from the examples (AC #28). *Airlock note:* a restored machine replays
  `bun install` from the transport, so the repack in Phase C is what makes
  this crate-equivalent step real offline.
- **B2. `rstest` adoption.** `#[fixture]` for per-test values (AC #3), the
  glob matrix as named `#[case]`s whose failures read without opening the
  file (AC #4), edge-case tables of AC #16–#19 where enumerable.
- **B3. `proptest` invariants.** AC #5–#13 as properties; seeds committed
  (AC #14); case counts bounded and the wall-clock budget recorded (AC #15);
  generator-driven edge cases of AC #16–#21 that tables cannot enumerate.

### Phase C — verification and hand-off

- **C1.** `pixi run gates` + `pixi run coverage`; record the `pixi run test`
  wall-clock budget in task-28's notes (AC #15, DoD).
- **C2.** `cargo deny check bans licenses sources` with the new
  dev-dependencies; update `deny.toml` if a licence is new (DoD).
- **C3.** Prove dev-only: `cargo tree -p pathway-fs-core -e normal` contains
  neither `rstest` nor `proptest` (DoD).
- **C4.** *Connected-side, out of this session's scope:* repack and republish
  the sandbox transport (`sandbox-pack` → `sandbox-doctor` →
  `sandbox-publish`) so a clean restore builds offline with the new
  dependencies (DoD).

## 6. The Rust dependency decision — three routes

`static.crates.io` is blocked, so `rstest`/`proptest` cannot arrive the normal
way from this environment. The routes, in order of fidelity to task-28:

| Route | What happens | Pros | Cons |
| --- | --- | --- | --- |
| **R1 — prescribed connected-side step** | A machine that reaches `static.crates.io` adds the dev-deps, refreshes `Cargo.lock`, repacks the transport; this plan's B2–B3 execute after restore | Exactly what task-28 specifies; clean lockfile; clean `deny.toml` | B2–B3 and parts of C wait on an external step |
| **R2 — git-source workaround** | Declare the dev-deps (and their ~10–15 missing transitive deps via `[patch.crates-io]`) as GitHub git dependencies pinned to release tags, vendor from git | Executable entirely from here | Lockfile diverges from the prescribed crates.io form; `deny.toml` sources churn; high patch-maintenance risk; transport repack still needed; must be unwound later |
| **R3 — interim hand-rolled properties** | Implement AC #5–#13's invariants now as bounded randomised tests over the vendored `fastrand 2.5.0`, inside the A1 kit, structured so each property body swaps mechanically into `proptest` when R1 lands | Every invariant is exercised today with zero dependency changes; A-phase work is identical either way | No shrinking, no persisted regression seeds (AC #14 deferred); a second, smaller migration later |

Recommendation: **A1–A4 and B1 unconditionally; R1 for B2–B3**, with **R3 as
the interim** if the invariants should not wait for the connected-side step.
R2 is not recommended: it trades a one-time external step for lasting
lockfile and licence-gate complexity.

## 7. Risks

- **A property that finds a real bug stops being a refactor.** Then the loop
  switches to the tdd skill: a failing test is red, the fix is its own
  commit, and the refactor sequence resumes after green.
- **Wall-clock creep** (AC #15): property case counts are stated explicitly
  wherever they differ from the default, and C1 records the budget.
- **Fixture kit becoming a framework.** The kit owns setup/teardown and tree
  construction, nothing else — assertions stay in the suites (tdd skill:
  tests verify behaviour at seams, helpers must not hide the seam).
- **Non-UTF-8 and permission edge cases differ under root / per-FS** (AC #17,
  #20): the skip-don't-fail pattern lives in the kit so every suite applies
  it identically.

## 8. Non-goals

Inherited from task-28: raising the coverage number, rewriting tests already
in the right shape (`error.rs`, `walk/entry.rs`), and benchmarks (task-4).
Added here: R2 is a documented option, not a goal.
