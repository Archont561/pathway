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

# Build everything: the native addon, the TypeScript packages, the docs site
pixi run build

# Everything that must pass before a push
pixi run gates

# The documentation site, live
pixi run dev

# Inspect backlog/tasks
pixi run backlog

# Browse skills
pixi run skills
```

## Checks

Pixi exposes repo-wide verbs only. Each one is `turbo run <verb>` over every package that implements it, so a verb covers both languages at once and you never have to think about which half of the repo a check belongs to.

| Command | What it does |
|---------|--------------|
| `pixi run lint` | `cargo fmt --check`, `clippy -D warnings`, `cargo deny` (licences/bans/sources), biome, actionlint |
| `pixi run test` | `cargo nextest` over the workspace, the Rust doctests, and the Bun suites |
| `pixi run typecheck` | `tsc --noEmit` and `astro check` |
| `pixi run gates` | The three above — everything that must pass before a push |
| `pixi run coverage` | Rust coverage via `cargo-llvm-cov`, written to `crates/lcov.info` |
| `pixi run ci` | `gates` plus coverage and the release and production builds: exactly what CI runs |
| `pixi run lint-advisories` | The RustSec advisory check. Needs the network, so it is CI-only and outside `lint` |

`deny.toml` is the gate for the crates.io decision: two of the three crates are published, so a dependency with a non-permissive or unmaintained licence has to fail a gate rather than be discovered by a consumer.

### Reaching one package

The commands themselves live in the package that owns them — `crates/package.json` owns every cargo command, `packages/path` the native addon, `apps/docs` the site. Pixi does not re-export them. To run one directly, filter the same turbo graph:

```sh
turbo run lint --filter=@repo/rust     # just the Rust gate
bun run docs:dev                       # just the docs site
bun run --cwd crates coverage:report   # just the coverage summary
```

Because it is one graph, `pixi run build` and `bun run docs:build` share a task and a cache entry rather than being two ways to build the same site.

## Documentation

`apps/docs` is an [Astro](https://astro.build) + [Starlight](https://starlight.astro.build) site. `pixi run dev` serves it locally; `pixi run build` and `pixi run typecheck` cover it as part of the repo-wide verbs, so it needs no workflow or task of its own.

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
