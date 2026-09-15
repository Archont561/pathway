---
title: "Walk Engine: Glob, Regex, Exclude, Pruning, Predicates"
domain: features
status: decided
created: 2025-07-11
updated: 2025-07-11
source: conversation
depends_on:
  - architecture/core-layers
  - architecture/fused-walk
  - competitive/verified-data
tags: [walk, traversal, glob, regex, exclude, pruning, predicate, async-iterator]
---

# Walk & Traversal Engine

## Overview

`walkFiles()` is the **flagship feature** of `@myorg/path`. It is the primary
justification for the native Rust dependency and the core of the fused-walk
architecture (see [fused-walk.md](../architecture/fused-walk.md)).

The API must feel effortless while the complexity lives entirely underneath:

```ts
for await (const file of project.walkFiles({
  glob: "**/*.{ts,tsx}",
  exclude: ["node_modules", ".git", "dist"],
})) {
  console.log(file.relativeTo(project));
}
```

---

## The Two First-Class Matchers

### Glob and Regex Are Equal Citizens

Both glob and regex are supported as top-level options. They are **not**
interchangeable aliases — they have different semantics and different
performance profiles, and the engine treats them distinctly.

```ts
// Glob matching (globset crate, compiled once, tested per-path)
project.walkFiles({ glob: "**/*.test.ts" });

// Regex matching (regex crate, compiled once, tested per-path)
project.walkFiles({ regex: /\.test\.ts$/ });

// Both simultaneously (AND logic — file must match both)
project.walkFiles({
  glob: "**/*.ts",
  regex: /(?<!\.d)\.ts$/,   // .ts but not .d.ts
});
```

### Internal Representation

The Rust engine maintains a tagged enum, not a unified "pattern" abstraction:

```rust
enum Matcher {
    Glob(GlobMatcher),    // globset::GlobSet — compiled glob patterns
    Regex(RegexMatcher),  // regex::Regex — compiled regular expression
    Both {
        glob: GlobMatcher,
        regex: RegexMatcher,
    },
    None,
}
```

This matters because:

1. **Glob matching** is prefix-aware and can short-circuit on directory
   components. `**/node_modules/**` can prune entire subtrees before
   descending.
2. **Regex matching** operates on the full path string and cannot prune
   subtrees. It is applied per-file after the walker reaches the entry.
3. Combining them uses AND logic: the glob narrows the search space (and
   potentially prunes directories), then the regex applies a secondary
   filter on the surviving paths.

### When to Use Which

| Scenario | Matcher | Why |
|----------|---------|-----|
| File extensions | `glob: "**/*.{ts,tsx}"` | Natural syntax, directory-prunable |
| Naming conventions | `regex: /\.test\.[jt]sx?$/` | Easier than glob for complex suffixes |
| Excluding patterns | `regex: /(?<!\.d)\.ts$/` | Negative lookbehind impossible in glob |
| Directory subtrees | `glob: "packages/*/src/**"` | Glob understands directory structure |
| Content-aware | Neither — use `filter` | See Predicates section below |

---

## Directory Exclusion & Pruning

### The Critical Distinction: Pruning vs Post-Filtering

This is one of the most important design decisions in the walker:

```ts
exclude: ["node_modules", ".git"]
```

This does **NOT** mean:
> "Find everything, then remove matches from the results."

This means:
> **"Never enter these directories during traversal."**

The distinction is enormous for performance:

| Strategy | `node_modules` with 80k files |
|----------|-------------------------------|
| Post-filter | Walk 80k entries → match → discard. ~200ms wasted. |
| Pre-descent prune | See `node_modules/` → skip. ~0.01ms. |

### Implementation

