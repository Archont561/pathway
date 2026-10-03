---
id: doc-phase-plan
title: "Phase 1-4 Implementation Plan, v0.1-v1.0 Roadmap, Benchmark Harness"
type: document
status: active
created_date: '2025-07-11'
updated_date: '2026-10-02'
tags: [phase, roadmap, benchmark, v0.1, v1.0, plan]
---

> Moved out of the knowledge base on 2026-10-02. This is delivery tracking, not
> design truth: it carries 85 checkboxes that map onto milestones `m-0`-`m-3`
> and tasks 1-27, so it belongs beside them. The design documents it draws on
> stay in `.knowledge/` and remain the source of design truth - see
> [knowledge-base-feature-map.md](knowledge-base-feature-map.md) for the
> traceability index.


# Implementation Phase Plan

## Overview

The project is executed in four disciplined phases. Each phase has a clear
scope, deliverables, and success criteria. **No phase begins until the
previous phase's success criteria are met.**

---

## Phase 1: Foundation (v0.1)

### Scope
Rust walker + TypeScript `Path` class + JSON serializer + benchmark proof.

### Steps

#### Step 0: Pre-Flight (added Sept 2026 — before any code)

Gates from the gap analysis; Phase 1 does not start until these are done:

- [x] **Owner sign-off: license + public package name.** MIT (single licence,
      one LICENSE file) and **`@archont561/pathway`** on npm — decided
      2026-09-30. npm forbids uppercase in package names, so the scope is
      lowercase even though the GitHub owner is `Archont561`.
- [ ] **Reserve crates.io names** for the Rust surface (D7). Decided:
      `pathway-fs` and `pathway-fs-core` — `pathway` itself is taken on
      crates.io (a 2022 placeholder, v0.0.0), hence the `-fs` suffix; the
      engine glue stays `pathway-fs-engine`, `publish = false` forever. The
      reservation on crates.io is still outstanding. See
      [rust-crate-surface.md](../../.knowledge/architecture/rust-crate-surface.md).
- [ ] **Fix the reference-code defects** now documented in
      [code-rust-walker.md](../../.knowledge/implementation/code-rust-walker.md) /
      [code-ts-path.md](../../.knowledge/implementation/code-ts-path.md): walkDirs filter, `Vec<String>`
      globs, generated binding loader, root-relative glob matching, error
      collection, `dot`/`gitignore`, chunked hashing, `cancel()`/`errors()`.
- [ ] **NAPI-RS spike (1–2 days):** `#[napi(async_iterator)]` +
      `AsyncTask` across Node 24/26 + Bun 1.3/1.4, per the napi.rs test
      checklist → freeze the streaming architecture (native async
      iterator vs. chunked paging fallback).
- [ ] **Refresh the CI matrix** (ci-distribution.md): Node 24/26, Bun
      1.3/1.4, cross toolchains, npm provenance, install smoke test.
- [ ] **Re-pull npm download stats** for any external benchmark document
      (the July 2025 figures are stale).

#### Step 1.1: Project Scaffolding (Week 1)
- [ ] Initialize Cargo workspace with the **three-crate split (D7)**:
      `crates/core` (rlib, all logic, zero napi deps), `crates/engine`
      (cdylib NAPI wrapper, `publish = false`), `crates/path` (stub for
      the v0.3 Rust API crate)
