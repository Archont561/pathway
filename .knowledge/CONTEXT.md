---
type: Project Context
title: "Project Context and Decision Log"
description: "Identity, current state, foundational decisions D1–D7, market snapshot, roadmap, and bundle conventions for @archont561/pathway."
tags: [context, decisions, roadmap]
status: stable
generated:
  by: pathway_kb/1.0
  at: 2026-09-30T00:00:00Z
verified:
  - by: human:archont561
    at: 2026-09-16T00:00:00Z
domain: meta
created: 2025-07-11
source: conversation
depends_on: []
---

# @archont561/pathway — Project Context

## Identity

A **native, pathlib-inspired filesystem API** for TypeScript, Bun, and Node.
Built on a 3-layer stack: ergonomic TS surface → NAPI-RS bridge → Rust engine.
The engine core also ships as a **published Rust crate** (`pathway-fs`),
giving Rust projects the same pathlib-like surface over the same core (D7).

**One-line pitch:**
> The filesystem library that build tools wish they had.
> (And for Rust: pathlib's convenience + ripgrep's walker, one crate.)

**Target audience:**
Build tool authors, monorepo infrastructure, CLI frameworks, server frameworks —
anyone currently stitching together `tinyglobby` + `fs-extra` + `pathe` +
`proper-lockfile` + `tmp` + custom hashing.

---

## Current State

**Phase:** Pre-implementation. Knowledge base updated 2026-09-16 to
incorporate the Sept 2026 gap analysis (runtime lines, `node:fs.glob`,
Bun 1.4, NAPI-RS iterator/AsyncTask, `unrs-resolver`, reference-code
fixes, benchmark re-baseline), and on 2026-09-30 to add **D7** (publishable
Rust core + ergonomic `pathway-fs` crate; three-crate workspace — see
[rust-crate-surface.md](/architecture/rust-crate-surface.md)). No code written yet.

**Next action:** Begin Phase 1 per [phase-plan.md](../backlog/docs/phase-plan.md)
— **Step 0 pre-flight first**: owner sign-off on license + public name
(npm **and** crates.io names, per D7), NAPI-RS `async_iterator`/`AsyncTask`
spike, CI matrix refresh (Node 24/26, Bun 1.3/1.4), then scaffolding
(three-crate workspace: `core` rlib + `engine` cdylib + `path` stub)
+ Rust walker + TS Path class + benchmark harness (vs `node:fs.glob`,
fdir, tinyglobby, Bun 1.3/1.4).

---

## Foundational Decisions

### D1: Three-layer architecture
- **Layer 1 — TypeScript API:** `Path` objects, `Serializer<T>`, async iterators.
  Feels "almost boring." Path string manipulation stays in JS via `pathe`.
- **Layer 2 — NAPI-RS bridge:** Coarse-grained. No per-`join()` FFI hops.
  Crosses boundary only for bulk operations (walk, hash, read, write, copy).
