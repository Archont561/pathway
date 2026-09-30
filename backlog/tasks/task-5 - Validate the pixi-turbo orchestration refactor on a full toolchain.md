---
id: task-5
title: Validate the pixi/turbo orchestration refactor on a full toolchain
status: In Progress
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30 13:05'
labels:
  - build
  - tooling
dependencies: []
priority: high
---

## Description

The orchestration refactor (commit `refactor(build): give pixi and turbo one path per verb`; see the 2026-09-30 entry in `.knowledge/log.md`) was written and statically verified, but **never executed**: the environment it was authored in had no `pixi`, `bun` or `cargo`, so no task, build or cache behaviour was observed. Everything below is therefore unproven, not broken-until-shown — but it touches the task graph, the cache keys and the CI step list, which is exactly the surface where a silent regression looks like a passing build.

What changed, and so what needs proving:

- Every package-scoped pixi task lost its `cwd` and now goes through `turbo run <task> --filter=<pkg>` via a root `package.json` script.
- `test` gained a `build:native` dependency (the fix for the hole task-3 would have hit).
- `PATHWAY_VERSION` was deleted from `turbo.json` `globalEnv`, the pixi tasks and `apps/docs/src/version.ts`; `apps/docs/turbo.json` (new Package Configuration) now declares the root `Cargo.toml` and `scripts/version.ts` as build inputs in its place.
- `apps/docs` renamed `check` → `typecheck`, so `astro check` is now inside `pixi run gates`.
- The `benches/*` workspace glob and both `bench` tasks were removed.
- `pixi.lock` was added to `globalDependencies`.

Run this on a machine with the real toolchain and record the results. Where a criterion fails, the fix belongs in this task, not a new one.

## Partial verification already done (2026-09-30)

A session with npm egress only (no conda, no crates.io, no GitHub release assets) obtained `bun` 1.3.11 from the `@oven/bun-linux-x64` npm tarball and exercised the JavaScript half of the graph directly, without pixi. Results, so this task starts from evidence rather than zero:

| Check | Result |
| --- | --- |
| `bun install --frozen-lockfile` after the `workspaces` edit | **pass** — `bun.lock` byte-identical, no regeneration needed |
| `test` depends on `build:native` (`turbo run test --dry=json`) | **pass** — `@archont561/pathway#test` → `@archont561/pathway#build:native` → `napi build …`. Pre-refactor it depended only on `@repo/typescript-config#build`, which is `<NONEXISTENT>` |
| `biome check .` over the hand-written JSON/TOML edits | **pass** — 27 files, no fixes needed |
| `turbo run typecheck` | **pass** — 2 packages incl. `pathway-docs` (`astro check`, 0 errors). 9.4 s cold for the whole task |
| Docs build with no `PATHWAY_VERSION` anywhere | **pass** — site emits `pathway-version = 0.1.0`, read from `Cargo.toml` |
| Version bump invalidates the docs build | **pass** — cache hit → bump → cache miss → site emits `0.1.1` |
| `crates/**` edit leaves the docs cache intact | **pass** *with* `apps/docs/turbo.json`; **fails without it** (hash changes) — see the correction note below |
| `bun test` in `packages/path` | **pass** — 7/7 |
| `turbo run build` end to end | **blocked** — fails at `build:native` because `napi build` needs cargo, which was unobtainable (crates.io and static.rust-lang.org unreachable) |

**Correction folded into the docs already:** the refactor originally claimed that without `apps/docs/turbo.json` a version bump would serve a stale cached site. Measurement disproved that — the bump was caught anyway, because the root `build` task depends on `build:native` and a package with no such script still gets a phantom one whose inputs are the whole Rust workspace. The Package Configuration's real value is narrowing that to the declared edge. `.knowledge/log.md`, `repo-structure.md`, both READMEs, `pixi.toml` and `apps/docs/src/version.ts` were corrected.

## Verification on the full toolchain (2026-09-30)

The rest was then executed for real. The toolchain came from this repository's own offline transport branch `sandbox/developer-linux-64` — no network path to rustup, crates.io or conda exists from that sandbox, but the airlock is self-sufficient, which is the first end-to-end proof that the sandbox design works as intended. `pixi.lock` at HEAD hashes to exactly the `lock_sha256` the transport manifest records, so the packed environment is valid for this commit; `pixi install --frozen --offline` is a 68 ms no-op. Versions: rust 1.98.1, cargo 1.98.1, pixi 0.81.0, bun 1.3.11, lefthook 2.1.15.

