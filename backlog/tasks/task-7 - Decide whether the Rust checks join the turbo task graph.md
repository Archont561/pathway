---
id: task-7
title: Decide whether the Rust checks join the turbo task graph
status: Done
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30 20:25'
labels:
  - build
  - tooling
milestone: m-0
dependencies:
  - TASK-5
priority: medium
---

## Description

Every cargo task (`fmt-check-rs`, `clippy`, `deny`, `test-rs`, `test-doc-rs`, `build-rs`, `coverage`) is invoked directly by pixi and is invisible to turbo. This is deliberate and matches the ownership rule — Cargo owns the Rust dependency graph — but it has a cost: `pixi run gates` and CI re-run `clippy --workspace --all-targets` on **every** commit, including README-only and docs-only ones. `Swatinem/rust-cache` makes the recompile cheap; it does not make the task not run.

This task is to decide, with evidence, whether that is worth fixing — and it is explicitly allowed to conclude "no".

**Do not fix it by giving every crate a `package.json`.** A per-crate façade duplicates the Cargo dependency graph in a second place, which is the failure mode the multi-language monorepo guidance warns about, and in a single Cargo workspace (one lockfile, one `target/`) it also invites false cache hits when a dependency crate changes.

The option worth evaluating is a **root-level turbo task**, which adds no files and duplicates no graph:

```jsonc
// turbo.json
"//#lint:rust": { "inputs": ["Cargo.toml", "Cargo.lock", "crates/**", "deny.toml"], "outputs": [] },
"//#test:rust": { "inputs": ["Cargo.toml", "Cargo.lock", "crates/**"], "outputs": [] }
```

```jsonc
// root package.json
"lint:rust": "cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check bans licenses sources",
"test:rust": "cargo nextest run --workspace"
```

**The tension to resolve, and the reason this was deferred rather than done:** it gives cargo commands two spellings — the granular `pixi run clippy` and the bundled, turbo-cached `lint:rust` — with the clippy flags written in two places. That is a mild version of the "one verb, two paths" problem the orchestration refactor just removed, so adopting it needs a deliberate answer for how the two stay in sync, not just a cache win.

Possible resolutions to weigh:
- Accept the duplication, with the granular pixi tasks documented as focused escape hatches and the bundle as the gate.
- Have the granular pixi tasks be the only definition and let the turbo root task shell into them (`pixi run clippy`), accepting a deeper call chain (pixi → bun → turbo → pixi → cargo).
- Decline, and instead reduce the cost another way — e.g. a `paths-filter` on the Rust CI steps, which solves the CI half without touching the local task graph at all.

## Resolution: ADOPTED (2026-09-30), by a third option this task did not list

The Rust checks are now in the turbo graph, and the cost this task was written about is gone. Neither option in the description was taken.

**What was done.** The Cargo workspace became **one** turbo package — `@repo/rust`, a single `crates/package.json` holding every cargo command as a script — rather than a root-level turbo task or a per-crate façade.

That threads the needle this task could not find:

- **It does not give every crate a `package.json`.** The explicit prohibition holds. There is one package for the whole workspace and it declares **no dependencies**, so the Cargo dependency graph is not restated anywhere. Cargo still resolves `core → path → engine` by itself; turbo sees one opaque node.
- **It does not create two spellings.** This was the reason the task was deferred, and the resolution is to *delete* the granular pixi tasks rather than keep them beside a bundle. `clippy`, `fmt-check-rs`, `deny`, `test-rs`, `test-doc-rs`, `build-rs`, `coverage` and `coverage-report` no longer exist as pixi tasks. Each flag is written exactly once, in `crates/package.json`. The focused escape hatch is a filter on the same graph — `turbo run lint --filter=@repo/rust` — not a second definition.
- **`lint-advisories` stayed out of the cached bundle**, as required. It is a pixi task and a separate CI step, because caching a network-dependent advisory check would defeat it.

**Why one package and not three.** Measured, because the guess could have gone either way: Cargo's unit of work is the workspace, and it holds a single lock on `target/`. Three parallel per-crate invocations therefore serialise on that lock *and* pay the process overhead three times.

| `cargo clippy`, warm, sources touched | Wall clock |
| --- | --- |
| one `--workspace` invocation | 195 ms, 201 ms |
| three parallel `-p <crate>` invocations | 320 ms, 365 ms |

The parallel runs logged `Blocking waiting for file lock on build directory` twice and `... on package cache` ten times. Splitting would cost ~1.8x and buy nothing, since cargo's own incremental compilation is already finer-grained than turbo's.

**The measurement this task asked for first.** It is much larger than the task assumed, because the old arrangement re-ran every cargo check unconditionally:

| | Before | After |
| --- | --- | --- |
| Warm `pixi run gates` | 5,781 ms | **794 ms** |
| `pixi run gates` after a README-only edit | 5,781 ms (nothing was cached) | **757 ms** — `@repo/rust#lint` is a cache hit |
| After an edit under `crates/` | re-runs | re-runs (`cache miss`) — correctness preserved |

So the answer to "is it worth fixing" is unambiguously yes: a **7x** reduction on the warm path, and a docs-only commit no longer pays for clippy at all.

**Where turbo may not cache.** `crates/turbo.json` marks `build:release` `cache: false`. Its output is `$TURBO_ROOT$/target`, outside the package, so turbo cannot capture or restore it; a cache hit would skip a build whose artefacts may have been evicted. `lint`, `test` and `coverage` are cached, because their only output is "it passed" — plus `lcov.info`, which cargo-llvm-cov writes inside the package.

The decision and both measurements are recorded in `.knowledge/implementation/repo-structure.md` (two new decision-table rows) and `.knowledge/log.md`.

## Acceptance Criteria

- [x] Measure first: how long do the Rust checks actually add to a warm `pixi run gates` and to a CI run on a docs-only change? If the answer is small, record it and close this as "declined" — that is a valid outcome.
- [x] If adopted: a docs-only commit skips the Rust checks entirely (cache hit), while any change under `crates/`, `Cargo.toml`, `Cargo.lock` or `deny.toml` still runs them.
- [x] If adopted: no crate gains a `package.json`, and the Cargo dependency graph is not restated anywhere.
- [x] If adopted: the duplicate-spelling question has an explicit answer, written down — which invocation is authoritative and how a flag change stays in sync.
- [x] If adopted: `deny-advisories` stays **out** of the cached bundle. It needs the network and is CI-only by design; caching a network-dependent advisory check would defeat its purpose.
- [x] Either way, the decision and its reasoning land in `.knowledge/implementation/repo-structure.md` so the next person does not re-litigate it.

## Definition of Done

- [x] A decision is recorded — adopted or declined — with the measurement that justified it.
- [x] If adopted, `pixi run gates` and `pixi run ci` are still green and the `lint` aggregator reflects the new arrangement.
