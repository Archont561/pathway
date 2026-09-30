# AI Agent Guidelines

This document provides guidelines for AI agents working on the pathway codebase.

## Project Identity

**pathway** is a native, pathlib-inspired filesystem API for TypeScript, Bun, Node and Rust. Its central architectural claim is two ergonomic surfaces over the same engine:

- **TypeScript surface** (`@myorg/path`, packages/path) — Path objects, pluggable serializers, async iteration. All string manipulation stays here (pathe). 
- **Rust surface** (`myorg-path`) — the ergonomic pathlib-like API over the same core (ships to crates.io as a separate crate; the core itself is `myorg-path-core`).

Engine: **three-crate split (D7)**. All logic in `crates/core` (rlib, zero napi deps, testable without Node), `crates/path` (ergonomic Rust API over core), `crates/engine` (cdylib NAPI-RS bridge; npm-only, never published to crates.io).

## Knowledge Base

The `.knowledge/` directory contains the OKF v0.2 bundle (architecture, features, competitive, implementation). Always consult it before architectural decisions.

## Layout

`crates/core` (engine, napi-free) → `crates/path` (Rust API) → `crates/engine` (NAPI-RS bridge); `packages/path` (the TS surface); `apps/docs` (Astro/Starlight site). One version for everything: `[workspace.package] version` in the root `Cargo.toml`. `scripts/version.ts` resolves it for the docs site, which takes it from `PATHWAY_VERSION` and falls back to the manifest; a test in `packages/path/test` asserts `package.json` matches. Never bump a version anywhere else.

## Tooling

- **pixi** manages environments/tasks (conda-forge). Run commands with `pixi run <task>`.
- **bun** is the JS runtime/workspace manager (turbo, biome, backlog, skills).
- **turbo** orchestrates workspace builds/tests.
- **biome** formats + lints TS/JS.
- **lefthook** hooks call pixi tasks (CI parity). `pixi run hooks-install` once per clone.
- **convco** enforces conventional commits and generates CHANGELOG.
- **cargo** for Rust crates; `cargo nextest run --workspace` must pass without Node.
- **cargo-nextest** runs the Rust suite (process isolation, parallel by default). **cargo-llvm-cov** produces coverage — the conda-forge `rust` package already ships llvm-tools, so no `rustup component add`. **cargo-deny** enforces `deny.toml`.
- **backlog.md** (`pixi run backlog`) for Markdown tasks.
- **skills** (`pixi run skills`) for agent skills vendored under `.agents/skills/`.

## Code Style

- Rust: `cargo fmt`, `cargo clippy` (warnings -> errors in CI). Prefer `Result<T, Error>`. The core must remain napi-free.
- TypeScript: strict mode; `exactOptionalPropertyTypes`; `noUncheckedIndexedAccess`; `noImplicitOverride`; `verbatimModuleSyntax`. Use biome.
- Boundary: **coarse-grained N-API** (bulk ops only, never per-`join()`). String ops stay in TS.

## Testing

- Rust: core + path unit tests run without Node, under `cargo nextest` (`pixi run test-rs`). Doctests are a separate task (`pixi run test-doc-rs`) because nextest does not run them. Engine glue has no logic to test yet.
- TS: Bun tests in `packages/path/test/`. Package must import without requiring a built `.node` file (`engineAvailable()` is safe). Build before runtime code paths that call native; the walk stub throws intentionally until implemented.
- Coverage: `pixi run coverage` (lcov) then `pixi run coverage-report`.
- Dependency policy: `deny.toml` is the gate for the crates.io decision. `pixi run deny` is offline and in the gate; `pixi run deny-advisories` needs the network and is CI-only, because cargo-deny 0.20 always fetches the RustSec database. Adding a dependency means editing `deny.toml` if its licence is new.

## Commits

Conventional Commits (enforced by lefthook + convco): `type(scope): description`. `feat|fix|perf|refactor|docs|build|ci|test|chore|style`.

## Gates

Run `pixi run gates` before pushing: fmt-check-rs, clippy, deny, lint-js, lint-actions, test-rs, test-doc-rs, test, typecheck. (sandbox-plan and deny-advisories are not in `gates` by design; CI runs them where the extra binaries and the network exist.) `pixi run ci` adds the release build and the docs site — an Astro build is too slow to make every commit wait on, but a broken site still fails CI.

## Rule: Core is NAPI-free

`crates/core` must compile and test with `cargo test -p myorg-path-core` in an environment with no Node. No `napi`/`napi-derive` dependencies in core.

## Questions

Consult `.knowledge/` first, then existing code patterns.

<!-- BEGIN:turborepo-agent-rules -->

# This is NOT the Turborepo you know

Turborepo configuration, task behavior, and CLI commands can vary between installed versions and may differ from your training data. Resolve the `turbo` package from this file's directory or relevant workspace; in monorepos, it may not be visible from the repository root. For example, run `node -p "require.resolve('turbo/package.json')"` from a workspace that depends on `turbo`.

Read `docs/README.md` inside that installed package first, then read the relevant pages from its `docs/` directory before changing Turborepo configuration or commands. Heed deprecation notices. These bundled docs match the installed package version and are available without network access.

This block is written and re-added by `turbo` before repository-scoped commands when an AI agent is detected. In the Turborepo source repository, its template is defined in `crates/turborepo-cli/src/cli/agent_guidance.rs`. Removing the managed block while updates are enabled means a later qualifying invocation will add it again. Set `"agentGuidance": false` in the root `turbo.json` or `turbo.jsonc` to opt out; this does not remove an existing block. Keep the block committed with your work to avoid an uncommitted change on the next agent invocation.
<!-- END:turborepo-agent-rules -->
