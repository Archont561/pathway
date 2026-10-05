# AI Agent Guidelines

This document provides guidelines for AI agents working on the pathway codebase.

**Start of session?** Follow [`.agents/skills/session/SKILL.md`](.agents/skills/session/SKILL.md): it
sequences restoration of the offline environment, backlog selection, the user-approved session
proposal, implementation, and the post-merge report. Its opening-prompt, standup, hand-off, and
report templates are in [`.agents/skills/session/standup-template.md`](.agents/skills/session/standup-template.md).

## Project Identity

**pathway** is a native, pathlib-inspired filesystem API for TypeScript, Bun, Node and Rust. Its central architectural claim is two ergonomic surfaces over the same engine:

- **TypeScript surface** (`@archont561/pathway`, packages/path) — Path objects, pluggable serializers, async iteration. All string manipulation stays here (pathe). 
- **Rust surface** (`pathway-fs`) — the ergonomic pathlib-like API over the same core (ships to crates.io as a separate crate; the core itself is `pathway-fs-core`).

Engine: **three-crate split (D7)**. All logic in `crates/core` (rlib, zero napi deps, testable without Node), `crates/path` (ergonomic Rust API over core), `crates/engine` (cdylib NAPI-RS bridge; npm-only, never published to crates.io).

## Knowledge Base

The `.knowledge/` directory contains the OKF v0.2 bundle (architecture, features, competitive, implementation). Always consult it before architectural decisions.

## Layout

`crates/core` (engine, napi-free) → `crates/path` (Rust API) → `crates/engine` (NAPI-RS bridge); `packages/path` (the TS surface); `apps/docs` (Astro/Starlight site). One version for everything: `[workspace.package] version` in the root `Cargo.toml`. `scripts/version.ts` reads it and the docs site imports that directly (no `PATHWAY_VERSION` environment variable — `apps/docs/turbo.json` declares the manifest a build input so the cache invalidates on a bump); a test in `packages/path/test` asserts `package.json` matches. Never bump a version anywhere else.

## Tooling

- **pixi** manages environments/tasks (conda-forge). Run commands with `pixi run <task>`.
- **bun** is the JS runtime/workspace manager (turbo, biome, backlog, skills).
- **turbo** orchestrates workspace builds/tests.
- **Orchestration rule — one verb, one path.** `pixi.toml` contains exactly two kinds of task: repo-wide verbs (`build`, `test`, `lint`, `fmt`, `typecheck`, `coverage`, `dev`, `build-release`), each one line of `bun run <verb>` into turbo; and repo management (changelog, backlog, sandbox transport, hooks) plus the two checks that cannot run offline. It must never use `cwd = "<a workspace package>"` and must never spell a cargo, tsc, astro or biome command. Commands live in the package that owns them: `crates/core`, `crates/path`, and `crates/engine` own their per-crate `nextest`/`clippy` scripts; `crates/package.json` owns workspace-wide Cargo commands such as rustfmt, cargo-deny, coverage, docs, and release builds; `packages/path` owns the addon; `apps/docs` owns the site. To reach one, filter — `turbo run test --filter=@repo/rust-core`, `turbo run lint --filter=@repo/rust --filter=@repo/rust-core`, `bun run docs:dev` — do not add a pixi task back. A task that shells into a package bypasses turbo's cache and silently diverges from the task turbo runs for the same verb.
- **Rust crates are Turbo packages for affected test/lint selection.** `@repo/rust-core`, `@repo/rust-path`, and `@repo/rust-engine` mirror the Cargo crates so `turbo run test --affected` / `turbo run lint --affected` can select changed crates and their dependents. Keep the workspace-wide commands in `@repo/rust` when they must remain one Cargo invocation or produce one workspace artifact.
- **No phantom tasks.** Do not declare a turbo task, a pixi task or a `workspaces` glob that nothing implements — it exits 0 having done nothing, which is worse than not existing. (This is why there is no `bench` task yet; it returns with the harness in backlog task-4.)
- **biome** formats + lints TS/JS.
- **lefthook** hooks call pixi tasks (CI parity). `pixi run hooks-install` once per clone.
- **convco** enforces conventional commits and generates CHANGELOG.
- **cargo** for Rust crates; `cargo nextest run --workspace` must pass without Node.
- **cargo-nextest** runs the Rust suite (process isolation, parallel by default). **cargo-llvm-cov** produces coverage — the conda-forge `rust` package already ships llvm-tools, so no `rustup component add`. **cargo-deny** enforces `deny.toml`.
- **backlog.md** (`pixi run backlog`) for Markdown tasks.
- **skills** (`pixi run skills`) for agent skills vendored under `.agents/skills/`.

## Mandatory source-change workflow: TDD and refactor skills

The repo vendors the process skills under `.agents/skills/` (pinned by `skills-lock.json` — consult these local copies, not external ones, because the lock pins what was reviewed). **Any change to source code must use the applicable skills below; do not treat this as optional guidance.**

