---
id: task-6
title: Cache the turbo task graph in CI
status: To Do
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30'
labels:
  - ci
  - build
dependencies:
  - TASK-5
priority: medium
---

## Description

`.github/workflows/ci.yml` caches the pixi environment (`setup-pixi` with `cache: true`) and the cargo target directory (`Swatinem/rust-cache`), but **nothing caches turbo's task cache**. `.turbo/` is gitignored and never restored, so every CI run starts cold and re-executes every turbo task from scratch even when the inputs are identical to the previous run.

That was tolerable when turbo only orchestrated `tsc`. It is not any more: after the orchestration refactor, the turbo graph owns the debug addon build (`build:native`, ~40s cold), the TypeScript build, the Astro production build **and** `astro check`. A docs-only PR currently pays for a cargo build of the engine; a Rust-only PR pays for a full Astro build. `Swatinem/rust-cache` softens the first case by caching `target/`, but turbo still re-runs the task — cargo just finishes faster.

Two options, in increasing order of effort:

1. **`actions/cache` on `.turbo`** — one step, no external service. Key on the run's commit with a restore-prefix fallback, the usual pattern. Good enough to make unchanged tasks free.
2. **A remote cache** — Vercel's, or a self-hosted implementation of the Turborepo cache API. Shares the cache between CI and developer machines, which is the bigger win but adds a service and a token to a repo that is otherwise entirely self-contained (note the airlocked-sandbox constraint in `.pixi-sandbox.toml`: a required remote cache must not become a hard dependency of a local build).

Start with option 1 and only escalate with evidence.

## Acceptance Criteria

- [ ] `.turbo` is cached and restored between CI runs.
- [ ] The cache key includes everything that legitimately invalidates the whole graph. `turbo.json` already lists `biome.json`, `pixi.toml` and `pixi.lock` in `globalDependencies`, so a toolchain change is covered by turbo's own hashing — confirm the Actions cache key does not *additionally* pin something that makes it never hit (a key on `github.sha` alone never hits; it needs a `restore-keys` prefix).
- [ ] A no-op PR (README typo) shows turbo cache hits for `build`, `build:native`, `test` and `typecheck`, and the CI wall time drops measurably. Record before/after numbers in the PR.
- [ ] A Rust-only change still rebuilds `build:native` and re-runs `test`, and a docs-only change still rebuilds the site — i.e. the cache is correct, not just fast. Reuse the invalidation matrix from task-5.
- [ ] The cache cannot serve a stale artefact across a toolchain bump: verify by changing `pixi.lock` and confirming a full rebuild.
- [ ] A local build still works with no cache and no network, so the airlocked sandbox is unaffected.

## Definition of Done

- [ ] CI is measurably faster on unchanged inputs with no loss of correctness.
- [ ] If a remote cache is adopted instead of `actions/cache`, the decision and its constraints are recorded in `.knowledge/implementation/ci-distribution.md`.
