---
id: task-5
title: Validate the pixi/turbo orchestration refactor on a full toolchain
status: To Do
assignee: []
created_date: '2026-09-30'
updated_date: '2026-09-30'
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

What remains for this task is everything that needs **pixi and cargo**: the pixi task layer itself, `build:native`, the Rust gates, `pixi run gates` / `pixi run ci` end to end, and the lefthook hooks.

## Acceptance Criteria

- [ ] `pixi install` succeeds and `pixi run bun-install` (`bun install --frozen-lockfile`) still accepts the existing `bun.lock` — removing the `benches/*` glob changed no resolved member, so the lockfile should not need regenerating. If it does, regenerate it in this task and note why.
- [ ] `pixi run gates` passes end to end.
- [ ] `pixi run ci` passes, and the Astro production build runs **exactly once** (it previously ran twice — once through turbo, once through `docs-build` going around it).
- [ ] On a clean checkout with no `.node` present, `pixi run test` builds the addon *before* `bun test` runs. This is the regression the refactor exists to prevent; verify it by deleting `packages/path/*.node` first.
- [ ] `pixi run build-native` and `turbo run build:native` produce a **cache hit** for each other — i.e. the pixi task no longer bypasses the cache.
- [ ] `pixi run docs-build` and `pixi run build` resolve to the same turbo task and the same cache entry for `pathway-docs`.
- [ ] Cache invalidation behaves as intended, verified by touching one file at a time and inspecting `turbo run build --dry=json` or the run summary:
  - a `.rs` change under `crates/` invalidates `build:native`, `build` and `test`
  - a change to `packages/path/src/**` invalidates `build`/`test` but **not** `build:native`
  - an `.mdx` change under `apps/docs/src/content/` invalidates only the docs tasks
  - a `[workspace.package] version` bump in the root `Cargo.toml` invalidates the docs build and the rebuilt site quotes the new number
  - a `crates/**` edit leaves the docs cache **intact** (this is what `apps/docs/turbo.json` buys; without it the site inherits a phantom `build:native` dependency on the whole Rust workspace)
- [ ] The docs site renders the correct version with no `PATHWAY_VERSION` set anywhere.
- [ ] Record the wall-clock cost `astro check` adds to a cold `pixi run gates`. If it is large enough to discourage running gates locally, either drop `apps/docs` back out of the `typecheck` task or split a `gates-fast`; note the decision here.
- [ ] `pixi run lint-js` works under its new root script name (`lint:js`), and the lefthook `js-biome` / `js-typecheck` hooks still fire on the right globs (`apps/**` replaced `benches/**` in the typecheck glob).
- [ ] `turbo run dev --filter=pathway-docs` and `turbo run preview --filter=pathway-docs` behave as persistent tasks (`pixi run docs-dev` / `docs-preview`); `preview` correctly builds first.

## Definition of Done

- [ ] `pixi run ci` is green on a clean clone and in GitHub Actions.
- [ ] Any deviation found is fixed here, and `.knowledge/log.md` + `.knowledge/implementation/repo-structure.md` are corrected if the refactor's description turned out to be wrong in any detail.