- **Implementing a feature or fix → [`.agents/skills/tdd/SKILL.md`](.agents/skills/tdd/SKILL.md).** Read it before changing source, establish the public seam, and work the red → green loop: failing test first, minimal code to pass, one seam/one test/one implementation per cycle.
- **Restructuring without changing behaviour → [`.agents/skills/refactor/SKILL.md`](.agents/skills/refactor/SKILL.md).** Read it before refactoring, establish or improve tests first, then make small behavior-preserving steps with gates green between them.
- **A change that includes both behavior and restructuring must separate the workflows.** Complete the TDD red → green slice first; perform the refactor afterward, without mixing feature changes into the refactor step or commit.

The skills are the required process for *how* source changes are made; this file remains the reference for *what* (gates, layout, conventions). Documentation-only changes do not require a TDD cycle, but must still preserve the repository's documented contracts.

## Code Style

- Rust: `cargo fmt`, `cargo clippy` (warnings -> errors in CI). Prefer `Result<T, Error>`. The core must remain napi-free.
- TypeScript: strict mode; `exactOptionalPropertyTypes`; `noUncheckedIndexedAccess`; `noImplicitOverride`; `verbatimModuleSyntax`. Use biome.
- Boundary: **coarse-grained N-API** (bulk ops only, never per-`join()`). String ops stay in TS.

## Testing

- Rust: per-crate unit/integration tests run without Node under `cargo nextest`, through the `@repo/rust-core`, `@repo/rust-path`, and `@repo/rust-engine` Turbo packages. Core and path doctests are part of those crates' `test` scripts because nextest does not run doctests. For an all-Rust bypass of Turbo, `bun run --cwd crates test:all` still runs the whole Cargo workspace.
- TS: Bun tests in `packages/path/test/`. Package import and `engineAvailable()` are safe without a built `.node` file; consuming a walk requires the native addon. The `test` task depends on `build:native`, so turbo builds the addon before the suite runs — never put `napi build` inside a test script, which would hide that edge from the task graph.
- Coverage: `pixi run coverage` writes `crates/lcov.info`; `bun run --cwd crates coverage:report` prints the same numbers as a table.
- Dependency policy: `deny.toml` is the gate for the crates.io decision. The offline subset (bans, licences, sources) is inside `pixi run lint`; `pixi run lint-advisories` needs the network and is CI-only, because cargo-deny 0.20 always fetches the RustSec database. Adding a dependency means editing `deny.toml` if its licence is new.

## Commits

Conventional Commits (enforced by lefthook + convco): `type(scope): description`. `feat|fix|perf|refactor|docs|build|ci|test|chore|style`.

## Gates

Run `pixi run gates` before pushing. It is three verbs — `lint`, `typecheck`, `test` — because every check now lives behind one of them: `lint` carries rustfmt, clippy, cargo-deny, biome and actionlint; `test` carries the Rust unit tests, the Rust doctests and the Bun suites; `typecheck` carries `tsc --noEmit` and `astro check`. Adding a check means adding a script to the package that owns it, not a line in `pixi.toml`. Turbo caches all of it, so a commit that does not touch docs pays nothing for the docs. (`lint-sandbox-plan` and `lint-advisories` are outside `gates` by design; CI runs them where the release binary and the network exist.) `pixi run ci` adds coverage, the release-mode Rust build and the production build — which covers the TypeScript package *and* the Astro site in one turbo graph, so there is no separate docs step.

## Rule: Core is NAPI-free

`crates/core` must compile and test with `cargo test -p pathway-fs-core` in an environment with no Node. No `napi`/`napi-derive` dependencies in core.

## Questions

Consult `.knowledge/` first, then existing code patterns.

<!-- BEGIN:turborepo-agent-rules -->

# This is NOT the Turborepo you know

Turborepo configuration, task behavior, and CLI commands can vary between installed versions and may differ from your training data. Resolve the `turbo` package from this file's directory or relevant workspace; in monorepos, it may not be visible from the repository root. For example, run `node -p "require.resolve('turbo/package.json')"` from a workspace that depends on `turbo`.

Read `docs/README.md` inside that installed package first, then read the relevant pages from its `docs/` directory before changing Turborepo configuration or commands. Heed deprecation notices. These bundled docs match the installed package version and are available without network access.

This block is written and re-added by `turbo` before repository-scoped commands when an AI agent is detected. In the Turborepo source repository, its template is defined in `crates/turborepo-cli/src/cli/agent_guidance.rs`. Removing the managed block while updates are enabled means a later qualifying invocation will add it again. Set `"agentGuidance": false` in the root `turbo.json` or `turbo.jsonc` to opt out; this does not remove an existing block. Keep the block committed with your work to avoid an uncommitted change on the next agent invocation.
<!-- END:turborepo-agent-rules -->
