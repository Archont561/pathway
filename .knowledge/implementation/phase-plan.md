---
title: "Phase 1–4 Implementation Plan, v0.1–v1.0 Roadmap, Benchmark Harness"
domain: implementation
status: decided
created: 2025-07-11
updated: 2025-07-11
source: conversation
depends_on:
  - architecture/fused-walk
  - features/walk-traversal
  - features/serializers
  - features/killer-features
  - competitive/positioning
  - implementation/repo-structure
tags: [phase, roadmap, benchmark, v0.1, v1.0, plan]
---

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

#### Step 1.1: Project Scaffolding (Week 1)
- [ ] Initialize Cargo workspace with `crates/engine`
- [ ] Initialize pnpm workspace with `packages/path`
- [ ] Configure NAPI-RS v3 with `tokio` async support
- [ ] Set up TypeScript build (tsc, vitest)
- [ ] Verify "hello world" NAPI-RS binding compiles and loads on Node + Bun
- [ ] Set up GitHub Actions CI skeleton (see [ci-distribution.md](./ci-distribution.md))

#### Step 1.2: Rust Traversal Engine (Weeks 2–3)
- [ ] Implement `NativeScanner` using `ignore` crate
- [ ] Add `globset` glob matching
- [ ] Add `regex` filtering
- [ ] Implement pre-descent directory exclusion (pruning)
- [ ] Implement batched result yielding (default batch size: 512)
- [ ] Add `withMetadata` option (stat info via `DirEntry::metadata`)
- [ ] Add BLAKE3 content hashing via `blake3` crate
- [ ] Write Rust unit tests for all walker configurations

**See:** [code-rust-walker.md](./code-rust-walker.md) for the implementation.

#### Step 1.3: TypeScript API (Weeks 3–4)
- [ ] Implement `Path` class with `pathe` for string ops
- [ ] Implement `Path.cwd()`, `join()`, `relativeTo()`
- [ ] Implement `readText()`, `writeText()`, `writeTextAtomic()`
- [ ] Implement `Serializer<T>` interface and built-in `json` serializer
- [ ] Implement `read<T>(serializer)` and `write<T>(serializer, data)`
- [ ] Implement `WalkIterator` async generator with batch consumption
- [ ] Implement `walkFiles()`, `walkDirs()`, `walk()`
- [ ] Write TypeScript tests for all Path operations

**See:** [code-ts-path.md](./code-ts-path.md) for the implementation.

#### Step 1.4: Benchmark Harness (Week 4)
- [ ] Create file tree generator (10k, 100k, 500k, 1M files)
- [ ] Benchmark A: Raw traversal (paths only)
- [ ] Benchmark B: Traversal + complex exclusion
- [ ] Benchmark C: Fused walk (traverse + stat + hash)
- [ ] Compare against: `fdir`, `tinyglobby`, `Bun.Glob.scan()`
- [ ] Document results and determine if ≥5x speedup is achieved

**See:** Benchmark section below for the full test plan.

### Success Criteria
- [ ] Fused walk ≥5x faster than best JS alternative on 100k+ files
- [ ] All tests pass on Node 22, Node 24, Bun latest
- [ ] All tests pass on Linux (glibc), macOS (arm64), Windows (x64)
- [ ] Package installs and loads correctly via NAPI-RS platform binaries
- [ ] API matches the design in [walk-traversal.md](../features/walk-traversal.md)

---

## Phase 2: Build System Features (v0.2)

### Scope
Temp dirs, content hashing, directory snapshots, additional hashers.

### Deliverables
- [ ] `Path.temp(callback)` — RAII temp directories via `tempfile` crate
- [ ] `file.hash(hasher)` — Single-file content hashing
- [ ] `project.hashTree(options)` — Parallel tree hashing via rayon
- [ ] `Hasher` interface with `blake3`, `xxhash`, `sha256` implementations
- [ ] `project.snapshot(options)` — Directory snapshot with stat + hash
- [ ] `Snapshot.diff(other)` — Added/removed/modified/unchanged
- [ ] `Snapshot.save()` / `Snapshot.load()` — Persistence for incremental builds

