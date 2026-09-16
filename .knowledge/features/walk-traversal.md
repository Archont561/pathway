---
title: "Walk Engine: Glob, Regex, Exclude, Pruning, Predicates"
domain: features
status: decided
created: 2025-07-11
updated: 2026-09-16
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

### Pattern Semantics (added Sept 2026)

**Glob patterns are matched against paths relative to the walk root**
(fast-glob-compatible). `**/*.ts` matches `src/a.ts`, `a/b/c.ts`, and
`x.ts` alike.

> ⚠️ The 2025 draft matched globs against **absolute** paths. Glob patterns
> are anchored and `*` does not cross `/`, so `**/*.ts` against
> `/repo/src/a.ts` matched only root-level files. This is fixed and covered
> by a dedicated unit-test matrix (nested, root-level, `*.ts` basename,
> Windows separators).

**Regex is matched against the full absolute path** (unchanged from the
original design).

**Glob arrays use AND logic:** `glob: ["**/*.ts", "!**/generated/**"]` —
when passed as an array, an entry must match **all** patterns (negation via
`!` prefix, like globby). A single string is a single pattern.

### Internal Representation

The Rust engine maintains a tagged enum, not a unified "pattern" abstraction:

```rust
enum Matcher {
    Glob(GlobSet),        // globset::GlobSet — ALL patterns compiled together
    Regex(RegexMatcher),  // regex::Regex — compiled regular expression
    Both {
        glob: GlobSet,
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

**Non-UTF-8 directory names:** `file_name().to_str()` is `None` for them, so
pruning is silently skipped (we descend). This matches libuv's lossy
behavior and is documented, not an error.

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

### Dotfiles & `.gitignore` (added Sept 2026)

Two opt-in controls that the 2025 draft lacked:

```ts
// dot (default: false) — skip dotfiles/dotdirs, like every glob incumbent
project.walkFiles({ dot: true });            // include .env, .github, …

// gitignore (default: false) — honor .gitignore/.ignore + parent .git dirs
project.walkFiles({ gitignore: true });
```

- **`dot: boolean` (default `false`)** maps to the `ignore` crate's
  `hidden(!dot)`. The 2025 draft had **no** `dot` option and included
  hidden files unconditionally — the opposite of Bun.Glob, `node:fs.glob`,
  fast-glob, and globby, all of which default to skipping dotfiles. With
  `dot: false`, walking a repo no longer silently enters `.git`.
- **`gitignore: boolean` (default `false`)** enables the `ignore` crate's
  native gitignore support (`git_ignore(true)` + `parents(true)` +
  `.ignore` files). `globby` ships a `gitignore: true` option; build tools
  regularly filter by git status. The 2025 draft hard-disabled it.

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
  readonly mtime: Date | null;      // null when withMetadata is false
  readonly isFile: boolean;
  readonly isDirectory: boolean;
  readonly isSymlink: boolean;
  readonly hash?: string;           // undefined = not requested
  readonly error?: string;          // set when the entry could not be fully read
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
    const head = await entry.path.readBytes(0, 200);
    return !head.toString().includes("@generated");
  },
})) {
  // Only hand-written, non-generated .ts files under 500KB
  console.log(file.path.value);
}
```

---

## Cancellation & Error Reporting (added Sept 2026)

The 2025 draft swallowed every traversal error (`Err(_) => continue`) and
had no way to stop a long walk. Both are fixed:

### `signal?: AbortSignal`

```ts
const controller = new AbortController();
const iter = project.walkFiles({ glob: "**/*", hash: "blake3",
  signal: controller.signal });

for await (const file of iter) {
  if (done()) break;      // early break
}
// The walk actually stops:
// - native async iterator: for-await exit calls return(), which cancels the
//   Rust producer (napi.rs)
// - paging fallback: the signal handler calls scanner.cancel() (AtomicBool
//   checked per entry in the Rust loop)
```

Long walks on million-file trees in CLIs must be interruptible — this is
table stakes for build-tool adoption.

### Per-Entry Errors

- Unreadable entries (EACCES, EMFILE, broken symlinks…) are **collected**,
  not dropped. `PathEntry.error` carries the per-entry failure; a global
  `errors` report is available after the walk completes.
- `hash` is three-state: `undefined` = not requested, a hex string =
  success, `entry.error` set = requested but failed. An empty hash string
  can never appear.
- `throwOnError?: boolean` (default `false`): when true, the first
  collected error is thrown at the end of the walk instead of reported.

### Progress