- **Layer 3 — Rust engine:** `ignore` crate (ripgrep's walker) + `globset` +
  `regex` + `blake3` + `serde`. Owns the `FileSystem` trait internally but
  does NOT expose it through N-API.

### D2: Rust is NOT for `join()`
Rust exists exclusively for **large-scale filesystem operations**: traversal,
pruning, content hashing, parallel I/O. Single-string path manipulation stays
in TypeScript. This is the technical justification for the native dependency.

### D3: The Fused Walk is the defensible moat
Competitors (`fdir`, `tinyglobby`, `Bun.Glob.scan()`) return paths only.
Real applications immediately stat, hash, and filter those paths — generating
thousands of additional JS↔C++ boundary crossings. Our Rust engine performs
traversal + stat + hash + filter in a **single syscall pass** across OS worker
threads, yielding fully populated batches. This is the benchmark we must prove.

### D4: Pluggable serializer pattern
`read(toml)` / `write(json, data)` with TypeScript generics. Serializers are
independent strategy objects (`parse`/`stringify`), not baked into `Path`.
Supports JS serializers and native Serde codecs interchangeably. No global
registration — registries are per-`FileSystem` instance.

### D5: Bun is an ally, not a competitor
`Bun.Glob.scan()` provides fast native traversal but lacks composition
(path objects, serialization, hashing, atomicity, sandboxing). We use NAPI-RS
across both Node and Bun. Bun's N-API support is best-effort upstream, so our
own CI matrix is the support contract.

### D6: Rejections
- **WASM/WASI:** Skipped. Filesystem packages need real OS access, not
  sandboxed WASM. NAPI-RS docs warn against claiming Bun/Deno WASI support.
- **`bun:ffi`:** Rejected. Bun describes it as experimental; Node-API is the
  stable production route.
- **Global serializer registry:** Rejected. `Path.register()` is mutable global
  state. Use per-instance registries via `FileSystem.create({ serializers })`.
- **Per-file FFI hops:** Rejected. `path.join("a").join("b").join("c")` must
  NOT cross the N-API boundary three times.

### D7: One core, two surfaces — publish a Rust crate (added 2026-09-30)
The engine is split into `crates/core` (**rlib**, all logic, zero napi
deps) and `crates/engine` (thin **cdylib** NAPI-RS wrapper). A third
crate, `crates/path`, is published to crates.io as **`pathway-fs`** — an
ergonomic, pathlib-like Rust API over the same core: fused walk builder,
glob-on-a-path, integrated hashing, typed serde read/write with atomic
semantics, temp/lock/sandbox sugar. Rust-surface rules: platform-native
path semantics (`std::path`, not pathe), independent semver + MSRV
policy, engine never on crates.io, TS remains the primary product (a
Rust-surface gap never blocks an npm release). Core split lands in
Phase 1 scaffolding; the crate ships as a preview at v0.3 and stabilizes
at v1.0. Full spec:
[rust-crate-surface.md](/architecture/rust-crate-surface.md).

---

## Verified Market Data

> **Round 1 figures below are from July 2025 — stale; re-pull before any
> external publication.** Round 2 re-verification (Sept 2026): Node 24
> LTS / Node 26 Current / Node 20 EOL; **`node:fs.glob` stable in Node
> core** (enters our benchmark set); **Bun 1.4 rewrote Bun in Rust**
> (CI + benchmarks on 1.3 **and** 1.4); chokidar 5.0.0; Deno 2 NAPI
> support; **`unrs-resolver`** (our v1.0 Resolver is now an adapter over
> it); Effect Platform v4 as adjacent typed-FS layer; NAPI-RS experimental
> iterators + `AsyncTask` guidance. Full detail:
> [competitive/verified-data.md](/competitive/verified-data.md).

| Library | Weekly Downloads (Jul 2025) | Stars | Notes |
|---------|-----------------|-------|-------|
| `tinyglobby` | **186M** | 523 | Ecosystem default. Uses `fdir` + `picomatch`. |
| `fast-glob` | 73M | 2,695 | Author recommends tinyglobby as replacement. |
| `fdir` | 37M | 1,674 | Fastest JS crawler. ~1M files in <1s. |
| `pathe` | — | ~2.5k | Pure path strings. No I/O. Used by Vite/Nuxt. |
| `fs-extra` | — | ~9.5k | Legacy workhorse. No TS generics, no native speed. |
| `fs-jetpack` | — | ~1k | Closest philosophical match. **Unmaintained 3+ years.** |
| `Bun.Glob` | — | — | Native `scan()` traversal + `match()` (Rust since Bun 1.4). No composition. |
| `node:fs.glob` (core) | — | — | **Stable C++-native glob in Node core (v22.17/v24+). Paths only — our benchmark baseline.** |
| `unrs-resolver` | — | — | Published Rust resolver (ESM/CJS + tsconfig + PnP). We integrate, don't rebuild. |

**Key gap confirmed:** No published library combines path objects + native
traversal + pluggable serialization + TypeScript generics. Turbo and oxc have
internal Rust filesystem engines but don't publish them as libraries.

---

## Phase Roadmap

Moved to [`backlog/docs/release-roadmap.md`](../backlog/docs/release-roadmap.md).
The per-release scope table is delivery tracking, not design truth; step-level
detail is in [`backlog/docs/phase-plan.md`](../backlog/docs/phase-plan.md).

---

## Open Questions

> Status (Sept 2026 re-verification): **Q1 — answered, spike pending:**
> NAPI-RS now ships experimental `#[napi(iterator)]` /
> `#[napi(async_iterator)]` (native `AsyncGenerator`, pull-based,
> `return()` = cancellation). Phase 1 Step 0 spikes it; chunked paging
> remains the fallback. **Q2 — effectively answered:** the current NAPI-RS
> decision table prescribes `AsyncTask<T>` (libuv thread pool) for
> blocking/CPU work, sidestepping the Tokio-runtime question. **Q3 —**
> resolved by the Q1 outcome (batching becomes a prefetch window only if
> the iterator API is adopted). **Q4 — decision stands** (JSON in JS).

1. **NAPI-RS v3 streaming:** Has `AsyncGenerator` binding stabilized, or is
   explicit chunked paging still the cleanest pattern?
2. **Bun N-API edge cases:** Any recent quirks with `tokio::task::spawn_blocking`
   inside Bun's runtime?
3. **Batch size tuning:** Optimal chunk size for the fused walk (256? 512? 1024?)
   — needs empirical benchmarking.
4. **Serde→JS bridge for JSON:** Is native `serde_json` → N-API object creation
   actually faster than V8/JSC `JSON.parse()` for small files? Likely not —
   reserve native codecs for TOML/YAML/MessagePack.

---

## File Conventions

This knowledge base is an **Open Knowledge Format (OKF) v0.2** bundle
(`okf_version: "0.2"` in [index.md](/index.md); spec:
`GoogleCloudPlatform/open-knowledge-format`). One file = one concept;
the reserved files are `index.md` (progressive-disclosure index) and
`log.md` (update log, newest first). Concept links use
bundle-root-absolute paths (`/architecture/fused-walk.md`).

Concept frontmatter:

```yaml
---
type: Project Context | Architecture Decision | Feature Spec |
      Market Intelligence | Implementation Spec |
      Reference Implementation | Roadmap    # required by OKF
title: "Human-readable title"
description: "One-line summary, mirrored in index.md"
tags: [tag1, tag2]
status: stable | draft | deprecated        # OKF lifecycle (§5.4)
generated:                                  # last content change (§5.2)
  by: pathway_kb/1.0
  at: YYYY-MM-DDThh:mm:ssZ
verified:                                   # trust events (§5.2/§5.3)
  - by: human:archont561                   #   human: ⇒ human-reviewed
    at: YYYY-MM-DDThh:mm:ssZ
  - by: process:gap-analysis-2026-09       #   process: ⇒ machine-confirmed
    at: YYYY-MM-DDThh:mm:ssZ
stale_after: YYYY-MM-DDThh:mm:ssZ           # only where facts decay
domain: architecture | features | competitive | implementation | meta
decision: decided | proposed | deprecated   # legacy KB decision status
created: YYYY-MM-DD
source: conversation | web-search | benchmark
depends_on:
  - domain/filename       # concept IDs: bundle path without .md
---
```

`domain`, `decision`, `created`, `source`, and `depends_on` are
producer-defined extension keys; OKF consumers preserve unknown keys.

Navigate via [index.md](/index.md).

---

## Session scratchpad

Dated working notes between sessions, newest last. Proposals and measurements
live here until they become backlog tasks or merged code — never as checked
acceptance criteria or speculative source files.

### 2026-10-07 — task-30 (walk errors) merged as PR #14

Landed: `feat(walk): surface native traversal errors as WalkError`
(task-30), main at `403baba`. `drive()` reads `walker.errors()` once after
the last batch and throws the exported `WalkError` (verbatim messages,
engine-capped at 1,000) instead of completing; per-entry `PathEntry.error`
still streams mid-walk, abort wins, early break stays silent. Side effect: TS
`hashTree` fails loudly on partial trees, matching core `hash_tree`. Suite on
merged main: 144 passing / 0 skipped (was 142/0 at open). Post-merge runs all
success: ci 37623753784, runtime matrix 37623753669, publish sandbox
37623753550, docs 37624136027. Transport repacked at source.commit `403baba`
(pixi 0.81.0, pixi-sandbox 0.5.2, pixi-unpack 0.7.11).

Open, in recommended order: task-12 (High, m-1, temp dirs) or task-14 (High,
m-1, snapshots/diff) next; task-28 Phase A (fixture kits, no new deps) is the
alternative first slice — its plan argues the kit should land before
task-12's suite copies tree-building code again. task-16 (Medium, locking)
unblocks task-21 + task-26. task-19's crates.io publish needs maintainer
credentials (not provable in the sandbox). Unverified observation: core's
`an_unreadable_file_is_reported_without_aborting_the_walk` imports
`std::os::unix` with no `#[cfg(unix)]` — worth a look from a Windows runner;
the task-30 TS tests are premise-guarded so safe everywhere.

Next session should start with:

> Restore Pathway's sandbox and baseline the suite (expect 144 passing / 0
> skipped — transport repacked at 403baba, pixi 0.81.0 / pixi-sandbox 0.5.2),
> then read `.knowledge/CONTEXT.md` § Session scratchpad — the 2026-10-07
> heading lists the open items.
>
> Task-12 (temp dirs, tiered guarantee settled in
> `.knowledge/features/killer-features.md` §1) is the recommended scope;
> confirm it or pick task-14 / task-28-Phase-A instead. Work in slices:
> core + engine + TS surface with premise-guarded tests first (fully local),
> then nothing external — no push/release proof needed beyond the PR checks.
>
> Propose the slice and stop. Repository rules are in `AGENTS.md`; the
> session procedure and handoff templates are in `.agents/skills/session/`.