### Success Criteria
- [ ] Snapshot + diff on 100k files completes in <500ms
- [ ] Temp dir cleanup verified on process crash (SIGKILL test)
- [ ] Tree hash is deterministic across runs

---

## Phase 3: Infrastructure Features (v0.3)

### Scope
Sandbox, file locking, parallel bulk ops, transformers.

### Deliverables
- [ ] `project.sandbox(subdir)` — Path containment with symlink resolution
- [ ] `file.withLock(callback)` — Native `flock()` / `LockFileEx()`
- [ ] `project.copyTo(dest, options)` — Parallel recursive copy
- [ ] `project.transform(dest, options)` — Parallel file transformation
- [ ] `Transformer` interface with `gzip`, `brotli` implementations
- [ ] `ContainmentError` with type-level `SandboxedPath` brand

### Success Criteria
- [ ] Sandbox blocks all `../` traversal attempts (including symlinks)
- [ ] File locking verified under concurrent access (10 processes)
- [ ] Parallel copy of 50k files ≥3x faster than `fs-extra.copy()`

---

## Phase 4: Power User Features (v0.4 → v1.0)

### Scope
Transactions, native codecs, detection, resolution, watching.

### Deliverables
- [ ] `Path.transaction(callback)` — Best-effort transactional operations
- [ ] `@myorg/path-toml` — Native TOML serializer via `toml-rs`
- [ ] `@myorg/path-yaml` — Native YAML serializer via `serde_yaml`
- [ ] `Detector` interface with `mime`, `encoding` implementations
- [ ] `Validator` interface with Zod/Valibot integration examples
- [ ] `Resolver` interface with Node and tsconfig resolution
- [ ] `project.watch()` — Native filesystem watching via `notify` crate
- [ ] `FileSystem.create({ serializers })` — Per-instance registry
- [ ] Full CI matrix with all 7 platform targets

### v1.0 Success Criteria
- [ ] All features from [killer-features.md](../features/killer-features.md) shipped
- [ ] All pluggable patterns from [pluggable-patterns.md](../features/pluggable-patterns.md) implemented
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
import { Path } from "@myorg/path";
import { fdir } from "fdir";
import { glob } from "tinyglobby";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";

const ROOT = process.argv[2] || "./fixtures/100k";

group("Raw Traversal (paths only)", () => {
  bench("fdir", async () => {
    return new fdir().glob("**/*.ts").crawl(ROOT);
  });

  bench("tinyglobby", async () => {
    return glob("**/*.ts", { cwd: ROOT });
  });

  bench("@myorg/path (native)", async () => {
    const results: string[] = [];
    for await (const f of new Path(ROOT).walkFiles({ glob: "**/*.ts" })) {
      results.push(f.value);
    }
    return results;
  });
});

group("Fused Walk (traverse + stat + hash)", () => {
  bench("fdir + fs.stat + crypto", async () => {
    const paths = await new fdir().glob("**/*.ts").crawl(ROOT);
    return Promise.all(paths.map(async (p: string) => {
      const content = await readFile(p);
      const hash = createHash("sha256").update(content).digest("hex");
      return { path: p, hash };
    }));
  });

  bench("@myorg/path fused (native)", async () => {
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
best JS alternative. If the improvement is only 5–10%, the Rust complexity
is not justified. If it is 10–20x faster with lower memory, that is the
foundation of the project.

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Phase 1 = walker + Path + JSON | Minimal viable product; proves the architecture |
| Benchmark before expanding | Must prove ≥5x speedup before committing to full Rust codebase |
| Phase 2 = build system features | Highest-value audience (build tools) gets their features first |
| Phase 3 = infrastructure | Server frameworks and CLI tools |
| Phase 4 = power user | Transactions, codecs, watching — complex but lower urgency |
| v1.0 = ecosystem replacement | Full feature parity with fs-extra + glob + tmp + lockfile |
| mitata for benchmarks | Fast, low-overhead benchmarking; works on Node and Bun |