`onProgress?: (p: { scanned: number; emitted: number; bytesHashed: number }) => void`
— counted per batch in the TS layer (no native callback needed). Build
tools display this; monorepo teams expect it.

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

> **2026 note:** Phase 1 Step 0 spikes the experimental NAPI-RS
> `#[napi(async_iterator)]` (native `AsyncGenerator`). If it passes the
> runtime matrix, this class is replaced by the native pull-based iterator
> and batches become a prefetch window inside Rust. See
> [napi-boundary.md](../architecture/napi-boundary.md) for the decision.

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
  glob?: string | string[];   // Glob pattern(s); AND logic if array;
                              // matched RELATIVE to the walk root
  regex?: RegExp;             // Regex; matched against the FULL absolute path

  // Pruning
  exclude?: string[];         // Directory names to never enter (pre-descent)
  dot?: boolean;              // Include dotfiles/dotdirs (default: false)
  gitignore?: boolean;        // Honor .gitignore/.ignore + parents (default: false)

  // Results
  absolute?: boolean;         // Return absolute paths (default: false → root-relative)
  maxDepth?: number;          // Maximum directory depth below the root
  filesOnly?: boolean;        // Yield only files (default: true)
  withMetadata?: boolean;     // Include stat info (default: false)
  hash?: "blake3" | "sha256" | "xxhash";  // Compute content hash (chunked I/O)

  // Filtering
  filter?: WalkFilter;        // JS predicate (post-native, may be async)

  // Control
  signal?: AbortSignal;       // Cancellation
  onProgress?: (p: { scanned: number; emitted: number; bytesHashed: number }) => void;
  throwOnError?: boolean;     // Throw collected errors at end (default: false)

  // Performance
  batchSize?: number;         // N-API batch size (default: 512)
  concurrency?: number;       // Rust worker threads (default: CPU count)
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

Implementation: a dedicated walk with `filesOnly: false` that yields only
entries where `isDirectory === true`. (The 2025 draft passed `filesOnly:
false` **without** the directory filter — it yielded files too. Fixed.)

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

Requires a **synchronous** N-API scan (blocking call, no promise). Document
the event-loop cost; intended for CLI cold starts where the process is
single-shot.

---

## Ordering, Determinism & Encoding Policies (added Sept 2026)

1. **Raw walk order is unspecified.** The `ignore` crate traverses in
   parallel; yield order varies by thread scheduling (same as fdir /
   tinyglobby consumers must already handle).
2. **`hashTree()` and `snapshot()` fold entries in sorted (path) order** so
   their outputs are deterministic across runs — required for build-cache
   keys.
3. **Non-UTF-8 filenames** are lossy-converted to U+FFFD, matching
   libuv/Node's own path representation. Round-trip (create → walk → open)
   is covered by tests. Exclusion pruning is skipped for non-UTF-8 dir
   names (documented above).
4. **Case sensitivity** follows the OS: glob matching is case-insensitive
   on case-insensitive volumes (macOS/Windows defaults), case-sensitive
   elsewhere — mirroring `std::path` behavior, like fast-glob's default.
5. **mtime** is exposed in ms (JS `Date`); internal snapshot storage keeps
   full nanosecond precision (see [killer-features.md](./killer-features.md)
   §2) because build systems write files within the same millisecond.

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Glob + regex as equal citizens | Different semantics, different strengths, both needed |
| `Matcher` enum internally | No false abstraction; each matcher type optimized separately |
| **Globs matched root-relative** | fast-glob-compatible; absolute matching is a bug (2026 audit) |
| `exclude` = pre-descent prune | 1000x faster than post-filter for large directories |
| Single `exclude`, no `ignore` | Simpler API; rare "descend but skip" case covered by `filter` |
| **`dot` default false** | Matches Bun.Glob / fs.glob / fast-glob / globby expectations (2026 audit) |
| **`gitignore` opt-in** | Native `ignore`-crate support; globby parity (2026 audit) |
| **`signal` cancellation + error collection** | Long walks must be interruptible; errors must be visible (2026 audit) |
| **`onProgress` per batch in TS** | Zero native overhead; expected by build tools (2026 audit) |
| JS `filter` as post-native | Explicit trade-off: flexibility at cost of per-file FFI |
| Batched async iterator | Clean per-file API with minimal N-API crossings |
| Default batch size 512 | Balanced memory/latency; tunable |
| `withMetadata` opt-in | Avoid stat overhead when only paths are needed |
| `hash` opt-in | Avoid I/O overhead when hashing isn't needed |
| **Sorted fold for hashTree/snapshot** | Deterministic cache keys (2026 audit) |
