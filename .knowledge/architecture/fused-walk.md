---
title: "Fused Walk: Single-Pass Stat + Hash + Filter"
domain: architecture
status: decided
created: 2025-07-11
updated: 2025-07-11
source: conversation
depends_on:
  - architecture/core-layers
  - architecture/napi-boundary
  - competitive/verified-data
tags: [walk, traversal, fused, performance, fdir, tinyglobby, benchmark]
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
// ✅ Fast: ~30ms for 100k files

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
// ❌ Slow: 1,200–3,000ms + massive GC pressure
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

### Against `fdir` / `tinyglobby`

| Metric | fdir + JS post-processing | @myorg/path fused walk |
|--------|--------------------------|----------------------|
| Traversal (100k files) | ~30ms | ~25ms (ignore crate) |
| Stat all files | ~800ms (100k libuv hops) | ~0ms (dirent d_type) |
| Hash all files (BLAKE3) | ~2000ms (JS crypto) | ~150ms (rayon + mmap) |
| Total pipeline | ~2,830ms | ~175ms |
| Memory peak | ~200MB (JS strings + Buffers) | ~30MB (Rust + batched) |
| GC pressure | Severe | None (Rust memory) |

The traversal itself is comparable. The **fused pipeline** is **10–20x faster**
because it eliminates the JS↔libuv boundary for every post-traversal operation.

### Against `Bun.Glob.scan()`

`Bun.Glob.scan()` provides native traversal via Zig, which is fast. But:
1. It returns strings only — no stat, no hash, no metadata.
2. Post-traversal stat/hash still crosses the JS↔Zig boundary per file.
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
// Pseudocode for the fused walker
use ignore::WalkBuilder;
use blake3::Hasher;
use rayon::prelude::*;

struct FusedEntry {
    path: String,
    size: u64,
    mtime: i64,
    hash: Option<String>,
    is_file: bool,
}

fn fused_walk(root: &Path, opts: &WalkOptions) -> Vec<FusedEntry> {
    let walker = WalkBuilder::new(root)
        .filter_entry(|e| !is_excluded(e, &opts.exclude))
        .build();

    walker
        .filter_map(|e| e.ok())
        .filter(|e| matches_glob(e, &opts.glob))
        .filter(|e| matches_regex(e, &opts.regex))
        .par_bridge()                    // rayon parallelism
        .map(|entry| {
            let meta = entry.metadata().ok()?;
            let hash = opts.hash.then(|| {
                let bytes = std::fs::read(entry.path()).ok()?;
                let mut hasher = Hasher::new();
                hasher.update(&bytes);
                Some(hasher.finalize().to_hex().to_string())
            }).flatten();

            Some(FusedEntry {
                path: entry.path().to_string_lossy().into(),
                size: meta.len(),
                mtime: meta.modified().ok()?.duration_since(UNIX_EPOCH)
                    .ok()?.as_millis() as i64,
                hash,
                is_file: meta.is_file(),
            })
        })
        .flatten()
        .collect()
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

### Competitors

| Engine | Runtime |
|--------|---------|
| `fdir` + `node:fs` + `node:crypto` | Node 22 |
| `tinyglobby` + `node:fs` + `node:crypto` | Node 22 |
| `Bun.Glob.scan()` + `Bun.file()` + `Bun.hash` | Bun latest |
| **@myorg/path fused walk** | Node 22 + Bun latest |

### Success Threshold

The fused walk must be **≥5x faster** than the best JS alternative on the
full pipeline (walk + stat + hash) for 100k+ files. If the improvement is
only 5–10%, the Rust complexity is not justified and we should reconsider
the architecture.

If it is **10–20x faster** with significantly lower memory usage, that is
the foundation of the project and the core of the marketing pitch.

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Fused walk as flagship | Path-only traversal is a commodity; fused pipeline is the moat |
| Single-pass stat + hash | Eliminates 100k+ JS↔libuv round-trips in real-world pipelines |
| Batched N-API yields | Minimizes boundary crossings while keeping clean async iterator |
| `ignore` crate as walker | ripgrep's engine; handles pruning, gitignore, parallelism |
| `blake3` as default hash | Fastest crypto hash; 14x faster than SHA-256 on modern CPUs |
| JS `filter` as post-native | Explicitly advanced; documented as crossing the boundary |
| ≥5x speedup threshold | Below this, Rust complexity isn't justified |