| Criterion | Result |
| --- | --- |
| `pixi run bun-install` leaves `bun.lock` byte-identical | **pass** — sha256 unchanged |
| `pixi run gates` | **pass**, exit 0 — all ten steps |
| `pixi run ci` | **pass**, exit 0 — twelve steps |
| Astro production build runs **exactly once** under `pixi run ci` | **pass** — on a cold turbo cache, exactly one `page(s) built`; `astro check` likewise once |
| No `.node`, cold cache → `pixi run test` builds the addon *first* | **pass** — `build:native` cache-missed and executed, *then* `test`; the 15 MB addon was produced and `node` loads it (`napiVersion`, `engineVersion`) |
| `pixi run build-native` ↔ `turbo run build:native` share a cache entry | **pass** — both hash `5a5d7aaa5d1aa494`; the second is a hit. The pixi task no longer bypasses the cache |
| `pixi run docs-build` ↔ `pixi run build` same task + entry | **pass** — both `pathway-docs#build` at `75407e8d585513ab`, miss then hit |
| Docs quote the right version with `PATHWAY_VERSION` unset | **pass** — `0.1.0` from `Cargo.toml`, in the production build *and* under `astro dev` |
| `pixi run lint-js` under the new `lint:js` script name | **pass** |
| `dev` / `preview` are persistent | **pass** — `dev` runs `cache bypass, force executing` and serves `/pathway/`; `preview` is uncached with `dependsOn: ["pathway-docs#build"]`, so it builds first |

### Cache invalidation, one file changed at a time

Hashes captured with `turbo run build test --dry=json` before and after each edit, then reverted.

| Mutation | `build:native` | `build` | `test` | `pathway-docs#build` |
| --- | --- | --- | --- | --- |
| `crates/core/src/lib.rs` | invalidated | invalidated | invalidated | **unchanged** |
| `packages/path/src/path.ts` | **unchanged** | invalidated | invalidated | unchanged |
| `apps/docs/src/content/docs/index.mdx` | unchanged | unchanged | unchanged | **invalidated** |
| `[workspace.package] version` bump | — | — | — | **invalidated** (`75407e8d…` → `e653dc51…`); rebuilt site emits `0.1.1` |

Every row is as designed. The last two rows are what `apps/docs/turbo.json` buys, and they now rest on measurement rather than on the incorrect claim this task previously carried.

### lefthook glob selection

Probed with `lefthook run pre-commit --file <path>` (without `--force`, which would defeat glob filtering).

| Staged file | Jobs woken |
| --- | --- |
| `crates/core/src/lib.rs` | `rust-fmt`, `rust-clippy` |
| `packages/path/src/path.ts` | `js-biome`, `js-typecheck` |
| `apps/docs/src/version.ts` | `js-biome`, `js-typecheck` — the `apps/**` glob working; under the old `benches/**` glob this file woke nothing |
| `.github/workflows/ci.yml` | `workflows` |

### Cost of `astro check` in `gates`, and the decision

| Measurement | Cold |
| --- | --- |
| `typecheck` filtered to the TS package (`tsc`) | 1.16 s |
| `typecheck` filtered to the docs (`astro check`) | 6.65 s |
| `typecheck`, both packages in parallel | 7.50 s |
| `pixi run gates`, cold turbo cache | 15.9 s |
| `pixi run gates`, warm | 5.8 s |

**Decision: keep `astro check` in `gates`.** Its marginal cost is ~6.3 s, and cold `gates` at 15.9 s / warm at 5.8 s is not slow enough to discourage anyone. No `gates-fast` split, and `apps/docs` stays in `typecheck`. Caveat on the numbers: `target/` was warm, so a genuinely fresh clone additionally pays cargo's first compile (~40 s) — that is cargo's cost, not Astro's, and `astro check`'s marginal share stays ~6 s either way.

### Findings