### 2026-10-07 — task-12 (temp dirs) merged as PR #16

Landed: `feat(temp): deliver scoped temp directories with tiered cleanup`
(task-12), main at `4dc34e8`. `Path.temp(options?, callback)` and
`FileSystem#temp` scope a directory to a callback — removed on return and on
throw — and `tempDir()` returns the handle (`path`, `remove`, `keep`) for a
directory that must outlive one callback. Core owns the mechanism (`TempDir`
guard, a path-only registry of live directories, `cleanup_live_temp_dirs()`),
the engine bridges it, and the TS surface installs a lazy `process.on("exit")`
flush — never `SIGINT`/`SIGTERM` handlers, because reinterpreting the host's
signals is a takeover, not a guarantee. Suite on merged main: 168 passing / 0
skipped (was 144/0; core 75 nextest + 34 doctests, engine 4, path 3, Bun 52
across 7 files). Post-merge runs all success: ci 37634502734, runtime matrix
37634502788, publish sandbox 37634502761, docs 37634730851. Transport repacked
at source.commit `4dc34e8` (pixi 0.81.0, pixi-sandbox 0.5.2, pixi-unpack
0.7.11), lock_sha256 unchanged at `9e15a944` — no dependency moved.

Two KB claims were false and are corrected in
`.knowledge/features/killer-features.md` §1 and `backlog/docs/phase-plan.md`
Phase 2, each pinned by a test rather than by prose:

