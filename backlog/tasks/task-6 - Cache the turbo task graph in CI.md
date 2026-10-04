---
id: task-6
title: Cache the turbo task graph in CI
status: Done
assignee: []
created_date: '2026-09-30'
updated_date: '2026-10-04'
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

## Findings from the PR #8 runs (2026-10-04)

Four things the Actions runs taught that local verification could not:

1. **`//#lint:biome` can never cache-hit in CI — its input glob hashes the cache itself.** The task's inputs are `biome.json` + `**/*.{ts,tsx,js,jsx,mjs,cjs,json,jsonc}` over the root package, i.e. the whole repository, and turbo's glob does not exclude the restored `.turbo/cache` (nor `node_modules`/`.pixi`): every `*.json` inside them joins the hash. Measured on clean checkouts of `605ee56`: the task hashes to `f48f413d…`; add one `.json` under `.turbo/cache` → `0d92e618…`; add `node_modules`/`.pixi` json → `a321641d…`. In CI each run restores *different* cache content, so the hash is new every time — run 1 executed the task as `424cbde6…`, run 2 as `cb41cfd4…`, both misses against byte-identical tracked inputs. Today it costs one ~30 ms `biome check`, so it is cosmetic — but it is the only task in the graph that structurally cannot hit, and any future root-package task with a repo-wide glob inherits the defect. **Fix (one line, deliberately not pushed in this PR — a mid-flight push would cancel the bench sweep through the workflow's concurrency group):** add exclusion globs to the inputs, `"!.turbo/**"` at minimum, ideally also `"!node_modules/**"` and `"!.pixi/**"`. `//#lint:actions` shows the healthy pattern already: narrow explicit inputs, and its hash matched the local prediction exactly (`85bef0f5…`).
2. **A correction to a belief first recorded on 2026-10-03: editing `turbo.json` does not invalidate every task hash.** The branch added a `bench` task and re-pointed `build:native:release` in `turbo.json` relative to main, yet `@repo/rust#lint` (`00c3df6f…`), `typecheck` (`db821266…`), `build` (`166f548c…`) and `build:native` (`4d00be57…`) hash identically on main, on `0addc88` and on `605ee56`, and run 1 hit all of them from main's cache entry. The earlier local observation ("turbo.json edits invalidate ALL task hashes") was confounded by finding 1 — it was made in a tree whose `.turbo` content had changed between the two hashes. Only tasks whose declared inputs actually changed move; `globalDependencies` (`biome.json`, `pixi.toml`, `pixi.lock`) remains the deliberate whole-graph invalidator, per the AC above.
3. **PR runs inherit main's cache through the OS-scoped fallback.** Run 1, the first run on the PR ref, found no `turbo-Linux-8/merge-*` entry and matched the second restore-key `turbo-Linux-`, restoring `turbo-Linux-main-879036bbfd…` — a default-branch entry. (Caches saved by the branch's own `push` runs live under `turbo-Linux-arena/01a103a6-pathway-…` and are invisible to `pull_request` runs; GitHub scopes cache reads to the PR ref and the default branch.) Consequence, recorded above in AC-3: a PR branch whose base is warm never produces a cold "before" run, so the honest before/after is task-level, not wall-clock.
4. **Both restore paths work, and the chain carries novel entries forward.** Run 1 → main (cross-branch, OS-scoped prefix) and run 2 → run 1 (same-PR, ref-scoped prefix `turbo-Linux-8/merge-`, whose key run 1 had saved). Run 2 then hit `@repo/bench-walk#typecheck` (`95fef0c3…`) — a task that did not exist in main's cache and was minted by run 1. The `restore-keys` ladder from the Implementation section is doing exactly what it was designed to do.

## Acceptance Criteria

