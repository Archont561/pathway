---
id: task-6
title: Cache the turbo task graph in CI
status: In Progress
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30 20:24'
labels:
  - ci
  - build
milestone: m-0
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

## Implementation (2026-09-30)

Option 1 taken: an `actions/cache@v6.1.0` step, no external service, no token. Added to **both** `ci.yml` and `docs.yml`, because after the orchestration refactor `docs.yml`'s two steps are turbo tasks as well (`docs-build` → `turbo run build --filter=pathway-docs`, `docs-check` → `turbo run typecheck --filter=pathway-docs`), so that workflow paid the same cold-cache cost.

Three things worth recording, because each is a way this step is commonly written wrong:

- **The path is `.turbo/cache`, not `node_modules/.cache/turbo`.** Verified empirically on turbo 2.11.5: after a run, `.turbo/cache` holds the `<hash>.tar.zst` / `-manifest.json` / `-meta.json` triples and `node_modules/.cache/turbo` does not exist. The `node_modules` path is turbo 1.x's; an `actions/cache` step pointed there restores nothing and reports no error, so it looks like it works forever.
- **The `restore-keys` are what hit, not the key.** The key is per-commit so each run writes a fresh entry; a key on `github.sha` alone can never hit, because the entry is only written *after* the run that would have used it. Two fallback prefixes: branch-scoped first, then OS-scoped.
- **The key is not workflow-scoped, on purpose.** `ci.yml` and `docs.yml` share the prefix, so a docs-only pull request can restore the Astro build the ci workflow already produced for the same inputs.

Correctness does not rest on the cache key at all: turbo rehashes every task's declared inputs against whatever was restored, so a stale restore costs a miss and never a wrong artefact. This was checked rather than assumed — see the criteria below.

The remote-cache option (option 2) was **not** taken and no evidence yet argues for it. It would add a service and a token to a repository that is otherwise self-contained, and `.pixi-sandbox.toml`'s airlock constraint says a remote cache must never become a hard dependency of a local build.

### What was verified locally, and what still needs a CI run

Verified on the restored full toolchain, entirely offline:

| Check | Result |
| --- | --- |
| turbo's cache location | `.turbo/cache` (turbo 2.11.5); `node_modules/.cache/turbo` absent |
| `actionlint` on both edited workflows | clean |
| A `pixi.lock` edit invalidates the whole graph | **pass** — all nine task nodes invalidated, so the cache cannot serve a stale artefact across a toolchain bump. `globalDependencies` is `["biome.json", "pixi.toml", "pixi.lock"]` |
| A Rust-only change still rebuilds `build:native` and re-runs `test` | **pass** — reused the task-5 invalidation matrix |
| A docs-only change still rebuilds the site | **pass** — same matrix |
| A local build works with no cache and no network | **pass** — every run in this session was offline with `CARGO_NET_OFFLINE=true`, and `.turbo` was deleted repeatedly between runs |

Still open, because it can only be observed on GitHub Actions: the before/after wall-clock numbers for a no-op pull request. `ci.yml` triggers on `push: [main]`, `pull_request` and `workflow_dispatch`, so a push to a topic branch does not exercise it — this needs a pull request or a manual dispatch.

## Acceptance Criteria

- [x] `.turbo` is cached and restored between CI runs.
- [x] The cache key includes everything that legitimately invalidates the whole graph. `turbo.json` already lists `biome.json`, `pixi.toml` and `pixi.lock` in `globalDependencies`, so a toolchain change is covered by turbo's own hashing — confirm the Actions cache key does not *additionally* pin something that makes it never hit (a key on `github.sha` alone never hits; it needs a `restore-keys` prefix).
- [ ] A no-op PR (README typo) shows turbo cache hits for `build`, `build:native`, `test` and `typecheck`, and the CI wall time drops measurably. Record before/after numbers in the PR.
- [x] A Rust-only change still rebuilds `build:native` and re-runs `test`, and a docs-only change still rebuilds the site — i.e. the cache is correct, not just fast. Reuse the invalidation matrix from task-5.
- [x] The cache cannot serve a stale artefact across a toolchain bump: verify by changing `pixi.lock` and confirming a full rebuild.
- [x] A local build still works with no cache and no network, so the airlocked sandbox is unaffected.

## Definition of Done

- [ ] CI is measurably faster on unchanged inputs with no loss of correctness.
- [ ] If a remote cache is adopted instead of `actions/cache`, the decision and its constraints are recorded in `.knowledge/implementation/ci-distribution.md`.
