# pathway

A native, pathlib-inspired filesystem API for TypeScript, Bun, Node and Rust. One engine, two ergonomic surfaces:

- **TypeScript surface (`@archont561/pathway`)** — Path objects, pluggable serializers, async iteration. String manipulation stays in TypeScript (`pathe`).
- **Rust surface (`pathway-fs`)** — The same engine behind an ergonomic pathlib-like API, publishable to crates.io; the core is `pathway-fs-core`.

**Three-crate split (D7):** all engine logic in `crates/core` (rlib, zero N-API dependencies, testable without Node), `crates/path` (Rust API over core), `crates/engine` (cdylib NAPI-RS bridge, npm-only).

```
pathway/
├── crates/core        # engine logic, napi-free: the whole test story runs without Node
├── crates/path        # the ergonomic Rust API over core
├── crates/engine      # the NAPI-RS bridge; npm-only, never on crates.io
├── packages/path      # @archont561/pathway, the TypeScript surface
├── apps/docs          # the documentation site (Astro + Starlight)
└── .knowledge/        # the source of truth for architecture and phases
```

## Getting started

```bash
# Install hooks once per clone
pixi run hooks-install

# Build everything (TS workspace via turbo + Rust crates)
pixi run build

# Run tests (Rust via cargo-nextest + TS via turbo)
pixi run gates

# The documentation site
pixi run docs-dev

# Inspect backlog/tasks
pixi run backlog

# Browse skills
pixi run skills
```

## Checks

| Command | What it does |
|---------|--------------|
| `pixi run gates` | Everything that must pass before a push: `cargo fmt --check`, `clippy -D warnings`, `cargo deny` (licences/bans/sources), biome, actionlint, `cargo nextest`, doctests, Bun tests, `tsc --noEmit` |
| `pixi run deny` | Dependency policy from `deny.toml`. Offline, so it also runs in the airlocked sandbox |
| `pixi run deny-advisories` | The RustSec advisory check. Needs the network — CI only |
| `pixi run coverage` / `pixi run coverage-report` | Rust coverage via `cargo-llvm-cov`, written to `lcov.info` |
| `pixi run test-doc-rs` | Doctests, which nextest does not run |

`deny.toml` is the gate for the crates.io decision: two of the three crates are published, so a dependency with a non-permissive or unmaintained licence has to fail a gate rather than be discovered by a consumer.

## Documentation

`apps/docs` is an [Astro](https://astro.build) + [Starlight](https://starlight.astro.build) site:

```sh
pixi run docs-dev      # dev server
pixi run docs-build    # production build (also covered by `pixi run build`)
pixi run docs-check    # type-check, frontmatter and routes included
```

Each of these is a filter over the same turbo graph the repo-wide verbs use (`turbo run <task> --filter=pathway-docs`), so `pixi run build` and `pixi run docs-build` share one task and one cache entry rather than being two ways to build the same site.

The version the site documents is read from `[workspace.package] version` in the root `Cargo.toml` — the one place it is written down — through `scripts/version.ts`. A test asserts `packages/path/package.json` agrees with Cargo, so the three published surfaces cannot drift. `apps/docs/turbo.json` declares that manifest and that script as the docs build's inputs, so the site's cache turns over on a version bump and *not* on every unrelated Rust change.

## Development workflow

The project uses [pixi](https://pixi.sh) for environments and task orchestration, [bun](https://bun.sh/) as the JS runtime/workspace manager, [turbo](https://turbo.build/) to orchestrate workspace builds/tests, and [biome](https://biomejs.dev/) for JS/TS formatting/linting. Rust uses `cargo` and is linted with `clippy`.

The N-API boundary is **coarse-grained** (bulk operations only). Never add a per-`join()` FFI hop.

## Architecture

The knowledge base (`.knowledge/`) is the source of truth. See `CONTEXT.md`, `architecture/`, `implementation/`, `features/`, and `competitive/`. No major architectural decision is made without consulting it first.

## Release

Release notes are generated with [convco](https://convco.github.io/) from conventional commits. The offline sandbox is managed by `pixi-sandbox` (separate branch transport). See `AGENTS.md` for agent guidelines and `.knowledge/` for the full phase plan.

## License

MIT — see [LICENSE](LICENSE). The license is settled; the public npm and crates.io names are not, so the crates are still `publish = false` (see `.knowledge/implementation/phase-plan.md`, Phase 1 Step 0).