- [ ] Verify `cargo test -p pathway-fs-core` runs green **without Node**
- [ ] Initialize the JS workspace around `packages/path` (bun workspaces — the repo's only JS runtime; there is no pnpm)
- [ ] Configure NAPI-RS v3 with `tokio` async support
- [ ] Set up TypeScript build (tsc, vitest)
- [ ] Verify "hello world" NAPI-RS binding compiles and loads on Node + Bun
- [ ] Set up GitHub Actions CI skeleton (see [ci-distribution.md](../../.knowledge/implementation/ci-distribution.md))

#### Step 1.2: Rust Traversal Engine (Weeks 2–3)

All items below are implemented in **`crates/core`** (napi-free);
`crates/engine` only exposes them over N-API.

- [ ] Implement `NativeScanner` using `ignore` crate
- [ ] Add `globset` glob matching — **Vec of patterns, AND logic, matched
      against root-relative paths** (see code-rust-walker.md)
- [ ] Add `regex` filtering (full absolute path)
- [ ] Implement pre-descent directory exclusion (pruning)
- [ ] Add `dot` (default false → `hidden(true)`) and `gitignore` options
- [ ] Add `absolute` option (default: root-relative paths)
- [ ] Implement batched result yielding (default batch size: 512)
- [ ] Add `withMetadata` option (stat info via `DirEntry::metadata`)
- [ ] Add BLAKE3/xxhash/sha256 content hashing via **chunked 64 KB reads**
      (never whole-file loads)
- [ ] Collect traversal/hash errors (`errors()` + per-entry `error`)
- [ ] Add `cancel()` (AtomicBool, wired to JS `AbortSignal`)
- [ ] Write Rust unit tests for all walker configurations, **including the
      glob-semantics matrix** (nested/root-level `**/*.ts`, `*.ts`,
      Windows separators)

**See:** [code-rust-walker.md](../../.knowledge/implementation/code-rust-walker.md) for the implementation.

#### Step 1.3: TypeScript API (Weeks 3–4)
- [ ] Implement `Path` class with `pathe` for string ops
- [ ] Implement `Path.cwd()`, `join()`, `relativeTo()`
- [ ] Implement `readText()`, `writeText()`, `writeTextAtomic()`
- [ ] Implement `Serializer<T>` interface and built-in `json` serializer
- [ ] Implement `read<T>(serializer)` and `write<T>(serializer, data)`
- [ ] Implement `WalkIterator` async generator with batch consumption
- [ ] Implement `walkFiles()`, `walkDirs()`, `walk()`
- [ ] Write TypeScript tests for all Path operations

**See:** [code-ts-path.md](../../.knowledge/implementation/code-ts-path.md) for the implementation.

#### Step 1.4: Benchmark Harness (Week 4)
- [ ] Create file tree generator (10k, 100k, 500k, 1M files)
- [ ] Benchmark A: Raw traversal (paths only)
- [ ] Benchmark B: Traversal + complex exclusion
- [ ] Benchmark C: Fused walk (traverse + stat + hash)
- [ ] Compare against: **`node:fs.glob`** (native C++ baseline), `fdir`,
      `tinyglobby`, `Bun.Glob.scan()` (Bun 1.3 **and** 1.4)
- [ ] Record per run: wall time (p50/p95), peak heap, GC pressure,
      time-to-first-entry, cancellation cost
- [ ] Document results and determine if ≥5x speedup is achieved

**See:** Benchmark section below for the full test plan.

### Success Criteria
- [ ] Fused walk ≥5x faster than the best alternative on 100k+ files —
      the baseline on Node 24 is `node:fs.glob` + `fs.stat` + `crypto`
      (native C++), not just pure JS
- [ ] All tests pass on Node 24 (LTS), Node 26 (Current), Bun 1.3.x,
      Bun 1.4.x
- [ ] All tests pass on Linux (glibc), macOS (arm64), Windows (x64)
- [ ] Package installs and loads correctly via NAPI-RS platform binaries
      (including the oldest-supported-npm install smoke test)
- [ ] API matches the design in [walk-traversal.md](../../.knowledge/features/walk-traversal.md)
      (including `dot`, `gitignore`, `absolute`, `signal`, error reporting)

---

## Phase 2: Build System Features (v0.2)

### Scope
Temp dirs, content hashing, directory snapshots, additional hashers.

### Deliverables
- [ ] `Path.temp(callback)` — RAII temp directories via `tempfile` crate
      + `O_TMPFILE` / `DELETE_ON_CLOSE` for the hard guarantee (tiered
      cleanup documented per [killer-features.md](../../.knowledge/features/killer-features.md))
- [ ] `file.hash(hasher)` — Single-file content hashing
- [ ] `project.hashTree(options)` — Parallel tree hashing (dedicated
      rayon stage over pruned paths)
- [ ] `Hasher` interface with `blake3`, `xxhash`, `sha256` implementations
- [ ] `project.snapshot(options)` — Directory snapshot with stat + hash
- [ ] `Snapshot.diff(other)` — Added/removed/modified/unchanged
- [ ] `Snapshot.save()` / `Snapshot.load()` — Persistence for incremental
      builds; **full nanosecond mtime precision** in the persisted format
      and **sorted-path fold** (deterministic across runs/machines)

### Success Criteria
- [ ] Snapshot + diff on 100k files completes in <500ms
- [ ] Temp dir cleanup verified: throw/exit (tier 1) and SIGKILL on
      Linux local FS + Windows (tier 2) — tier-3 platforms documented
- [ ] Tree hash is deterministic across runs
- [ ] Two files written in the same millisecond are distinguished in
      mtime-mode diffs (ns-precision check)

---

## Phase 3: Infrastructure Features (v0.3)

### Scope
Sandbox, file locking, parallel bulk ops, transformers.

### Deliverables
- [ ] `project.sandbox(subdir)` — Path containment with **per-component
      realpath / fd-based `openat(O_NOFOLLOW)`** (final-path checks alone
      miss intermediate symlink escapes + TOCTOU — see
      killer-features.md hardening requirements)
- [ ] `file.withLock(callback)` — Native `flock()` / `LockFileEx()`,
      sidecar option, NFS/Windows caveats documented
- [ ] `project.copyTo(dest, options)` — Parallel recursive copy
- [ ] `path.moveTo(dest)` / `rename()` — fs-extra `move` parity
- [ ] `project.transform(dest, options)` — Parallel file transformation
- [ ] `Transformer` interface with `gzip`, `brotli` implementations
- [ ] Symlink controls: `followSymlinks` walk option, `lstat` support,
      broken-symlink policy (`throwOnError` parity)
- [ ] Atomic-write hardening shipped: `O_EXCL` temp creation +
      best-effort directory fsync after rename
- [ ] `ContainmentError` with type-level `SandboxedPath` brand
- [ ] **`pathway-fs` Rust crate preview on crates.io (D7):** fluent
      walk builder, glob-on-a-path, hashing, typed serde read/write
      (atomic), temp/lock sugar over `crates/core`; docs.rs docs +
      README quickstart. Independent semver; a gap here never blocks
      the npm release
      ([rust-crate-surface.md](../../.knowledge/architecture/rust-crate-surface.md))

### Success Criteria
- [ ] Sandbox blocks the full test matrix: `../`, intermediate symlinks,
      symlink loops, case-insensitive FS, Unicode normalization,
      `public-evil` prefix collision
- [ ] File locking verified under concurrent access (10 processes)
- [ ] Parallel copy of 50k files ≥3x faster than `fs-extra.copy()`

---

## Phase 4: Power User Features (v0.4 → v1.0)

### Scope
Transactions, native codecs, detection, resolution, watching.

### Deliverables
- [ ] `Path.transaction(callback)` — Best-effort transactional operations
- [ ] `@archont561/pathway-toml` — Native TOML serializer via `toml-rs`
- [ ] `@archont561/pathway-yaml` — Native YAML serializer via `serde_yaml`
- [ ] `Detector` interface with `mime`, `encoding` implementations
- [ ] `Validator` interface with Zod/Valibot integration examples
- [ ] **`Resolver` interface backed by `unrs-resolver`** (Rust crate in
      the engine, npm package as fallback) — adapter, NOT a from-scratch
      resolver (Sept 2026 decision); interface stays open for other
      implementations
- [ ] `project.watch()` — Native filesystem watching via `notify` crate
      (makes the "uninstall chokidar" pitch true at v0.4)
- [ ] **Content-Addressed Store** (`FileSystem.cas()`) — backs the
      "content-addressed caching primitives" pitch (pluggable-patterns.md §F)
- [ ] `FileSystem.create({ serializers })` — Per-instance registry
- [ ] Full CI matrix with all 7 platform targets

### v1.0 Success Criteria
- [ ] All features from [killer-features.md](../../.knowledge/features/killer-features.md) shipped
- [ ] All pluggable patterns from [pluggable-patterns.md](../../.knowledge/features/pluggable-patterns.md) implemented
- [ ] **Rust surface stabilized (D7):** `pathway-fs` 1.0 on crates.io,
      MSRV CI-enforced, docs.rs coverage for the full walk/hash/serde
      surface
- [ ] 10k+ GitHub stars
- [ ] 1M+ weekly npm downloads
- [ ] Adopted by ≥2 major build tools or frameworks

---

## Benchmark Harness

### File Tree Generator

```ts
// benches/fixtures/generate.ts
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "pathe";

async function generateTree(root: string, fileCount: number): Promise<void> {
  const dirs = ["src", "src/components", "src/utils", "lib", "test", "docs"];
  const extensions = [".ts", ".tsx", ".js", ".json", ".css", ".md"];

  for (let i = 0; i < fileCount; i++) {
    const dir = dirs[i % dirs.length];
    const ext = extensions[i % extensions.length];
    const filePath = join(root, dir, `file-${i}${ext}`);
    await mkdir(join(root, dir), { recursive: true });
    await writeFile(filePath, `// File ${i}\nexport const x = ${i};\n`);
  }

  // Add node_modules to test exclusion
  for (let i = 0; i < Math.floor(fileCount * 0.3); i++) {
    const filePath = join(root, "node_modules", "pkg", `file-${i}.js`);
    await mkdir(join(root, "node_modules", "pkg"), { recursive: true });
    await writeFile(filePath, `module.exports = ${i};\n`);
  }
}
```

### Benchmark Suite

```ts
// benches/fused-walk.bench.ts
import { bench, group, run } from "mitata";
import { Path } from "@archont561/pathway";
import { fdir } from "fdir";
import { glob } from "tinyglobby";
import { glob as nodeGlob, readFile } from "node:fs/promises";
import { createHash } from "node:crypto";

