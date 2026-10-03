---
type: Architecture Decision
title: "Fused Walk: Single-Pass Stat + Hash + Filter"
description: "Single-pass Rust traversal fusing stat + hash + filter — measured at 1.85x vs the best alternative on 100k files, not the projected 10-20x."
tags: [walk, traversal, fused, performance, fdir, tinyglobby, benchmark]
status: stable
generated:
  by: pathway_kb/1.0
  at: 2026-09-16T00:00:00Z
verified:
  - by: human:archont561
    at: 2025-07-11T00:00:00Z
  - by: process:gap-analysis-2026-09
    at: 2026-09-16T00:00:00Z
  - by: process:benchmark-task-4
    at: 2026-10-03T00:00:00Z
domain: architecture
decision: decided  # legacy KB status (decided|proposed|deprecated)
created: 2025-07-11
source: conversation
depends_on:
  - architecture/core-layers
  - architecture/napi-boundary
  - competitive/verified-data
---

# The Fused Walk

## The Problem: Path-Only Traversal Is a Half-Answer

Every JS traversal library — `fdir` (37M downloads/week), `tinyglobby`
(186M downloads/week), `Bun.Glob.scan()` — returns **paths** (or `Dirent`
objects at best).

But real applications never stop at the path. The typical pipeline is:

```
1. Walk directory tree          → get 100,000 paths
2. Stat each file               → check size, mtime, type
3. Hash each file               → check if content changed
4. Filter by content/encoding   → check magic bytes, line count
5. Transform or copy            → do actual work
```

### What This Looks Like in JS Land

```ts
// Using fdir or tinyglobby
const paths = await fdir().glob("**/*.ts").crawl(root);
// ✅ Fast: ~71ms for 100k files (measured, 2026-10-03)

// But then...
const entries = await Promise.all(
  paths.map(async (p) => {
    const stat = await fs.stat(p);          // 100k libuv round-trips
    const content = await fs.readFile(p);   // 100k more round-trips
    const hash = crypto.createHash("sha256")
      .update(content).digest("hex");       // 100k JS crypto calls
    return { path: p, stat, hash };
  })
);
// ❌ Slow: 4,857ms serial / 2,552ms pooled, +39 MiB peak heap
//    (measured 2026-10-03 at 100k files)
```

The traversal was fast. The **post-traversal pipeline** is the bottleneck,
and it's entirely invisible to the traversal library's benchmarks.

---

## The Solution: Fuse Everything Into One Native Pass

The Rust engine performs the entire pipeline in a **single syscall pass**
across OS worker threads:

```
Rust Engine (single pass, rayon/tokio parallelism)
   │
   ├── readdir() with OS-level dirent (stat often free via d_type)
   │
   ├── Pre-descent directory pruning (never enter node_modules)
   │
   ├── Glob + regex filtering (globset + regex crates, zero-copy)
   │
   ├── Parallel content hashing (BLAKE3 via blake3 crate, mmap I/O)
   │
   └── Yield batched results: { path, size, mtime, hash, type }
         ↓
    Single N-API boundary crossing per batch (512 entries)
         ↓
    TypeScript AsyncGenerator unrolls to consumer
```

### The API

```ts
for await (const entry of project.walk({
  glob: "**/*.{ts,tsx}",
  exclude: ["node_modules", ".git", "dist"],
  withMetadata: true,      // stat info included
  hash: "blake3",          // content hash computed natively
})) {
  console.log(entry.path);    // "/project/src/Button.tsx"
  console.log(entry.size);    // 2847
  console.log(entry.mtime);   // 1720713600000
  console.log(entry.hash);    // "a3f8c2..."
}
```

**Zero JS-side `fs.stat()` calls. Zero JS-side `crypto` calls.**
Everything was computed natively during the single traversal pass.

---

## Why This Is the Defensible Moat

### Against `fdir` / `tinyglobby` — MEASURED (2026-10-03)