1. **`test-doc-rs` is a vacuous gate.** `cargo test --doc -p pathway-fs-core -p pathway-fs` reports `0 passed; 0 failed` for *both* crates — there is not one doctest in the workspace. `pixi.toml` justifies the task on the grounds that a public Rust API "is only as documented as its examples compile", but there are no examples to compile, so the gate cannot fail. Filed as task-8.
2. `pixi run hooks-install` renames any pre-existing `commit-msg` hook to `commit-msg.old`. That is lefthook's documented behaviour rather than a repo bug, but it silently displaces a hook on a machine that already had one.
3. The phantom `build:native` and `test` nodes on `pathway-docs` and `@repo/typescript-config` still churn their hashes on a `crates/` edit. They carry no script and no outputs, so nothing executes and nothing is served stale — this is exactly the mechanism the corrected documentation describes, now confirmed by hash.

## Acceptance Criteria

- [x] `pixi install` succeeds and `pixi run bun-install` (`bun install --frozen-lockfile`) still accepts the existing `bun.lock` — removing the `benches/*` glob changed no resolved member, so the lockfile should not need regenerating. If it does, regenerate it in this task and note why.
- [x] `pixi run gates` passes end to end.
- [x] `pixi run ci` passes, and the Astro production build runs **exactly once** (it previously ran twice — once through turbo, once through `docs-build` going around it).
- [x] On a clean checkout with no `.node` present, `pixi run test` builds the addon *before* `bun test` runs. This is the regression the refactor exists to prevent; verify it by deleting `packages/path/*.node` first.
- [x] `pixi run build-native` and `turbo run build:native` produce a **cache hit** for each other — i.e. the pixi task no longer bypasses the cache.
- [x] `pixi run docs-build` and `pixi run build` resolve to the same turbo task and the same cache entry for `pathway-docs`.
- [x] Cache invalidation behaves as intended, verified by touching one file at a time and inspecting `turbo run build --dry=json` or the run summary:
  - a `.rs` change under `crates/` invalidates `build:native`, `build` and `test`
  - a change to `packages/path/src/**` invalidates `build`/`test` but **not** `build:native`
  - an `.mdx` change under `apps/docs/src/content/` invalidates only the docs tasks
  - a `[workspace.package] version` bump in the root `Cargo.toml` invalidates the docs build and the rebuilt site quotes the new number
  - a `crates/**` edit leaves the docs cache **intact** (this is what `apps/docs/turbo.json` buys; without it the site inherits a phantom `build:native` dependency on the whole Rust workspace)
- [x] The docs site renders the correct version with no `PATHWAY_VERSION` set anywhere.
- [x] Record the wall-clock cost `astro check` adds to a cold `pixi run gates`. If it is large enough to discourage running gates locally, either drop `apps/docs` back out of the `typecheck` task or split a `gates-fast`; note the decision here.
- [x] `pixi run lint-js` works under its new root script name (`lint:js`), and the lefthook `js-biome` / `js-typecheck` hooks still fire on the right globs (`apps/**` replaced `benches/**` in the typecheck glob).
- [x] `turbo run dev --filter=pathway-docs` and `turbo run preview --filter=pathway-docs` behave as persistent tasks (`pixi run docs-dev` / `docs-preview`); `preview` correctly builds first.

## Definition of Done

- [ ] `pixi run ci` is green on a clean clone and in GitHub Actions. — **half done.** Green on a clean restored toolchain (exit 0, twelve steps). Not yet observed on GitHub Actions: `ci.yml` triggers on `push: [main]`, `pull_request` and `workflow_dispatch`, so a push to a topic branch does not run it. Closing this needs a pull request or a manual dispatch. Every CI step except the two that need network (`deny-advisories`) or a release binary (`lint-sandbox-plan`, which did run, via the lefthook probe) has been executed locally, including `coverage`.
- [x] Any deviation found is fixed here, and `.knowledge/log.md` + `.knowledge/implementation/repo-structure.md` are corrected if the refactor's description turned out to be wrong in any detail. — one deviation found and fixed: `.github/workflows/docs.yml` still documented `PATHWAY_VERSION` as live behaviour and still credited `docs-build` as the `ci` aggregator's docs step. Both corrected. `repo-structure.md` and `ci-distribution.md` were re-checked and make no claim that measurement contradicts; `.knowledge/log.md` gained a verification entry. The separate `test-doc-rs` finding is not a refactor deviation and is filed as task-8.