const ROOT = process.argv[2] || "./fixtures/100k";

// Wall time is not enough: record peak heap + GC pressure per run
// (see "Measured Metrics" in fused-walk.md), plus time-to-first-entry
// and cancellation cost.

group("Raw Traversal (paths only)", () => {
  bench("node:fs.glob (native C++)", async () => {
    return [...await nodeGlob("**/*.ts", { cwd: ROOT })];
  });

  bench("fdir", async () => {
    return new fdir().glob("**/*.ts").crawl(ROOT);
  });

  bench("tinyglobby", async () => {
    return glob("**/*.ts", { cwd: ROOT });
  });

  bench("@archont561/pathway (native)", async () => {
    const results: string[] = [];
    for await (const f of new Path(ROOT).walkFiles({ glob: "**/*.ts" })) {
      results.push(f.value);
    }
    return results;
  });
});

group("Fused Walk (traverse + stat + hash)", () => {
  bench("node:fs.glob + fs.readFile + crypto", async () => {
    const paths = [...await nodeGlob("**/*.ts", { cwd: ROOT })];
    return Promise.all(paths.map(async (p: string) => {
      const content = await readFile(p);
      const hash = createHash("sha256").update(content).digest("hex");
      return { path: p, hash };
    }));
  });

  bench("fdir + fs.stat + crypto", async () => {
    const paths = await new fdir().glob("**/*.ts").crawl(ROOT);
    return Promise.all(paths.map(async (p: string) => {
      const content = await readFile(p);
      const hash = createHash("sha256").update(content).digest("hex");
      return { path: p, hash };
    }));
  });

  bench("@archont561/pathway fused (native)", async () => {
    const results: any[] = [];
    for await (const f of new Path(ROOT).walkFiles({
      glob: "**/*.ts",
      withMetadata: true,
      hash: "blake3",
    })) {
      results.push({ path: f.value, size: f.size, hash: f.hash });
    }
    return results;
  });
});

