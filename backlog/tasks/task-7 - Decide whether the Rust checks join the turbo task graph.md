---
id: task-7
title: Decide whether the Rust checks join the turbo task graph
status: To Do
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30'
labels:
  - build
  - tooling
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

## Acceptance Criteria

- [ ] Measure first: how long do the Rust checks actually add to a warm `pixi run gates` and to a CI run on a docs-only change? If the answer is small, record it and close this as "declined" — that is a valid outcome.
- [ ] If adopted: a docs-only commit skips the Rust checks entirely (cache hit), while any change under `crates/`, `Cargo.toml`, `Cargo.lock` or `deny.toml` still runs them.
- [ ] If adopted: no crate gains a `package.json`, and the Cargo dependency graph is not restated anywhere.
- [ ] If adopted: the duplicate-spelling question has an explicit answer, written down — which invocation is authoritative and how a flag change stays in sync.
- [ ] If adopted: `deny-advisories` stays **out** of the cached bundle. It needs the network and is CI-only by design; caching a network-dependent advisory check would defeat its purpose.
- [ ] Either way, the decision and its reasoning land in `.knowledge/implementation/repo-structure.md` so the next person does not re-litigate it.

## Definition of Done

- [ ] A decision is recorded — adopted or declined — with the measurement that justified it.
- [ ] If adopted, `pixi run gates` and `pixi run ci` are still green and the `lint` aggregator reflects the new arrangement.