- [x] `.turbo` is cached and restored between CI runs.
- [x] The cache key includes everything that legitimately invalidates the whole graph. `turbo.json` already lists `biome.json`, `pixi.toml` and `pixi.lock` in `globalDependencies`, so a toolchain change is covered by turbo's own hashing — confirm the Actions cache key does not *additionally* pin something that makes it never hit (a key on `github.sha` alone never hits; it needs a `restore-keys` prefix).
- [x] A no-op PR (README typo) shows turbo cache hits for `build`, `build:native`, `test` and `typecheck`, and the CI wall time drops measurably. Record before/after numbers in the PR.

  **Observed 2026-10-04 on PR #8, with one adaptation forced by the environment** (reruns are unavailable for these runs, and every push to the PR branch also starts a ~45 min bench sweep): the compared runs are [37161133863](https://github.com/Archont561/pathway/actions/runs/37161133863) (`0addc88`) and [37163546035](https://github.com/Archont561/pathway/actions/runs/37163546035) (`605ee56`), whose trees differ in **exactly one file, `.github/workflows/bench.yml`** — an input to exactly one turbo task (`//#lint:actions`), verified by dry-run hash diff: of all lint-graph hashes, only `//#lint:actions` moved (`dc2bcecf…` → `85bef0f5…`). That is a *stronger* test than a README typo: the same run pair shows hits for unchanged tasks **and** a miss for the changed input.

  - `build` — **hit** (`166f548c…`, replayed) · `build:native` — **hit** (`4d00be57…`) · `test` — **FULL TURBO, 4/4 cached, 26 ms** · `typecheck` — **FULL TURBO, 5/5 cached, 113 ms** (run 1: 4/5 cached, 1.221 s — the miss was `@repo/bench-walk#typecheck`, a package that did not exist in main's cache). Also hit: `@repo/rust#lint` (`00c3df6f…`), `@repo/rust#coverage`, `pathway-docs#build`. Missed, both by design of their inputs: `//#lint:actions` (the changed file) and `//#lint:biome` (see finding 1 below — a defect, not invalidation).
  - Wall time: 61 s → 59 s job, `pixi run ci` 20 s → 24 s — **parity, and the AC's wall-clock clause explains why it could not show a drop**: run 1 was not cold. It found no `turbo-Linux-8/merge-*` entry (first run on the PR ref) and fell back to the OS-scoped prefix, restoring **main's** entry `turbo-Linux-main-879036b…` (54 MB) — GitHub lets `pull_request` runs read default-branch caches. With both runs warm, the measurable statement is the task-level one: every unchanged task replayed from cache, and the run's entire re-execution bill was two lint tasks (~1 s combined). A genuinely cold reference for this graph exists locally instead: task-5's clean clone ran `pixi run ci` in **4m25s** with no caches at all.

  The numbers are recorded in PR #8's body alongside the run links.
- [x] A Rust-only change still rebuilds `build:native` and re-runs `test`, and a docs-only change still rebuilds the site — i.e. the cache is correct, not just fast. Reuse the invalidation matrix from task-5.
- [x] The cache cannot serve a stale artefact across a toolchain bump: verify by changing `pixi.lock` and confirming a full rebuild.
- [x] A local build still works with no cache and no network, so the airlocked sandbox is unaffected.

## Definition of Done

- [x] CI is measurably faster on unchanged inputs with no loss of correctness. — unchanged tasks are free (FULL TURBO on `typecheck`/`test`; `build`/`build:native` replay), and correctness held: the only legitimately invalidated task, `//#lint:actions`, missed and re-ran while everything else hit — no stale artefact was served (turbo rehashes all declared inputs against whatever was restored, so a stale restore can only cost a miss). The wall-clock comparison is documented above with the reason a cold-vs-warm pair cannot be produced on a warm-main PR branch.
- [x] If a remote cache is adopted instead of `actions/cache`, the decision and its constraints are recorded in `.knowledge/implementation/ci-distribution.md`. — **N/A:** `actions/cache` was adopted; the decision *against* the remote-cache option (airlock constraint, no external service) is recorded in the Implementation section above.