The `ignore` crate (ripgrep's walker) provides this natively via
`filter_entry()`:

```rust
walker.filter_entry(move |entry| {
    if entry.file_type().map_or(false, |ft| ft.is_dir()) {
        if let Some(name) = entry.file_name().to_str() {
            // Prune: never descend into excluded directories
            return !exclude_set.contains(name);
        }
    }
    true
});
```

### Exclusion Semantics

Exclusion patterns match against **directory names**, not full paths:

```ts
// ✅ Excludes ANY directory named "dist" at any depth
exclude: ["dist"]

// ✅ Excludes "node_modules" and ".git" everywhere
exclude: ["node_modules", ".git"]

// ❌ NOT supported: path-based exclusion like "packages/foo/dist"
//    Use a glob filter or JS predicate for path-specific exclusion
```

This keeps the pruning logic fast (hash-set lookup on directory name) and
covers 95% of real-world use cases. Path-specific exclusion is handled by
the `filter` predicate (see below).

### `exclude` vs `ignore` (Considered and Resolved)

Early design considered two separate concepts:

```ts
exclude: [...]  // Never descend
ignore: [...]   // Descend but don't yield
```

**Decision: Single `exclude` with documented pre-descent semantics.**

Rationale:
1. Two overlapping concepts create confusion ("why is my file missing?").
2. The "descend but don't yield" case is vanishingly rare — if you don't
   want the files, why pay the I/O cost of entering the directory?
3. The rare case is covered by `filter` predicates.
4. Simpler API = faster adoption.

---

## Predicates (JS-Side Post-Native Filter)

### The Problem

Glob and regex operate on **path strings**. Many real-world filters need
**file metadata or content**:

- "Only files smaller than 1MB"
- "Only files modified in the last 24 hours"
- "Only files containing a specific import"
- "Only files that pass a custom validation"

### The API

```ts
await project.walkFiles({
  glob: "**/*.json",

  filter: async (entry) => {
    return entry.size < 1_000_000;
  },
});
```

### The Architecture: Post-Native Filter

Predicates are **explicitly a JS-side post-filter**. The execution order is:

```
Rust traversal
    ↓
Cheap native filters (glob, regex, exclude pruning)
    ↓
Batch yield across N-API boundary
    ↓
JS filter predicate (per-file, may be async)
    ↓
Consumer callback / yield
```

This is a deliberate trade-off:

| Aspect | Native filter | JS predicate |
|--------|--------------|--------------|
| Speed | Fast (no FFI) | Slow (crosses boundary per file) |
| Flexibility | Path-only | Arbitrary logic, async, content-aware |
| Use case | 90% of filters | 10% advanced cases |

The JS predicate is acceptable because:
1. It runs **after** the native filters have already pruned the search space.
   If the native filters reduce 100k files to 500, the JS predicate only
   crosses the boundary 500 times.
2. It is **explicitly opt-in**. Users who don't provide a `filter` callback
   get zero JS-side overhead.
3. It is **documented** as an advanced feature with known performance
   implications.

### Predicate Signature

```ts
type WalkFilter = (entry: PathEntry) => boolean | Promise<boolean>;

interface PathEntry {
  readonly path: Path;
  readonly size: number;
  readonly mtime: Date;
  readonly isFile: boolean;
  readonly isDirectory: boolean;
  readonly isSymlink: boolean;
  readonly hash?: string;       // Only if hash option was set
  relativeTo(base: Path): string;
}
```

### Example: Content-Aware Filter

```ts
for await (const file of project.walkFiles({
  glob: "**/*.ts",
  exclude: ["node_modules"],
  withMetadata: true,

  filter: async (entry) => {
    // Skip large files without reading them
    if (entry.size > 500_000) return false;

    // Read first 200 bytes to check for generated file marker
    const head = await entry.path.readBytes(200);
    return !head.toString().includes("@generated");
  },
})) {
  // Only hand-written, non-generated .ts files under 500KB
  console.log(file.path.value);
}
```

---

## Async Iterator Protocol

### The Consumer API

```ts
// Standard async iteration
for await (const file of project.walkFiles(options)) {
  console.log(file);
}

// Manual iteration
const walker = project.walkFiles(options);
const first = await walker.next();
if (!first.done) {
  console.log(first.value);
}

// Collect all (convenience, loads into memory)
const allFiles = await project.walkFiles(options).toArray();
```

### The Internal Chunking

The consumer sees a per-file async iterator. Internally, the engine works
in chunks to minimize N-API boundary crossings:

```ts
class WalkIterator implements AsyncIterableIterator<PathEntry> {
  private batch: PathEntry[] = [];
  private index = 0;
  private exhausted = false;

  async next(): Promise<IteratorResult<PathEntry>> {
    // Refill batch when empty
    if (this.index >= this.batch.length) {
      if (this.exhausted) return { done: true, value: undefined };

      this.batch = await this.scanner.nextBatch();  // N-API call
      this.index = 0;

      if (this.batch.length === 0) {
        this.exhausted = true;
        return { done: true, value: undefined };
      }
    }

    const entry = this.batch[this.index++];

    // Apply JS predicate if present
    if (this.options.filter && !(await this.options.filter(entry))) {
      return this.next();  // Skip and recurse
    }

    return { done: false, value: entry };
  }

  [Symbol.asyncIterator]() {
    return this;
  }
}
```

### Batch Size Tuning

| Batch Size | N-API Crossings (100k files) | Memory per Batch | Latency per Yield |
|-----------|------------------------------|-----------------|-------------------|
| 64 | ~1,563 | ~16KB | Low |
| 256 | ~391 | ~64KB | Low |
| **512** | **~196** | **~128KB** | **Balanced** |
| 1024 | ~98 | ~256KB | Higher initial |
| 4096 | ~25 | ~1MB | High initial |

**Default: 512.** Tunable via `batchSize` option for advanced users.

---

## Walk Options Reference

```ts
interface WalkOptions {
  // Matching (at least one recommended)
  glob?: string | string[];       // Glob pattern(s), AND logic if array
  regex?: RegExp;                 // Regex pattern, AND with glob if both set

  // Pruning
  exclude?: string[];             // Directory names to never enter

  // Metadata
  withMetadata?: boolean;         // Include stat info (default: false)
  hash?: "blake3" | "sha256" | "xxhash";  // Compute content hash
  maxDepth?: number;              // Maximum directory depth

  // Filtering
  filesOnly?: boolean;            // Yield only files (default: true)
  filter?: WalkFilter;            // JS predicate (post-native, may be async)

  // Performance
  batchSize?: number;             // N-API batch size (default: 512)
  concurrency?: number;           // Rust worker threads (default: CPU count)
}
```

---

## Walk Variants

### `walkFiles()` — Files Only (Default)

```ts
for await (const file of project.walkFiles({ glob: "**/*.ts" })) {
  // file.isFile === true always
}
```

### `walkDirs()` — Directories Only

```ts
for await (const dir of project.walkDirs({ exclude: ["node_modules"] })) {
  // dir.isDirectory === true always
}
```

### `walk()` — Everything

```ts
for await (const entry of project.walk({ withMetadata: true })) {
  if (entry.isFile) { /* ... */ }
  if (entry.isDirectory) { /* ... */ }
}
```

### `walkSync()` — Synchronous (Escape Hatch)

```ts
for (const file of project.walkSync({ glob: "**/*.ts" })) {
  // Blocks the event loop. Use only in CLI startup or scripts.
}
```

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Glob + regex as equal citizens | Different semantics, different strengths, both needed |
| `Matcher` enum internally | No false abstraction; each matcher type optimized separately |
| `exclude` = pre-descent prune | 1000x faster than post-filter for large directories |
| Single `exclude`, no `ignore` | Simpler API; rare "descend but skip" case covered by `filter` |
| JS `filter` as post-native | Explicit trade-off: flexibility at cost of per-file FFI |
| Batched async iterator | Clean per-file API with minimal N-API crossings |
| Default batch size 512 | Balanced memory/latency; tunable |
| `withMetadata` opt-in | Avoid stat overhead when only paths are needed |
| `hash` opt-in | Avoid I/O overhead when hashing isn't needed |