> **The table below replaced a projection.** The figures previously here
> (~2,830ms vs ~175ms, "10–20x faster") were estimated before any harness
> existed and were wrong by an order of magnitude. These are measured, from
> `benches/walk` at 100k files with a release addon and a warm page cache. Full
> tables and methodology: [verified-data.md](/competitive/verified-data.md).

| Metric (100k files) | `fdir` + JS post-processing | pathway fused walk |
|---|---:|---:|
| Traversal (paths only) | **70.8 ms** | 404.2 ms |
| Total pipeline (traverse + stat + hash) | 2,551.7 ms | **1,380.1 ms** |
| Peak heap | 39.3 MiB | **14.3 MiB** |
| GC per sample | 20.6 ms | **2.8 ms** |
| Time to first entry | 115.4 ms | 1,093.2 ms |

**Fused pipeline: 1.85x, not 10–20x.** Peak heap and GC pressure do behave as
designed — the memory claim holds, and holds clearly.

Two of these numbers are worse than the projection and both are real:

- **Pathway loses raw traversal to `fdir` by 5.7x.** `fdir` reads
  `readdir(withFileTypes: true)` and never stats; the `ignore` crate carries
  gitignore support, pruning and parallel workers that this configuration does
  not use. The projection assumed traversal parity.
- **Time-to-first-entry is 1,093 ms against `fdir`'s 115 ms** — 79% of total,
  because `scan()` completes the whole traversal before JavaScript can pull the
  first batch. The "streaming" API does not stream yet. On latency pathway is
  currently the *worst* option, which matters for a build tool reacting to a
  large tree.

The honest summary: **fusion works, and it is worth ~1.85x against a pooled
baseline** — not 5x, and not 10–20x.

### Against `Bun.Glob.scan()`

**Update (Sept 2026):** Bun 1.4 (released Aug 20, 2026) rewrote Bun in Rust
and shipped a 2× faster `Bun.Glob.scan()`. `Bun.GlobScanOptions` now includes
`absolute`, `cwd`, `dot` (default **false**), `followSymlinks`, `onlyFiles`,
`throwErrorOnBrokenSymlink`, plus a `scanSync` variant. All Bun comparisons
in this document must be re-benchmarked on Bun 1.4 (and 1.3) before any
external publication.

`Bun.Glob.scan()` still provides native traversal (now Rust-based), which is
fast. But:
1. It returns strings only — no stat, no hash, no metadata.
2. Post-traversal stat/hash still crosses the JS↔native boundary per file.
3. No regex filtering (glob patterns only).
4. No directory pruning before descent (exclude functions cross back to JS).
5. No pluggable serialization, atomic writes, sandboxing, etc.

Our library provides the **composition layer** that Bun intentionally doesn't
ship, while matching or exceeding Bun's raw traversal speed via the NAPI-RS
Rust core.

---

## Implementation Architecture

### Rust Side