await run();
```

> **Competitor set (Sept 2026):** `node:fs.glob` is stable and C++-native
> in Node core — it is the fused walk's primary baseline on Node 24.
> The Bun comparison runs on **both** Bun 1.3 and 1.4 (Rust rewrite).

### Test Matrix

| Tree Size | Filter | Hash | Expected JS Time | Target Native Time |
|-----------|--------|------|-----------------|-------------------|
| 10k | glob | none | ~50ms | ~20ms |
| 100k | glob + exclude | none | ~300ms | ~50ms |
| 100k | glob + exclude | BLAKE3 | ~2,500ms | ~200ms |
| 500k | glob + regex + exclude | BLAKE3 | ~15,000ms | ~800ms |
| 1M | glob + exclude | xxHash | ~30,000ms | ~1,200ms |

### Success Threshold

The fused walk (traverse + stat + hash) must be **≥5x faster** than the
best alternative — on Node 24 that means `node:fs.glob` + `fs.stat` +
`crypto` (the native C++ baseline), not just pure JS. If the improvement is
only 5–10%, the Rust complexity is not justified. If it is 10–20x faster
with lower memory, that is the foundation of the project.

> **Measured 2026-10-03 (backlog task-4): the threshold FAILS.** The fused walk
> is **1.85x** faster than the strongest baseline at 100k files (`fdir` + a
> 32-wide stat/hash pool), 3.8x against the serial `node:fs.glob` pipeline the
> threshold assumes. Peak heap (14.3 vs 39.3 MiB) and GC (2.8 vs 20.6 ms per
> sample) do hold up. The estimated table above is superseded — the measured
> tables, the methodology traps (debug addon, unsettled page cache) and the gap
> analysis are in
> [verified-data.md](../../.knowledge/competitive/verified-data.md); the
> re-baselining options are in
> [fused-walk.md](../../.knowledge/architecture/fused-walk.md). **The ≥5x and
> 10–20x figures are not available to marketing.**

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Phase 1 = walker + Path + JSON | Minimal viable product; proves the architecture |
| Core/engine split in Phase 1, Rust crate at v0.3 (D7) | Split is cheap during scaffolding; deferring the ergonomic crate keeps Phase 1 focused on the benchmark gate |
| Benchmark before expanding | Must prove ≥5x speedup before committing to full Rust codebase |
| Phase 2 = build system features | Highest-value audience (build tools) gets their features first |
| Phase 3 = infrastructure | Server frameworks and CLI tools |
| Phase 4 = power user | Transactions, codecs, watching — complex but lower urgency |
| v1.0 = ecosystem replacement | Full feature parity with fs-extra + glob + tmp + lockfile |
| mitata for benchmarks | Fast, low-overhead benchmarking; works on Node and Bun |