- **A temp directory is never tier 2.** Probed: `open(dir, O_TMPFILE |
  O_DIRECTORY | O_RDWR)` returns an unnamed *regular* file (`kind=reg`,
  `nlink=0`) — the kernel ignores `O_DIRECTORY` — and `openat` inside it fails
  `ENOTDIR`. `O_TMPFILE` / `DELETE_ON_CLOSE` are a property of unnamed temp
  *files*, so the hard SIGKILL row now says exactly that and the file
  primitive is TASK-31. `linux_cannot_make_a_temp_directory_anonymous` is the
  regression test that keeps the claim honest.
- **`tempfile` has no atexit hook**, and `process.exit()` runs no destructors.
  Measured on Node 22.22.3 and Bun 1.3.11: an `exit` hook runs for
  `process.exit(0)` and for a signal whose handler exits cleanly, and for
  neither a default-disposition `SIGINT`/`SIGTERM` nor `SIGKILL`. Tier 1 is
  therefore the registry plus the flush this task added; `SIGINT`, `SIGTERM`
  and `SIGKILL` are tier 3, tested as observed (child-process probes in core
  and in the Bun suite).

TASK-30 was finished in PR #14 but left at `status: To Do`; closed here.
TASK-31 (unnamed temp files, tier 2) filed with a description.