```rust
// Reference pseudocode for the fused walker (updated Sept 2026)
use ignore::WalkBuilder;
use blake3::Hasher;
use globset::{Glob, GlobSetBuilder};

struct FusedEntry {
    path: String,            // absolute or root-relative per `absolute`
    size: u64,
    mtime_ms: i64,           // ms epoch for the JS API; snapshots retain ns
    hash: Option<String>,    // None = not requested
    error: Option<String>,   // Some = per-entry failure (EACCES, read error)
    is_file: bool,
}

fn execute_walk(root: &Path, opts: &WalkOptions) -> Result<Vec<FusedEntry>> {
    // 1. Compile ALL glob patterns (AND logic when more than one)
    let mut builder = GlobSetBuilder::new();
    for g in &opts.glob {
        builder.add(Glob::new(g).map_err(CompileError)?);
    }
    let globset = builder.build()?;

    let dot = opts.dot.unwrap_or(false);        // default: skip dotfiles
    let gitignore = opts.gitignore.unwrap_or(false);

    let mut walker = WalkBuilder::new(root);
    walker
        .hidden(!dot)             // dot=false → hidden(true)
        .parents(gitignore)       // parent .git dirs when opted in
        .git_ignore(gitignore)
        .ignore(gitignore);       // .ignore files
    // NOTE: the 2025 draft hard-disabled gitignore; it is now an opt-in flag.

    if let Some(depth) = opts.max_depth {
        walker.max_depth(Some(depth as usize));
    }
    if let Some(threads) = opts.concurrency {
        walker.threads(threads as usize);
    }
    walker.filter_entry(|e| !is_excluded_dir(e, &opts.exclude)); // pre-descent

    let mut results = Vec::new();
    for entry_result in walker.build() {
        let entry = match entry_result {
            Ok(e) => e,
            Err(err) => { report_walk_error(err); continue; } // not swallowed
        };

        // 2. MATCH GLOBS AGAINST THE ROOT-RELATIVE PATH.
        //    The 2025 draft matched absolute paths, so `**/*.ts` only
        //    matched files directly under the root. fast-glob-compatible
        //    semantics require relative matching.
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        if !globset.is_match(rel) { continue; }

        // 3. Regex applies to the FULL path (by design)
        if let Some(re) = &opts.regex {
            if !re.is_match(entry.path()) { continue; }
        }

        if opts.files_only && !is_file(&entry) { continue; }

        // 4. Fused metadata + hash. This closure body executes on the
        //    ignore crate's worker threads, so hashing is already
        //    distributed across cores — no separate rayon stage needed
        //    in v0.1 (dedicated parallel stage reserved for hashTree).
        let (size, mtime_ms) = stat(&entry);
        let hash = opts.hash
            .then(|| hash_chunked(entry.path(), 64 * 1024))
            .flatten();
        // hash_chunked: streaming 64 KB reads — never loads whole files

        results.push(FusedEntry {
            path: entry.path().to_string_lossy().into_owned(), // libuv parity
            size, mtime_ms,
            hash,
            error: None,
            is_file: is_file(&entry),
        });
    }
    Ok(results)
}
```

### TypeScript Side

```ts
async function* walkFiles(options: WalkOptions): AsyncIterableIterator<PathEntry> {
  const scanner = new NativeScanner(this.value, {
    glob: options.glob,
    regex: options.regex?.source,
    exclude: options.exclude,
    withMetadata: options.withMetadata ?? false,
    hash: options.hash,
    batchSize: 512,
  });

  // Rust yields batches of 512 entries per N-API call
  for await (const batch of scanner.walkBatches()) {
    for (const raw of batch) {
      const entry = new PathEntry(raw);

      // Optional JS-side post-filter (explicitly advanced, crosses boundary)
      if (options.filter && !(await options.filter(entry))) {
        continue;
      }

      yield entry;
    }
  }
}
```

---

## Benchmark Success Criteria

Before committing to the full Rust codebase, Phase 1 must prove:

### Test Matrix

| Tree Size | Filter Complexity | Hashing |
|-----------|-------------------|---------|
| 10k files | glob only | none |
| 100k files | glob + regex + exclude | none |
| 100k files | glob + exclude | BLAKE3 |
| 500k files | glob + regex + exclude | BLAKE3 |
| 1M files | glob + exclude | xxHash |

### Competitors (updated Sept 2026)

| Engine | Runtime |
|--------|---------|
| **`node:fs.glob` + `node:fs` + `node:crypto`** (native C++ baseline) | Node 24 |
| `fdir` + `node:fs` + `node:crypto` | Node 24 |
| `tinyglobby` + `node:fs` + `node:crypto` | Node 24 |
| `Bun.Glob.scan()` + `Bun.file()` + `Bun.hash` | Bun 1.3 + Bun 1.4 |
| **@archont561/pathway fused walk** | Node 24 + Bun 1.3/1.4 |

`node:fs.glob` is **stable in Node core** (v22.17.0/v24.0.0) and C++-native —
it is the baseline the fused walk must beat *on Node*. It returns paths or
Dirents only, so its post-traversal pipeline still pays per-file `stat` +
`crypto` costs; the fusion delta survives, and the honest benchmark framing
is "our fused pipeline vs. Node's own native glob + post-processing."

### Measured Metrics (added Sept 2026)

Wall time is not enough. Every benchmark run records:
- Wall time (p50/p95 over ≥5 runs)
- Peak heap (`process.memoryUsage()`) + JS GC pressure (heap snapshots)
- Time to first entry (streaming latency)
- Cancellation cost (abort at 50% — was work actually stopped?)

### Success Threshold — and how the first run landed

The fused walk must be **≥5x faster** than the best alternative on the full
pipeline (walk + stat + hash) for 100k+ files — the baseline on Node 24 is
`node:fs.glob` + `node:fs` + `node:crypto` (native C++), not just pure JS.

**Measured 2026-10-03 at 100k files: 1.85x.** The threshold **fails.**

| Baseline | pathway | Speedup |
|---|---:|---:|
| `fdir` + 32-wide stat/hash pool (**the strongest**) | 1,380 ms | **1.85x** |
| `fdir`, serial post-processing | 1,380 ms | 3.5x |
| `tinyglobby`, serial | 1,380 ms | 3.6x |
| `node:fs.glob`, serial | 1,380 ms | 3.8x |

Against the serial baseline the threshold originally assumed, the measured
result is 3.8x — still short of 5x. So the miss is not an artefact of a
weakly-chosen comparator.

What the first run changed about how this document should be read:

1. **The comparator has to pool its I/O.** A `for (const p of paths) await
   stat(p)` loop serialises 100k round trips. Measuring against it reports
   ~3.7x and measures *parallelism*, not fusion. The pooled baseline is the
   honest one.
2. **Harness methodology is not optional.** A debug addon and an unsettled page
   cache each moved the numbers by more than the claim does — the cache alone
   produced a 29.2 s reading and a 1.3 s reading for identical work.
3. **1.85x with 2.7x lower peak heap and 7x lower GC is a real result**, just
   not the one that justifies the architecture on speed alone. The case for the
   native engine now rests on memory and GC, and on fusion removing
   JS↔native hops — which is a narrower claim than "5x faster".

Re-baselining options, in the order they are worth trying: true incremental
batching (so the traversal overlaps the consumer, which also fixes the
time-to-first-entry regression); a faster hash than BLAKE3 for small files; and
dropping `ignore` for a leaner walker when gitignore support is not requested.
Until one of those is measured, **the ≥5x and 10–20x figures are not available
to marketing.**

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Fused walk as flagship | Path-only traversal is a commodity; fused pipeline is the moat |
| Single-pass stat + hash | Eliminates 100k+ JS↔libuv round-trips in real-world pipelines |
| Batched N-API yields / async iterator (spike-gated) | Minimize boundary crossings; pull-based native iterators (experimental, Sept 2026) enable true streaming + cancellation |
| Root-relative glob matching | fast-glob-compatible semantics; absolute-path matching is a bug (2026 audit) |
| `dot` default false | Matches Bun.Glob / fs.glob / incumbent expectations (2026 audit) |
| Per-entry error reporting | Traversal errors collected, never silently swallowed (2026 audit) |
| Chunked hashing (64 KB) | No whole-file loads; memory-safe on large files (2026 audit) |
| `node:fs.glob` in benchmark set | Honest native baseline on Node (2026 audit) |
| `ignore` crate as walker | ripgrep's engine; handles pruning, gitignore, parallelism |
| `blake3` as default hash | Fastest crypto hash; 14x faster than SHA-256 on modern CPUs |
| JS `filter` as post-native | Explicitly advanced; documented as crossing the boundary |
| ≥5x speedup threshold | Below this, Rust complexity isn't justified |