Environment facts for next time. The sandbox rolled state back between turns
mid-session: the working branch's ref returned to the clone base (`2739744`)
and `.pixi` plus `.pixi-sandbox` disappeared, so the rebaseline needed a fresh
`bash scripts/restore.sh`. The landed commits survived only because they had
been pushed — commit and push before ending a turn, and treat a local `git log`
as untrusted at the start of the next one. Also, `git fetch origin` does not
bring `sandbox/*`; read the manifest with
`git fetch origin 'refs/heads/sandbox/*:refs/remotes/origin/sandbox/*' --depth 1`.

Open, in recommended order: task-14 (High, m-1, snapshots / persistence /
diff) or task-28 Phase A (fixture kits) next — task-28's note that the
transport lacks the new dev-dependencies is **stale**: the restored vendor tree
carries rstest 0.27.0, proptest 1.11.0 and fast-check 4.10.2, and they compile
offline, so Phase B is reachable from an airlocked sandbox too. task-16
(Medium, locking) unblocks task-21 + task-26. TASK-31 (High) is the tier-2 file
half of this task. task-19's crates.io publish still needs maintainer
credentials.

Verified by inspection, not yet by a runner: three tests in
`crates/core/src/walk/scanner.rs` (~806 symlinks, ~872 and ~1086 permissions)
`use std::os::unix::…` with no `#[cfg(unix)]`, so `cargo nextest` would fail to
*compile* on Windows — invisible today because `ci` runs the Rust suite on
ubuntu only and the runtime matrix builds the addon but runs just the JS
suites. Gate those three tests, or add a Windows Rust job (task-11/task-27
territory). The temp-dir signal probes are `#[cfg(unix)]`-gated and skip on
Windows, so the Windows rows of the temp tier table remain design, not proof.

Next session should start with:

> Restore Pathway's sandbox and baseline the suite (expect 168 passing / 0
> skipped — transport repacked at 4dc34e8, pixi 0.81.0 / pixi-sandbox 0.5.2),
> then read `.knowledge/CONTEXT.md` § Session scratchpad — the newest
> 2026-10-07 heading (task-12 / PR #16) lists the open items.
>
> Task-14 (snapshots, persistence and diffing) is the recommended scope;
> task-28 Phase A (fixture kits — its "transport not repacked" prerequisite is
> stale, the dev-dependencies are vendored and compile offline) and task-16
> (locking, unblocks task-21 + task-26) are the alternatives. Work in slices:
> core + engine + TS surface with premise-guarded tests first (fully local),
> then nothing external — no push/release proof needed beyond the PR checks.
>
> Propose the slice and stop. Repository rules are in `AGENTS.md`; the session
> procedure and handoff templates are in `.agents/skills/session/`.
