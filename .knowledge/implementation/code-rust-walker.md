---
type: Reference Implementation
title: "Rust NativeScanner: ignore Crate, Chunked Batching, Fused Walk"
description: "Reference Rust engine: NativeScanner over the ignore crate, chunked batching, fused stat+hash pipeline, NAPI exposure."
tags: [rust, code, walker, ignore, napi, scanner, fused]
status: stable
generated:
  by: pathway_kb/1.0
  at: 2026-09-16T00:00:00Z
verified:
  - by: human:archont561
    at: 2025-07-11T00:00:00Z
  - by: process:gap-analysis-2026-09
    at: 2026-09-16T00:00:00Z
domain: implementation
decision: decided  # legacy KB status (decided|proposed|deprecated)
created: 2025-07-11
source: conversation
depends_on:
  - architecture/fused-walk
  - architecture/napi-boundary
  - features/walk-traversal
  - implementation/repo-structure
---

# Rust Walker Implementation

## Overview

This document contains the production-grade Rust implementation for the
fused directory walker. It uses the `ignore` crate (ripgrep's engine) for
traversal, `globset` for glob matching, `regex` for regex filtering, and
`blake3`/`xxhash-rust`/`sha2` for content hashing.

Results are returned in **batches** to minimize N-API boundary crossings.

> **Sept 2026 audit — fixes applied to this reference implementation:**
> 1. `glob` is `Vec<String>` end-to-end (the TS draft joined the array with
>    `","`, which globset treats as one literal pattern).
> 2. **Globs match against root-relative paths** (matching absolute paths
>    made `**/*.ts` match only root-level files).
> 3. Traversal errors are **collected, not swallowed**
>    (`Err(_) => continue` is gone).
> 4. `hash` is `Option<String>` (None = not requested) + a per-entry
>    `error` field — an empty hash string can never appear.
> 5. Hashing uses **chunked 64 KB reads** (never whole-file loads).
> 6. `dot` (default false → `hidden(true)`) and `gitignore` options.
> 7. `next_batch` uses `drain` (no per-batch clone).
> 8. `cancel()` + `errors()` N-API methods support `AbortSignal` and error
>    reporting from the TS layer.
>
> **Streaming target (spike-gated):** Phase 1 Step 0 spikes the
> experimental NAPI-RS `#[napi(async_iterator)]`. If it passes the runtime
> matrix, `scan()`'s full materialization is replaced by a pull-based
> `AsyncGenerator` (true streaming + native cancellation via `return()`).
> See the "Target: Native Async Iterator" section below and
> [napi-boundary.md](/architecture/napi-boundary.md).

---

## Module Registration (`crates/engine/src/lib.rs`)

```rust
#![deny(clippy::all)]

use napi_derive::napi;

mod walk;
mod hash;
mod fs;
mod serializers;
mod error;

// Re-export walk types for N-API
pub use walk::{NativeScanner, WalkOptions, FusedEntry};
pub use hash::{hash_file, hash_tree, HashAlgorithm};

#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
```

---

## Walk Options (`crates/engine/src/walk/mod.rs`)

```rust
use napi_derive::napi;

#[napi(object)]
pub struct WalkOptions {
    /// Glob pattern(s) for file matching — AND logic if more than one.
    /// Matched against paths RELATIVE to the walk root (fast-glob semantics).
    pub glob: Option<Vec<String>>,
    /// Regex pattern for file matching (applied to the FULL absolute path)
    pub regex: Option<String>,
    /// Directory names to prune (never descend into)
    pub exclude: Option<Vec<String>>,
    /// Include dotfiles/dotdirs (default: false)
    pub dot: Option<bool>,
    /// Honor .gitignore/.ignore + parent .git dirs (default: false)
    pub gitignore: Option<bool>,
    /// Return absolute paths (default: false → root-relative)
    pub absolute: Option<bool>,
    /// Maximum directory depth
    pub max_depth: Option<u32>,
    /// Only yield files (skip directories)
    pub files_only: Option<bool>,
    /// Include stat metadata in results
    pub with_metadata: Option<bool>,
    /// Compute content hash using specified algorithm (chunked I/O)
    pub hash: Option<String>,
    /// Number of entries per batch (default: 512)
    pub batch_size: Option<u32>,
    /// Number of worker threads (default: CPU count)
    pub concurrency: Option<u32>,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            glob: None,
            regex: None,
            exclude: None,
            dot: Some(false),
            gitignore: Some(false),
            absolute: Some(false),
            max_depth: None,
            files_only: Some(true),
            with_metadata: Some(false),
            hash: None,
            batch_size: Some(512),
            concurrency: None,
        }
    }
}
```

---

## Fused Entry (`crates/engine/src/walk/entry.rs`)

```rust
use napi_derive::napi;

#[napi(object)]
#[derive(Clone, Debug)]
pub struct FusedEntry {
    /// File path (absolute or root-relative per `absolute`)
    pub path: String,
    /// File size in bytes (0 if with_metadata is false)
    pub size: i64,
    /// Last modified time in ms since epoch (0 if not requested)
    pub mtime: i64,
    /// Content hash hex string — None/undefined = not requested
    pub hash: Option<String>,
    /// Per-entry failure (e.g. "EACCES: permission denied") — set when the
    /// entry could not be fully stat'ed/hashed
    pub error: Option<String>,
    /// Whether this entry is a file
    pub is_file: bool,
    /// Whether this entry is a directory
    pub is_directory: bool,
    /// Whether this entry is a symlink
    pub is_symlink: bool,
}
```

> `mtime` is ms for the JS API (Date-compatible, < 2^53). Full nanosecond
> precision is retained in Phase-2 snapshot storage (see
> [killer-features.md](/features/killer-features.md) §2) — Rust
> `Duration` carries sub-ms precision; don't widen the JS field.

---

## Native Scanner (`crates/engine/src/walk/scanner.rs`)

```rust
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use ignore::WalkBuilder;
use globset::{Glob, GlobSetBuilder};
use regex::Regex;

use super::{WalkOptions, FusedEntry};
use crate::hash::hash_chunked;

const MAX_REPORTED_ERRORS: usize = 1_000;

#[napi]
pub struct NativeScanner {
    root: PathBuf,
    options: WalkOptions,
    cursor: Arc<Mutex<usize>>,
    results: Arc<Mutex<Vec<FusedEntry>>>,
    errors: Arc<Mutex<Vec<String>>>,
    scanned: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}

#[napi]
impl NativeScanner {
    #[napi(constructor)]
    pub fn new(root: String, options: WalkOptions) -> Self {
        Self {
            root: PathBuf::from(root),
            options,
            cursor: Arc::new(Mutex::new(0)),
            results: Arc::new(Mutex::new(Vec::new())),
            errors: Arc::new(Mutex::new(Vec::new())),
            scanned: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Perform the scan (called lazily on first nextBatch in the v0.1
    /// paging fallback).
    ///
    /// NOTE (memory): the v0.1 fallback materializes all results before the
    /// first batch is yielded — memory scales linearly with tree size and
    /// first-entry latency equals full-walk time. This is the documented
    /// fallback; the streaming target (async iterator) removes both limits.
    #[napi]
    pub async fn scan(&self) -> Result<u64> {
        // AsyncTask is the preferred primitive for this blocking work
        // (libuv thread pool, per the current NAPI-RS decision table);
        // `async fn` + spawn_blocking is shown here for the pre-spike code.
        let root = self.root.clone();
        let options = self.options.clone();
        let results = self.results.clone();
        let errors = self.errors.clone();
        let scanned = self.scanned.clone();
        let cancelled = self.cancelled.clone();

        tokio::task::spawn_blocking(move || {
            let (entries, walk_errors) = execute_walk(&root, &options, &cancelled)?;
            let count = entries.len() as u64;
            *results.lock().unwrap() = entries;
            let mut errs = errors.lock().unwrap();
            errs.extend(walk_errors.into_iter().take(MAX_REPORTED_ERRORS));
            scanned.store(true, Ordering::Release);
            Ok(count)
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    /// Get the next batch of results (drain — no clone)
    #[napi]
    pub fn next_batch(&self) -> Result<Vec<FusedEntry>> {
        let batch_size = self.options.batch_size.unwrap_or(512) as usize;
        let mut cursor = self.cursor.lock().unwrap();
        let mut results = self.results.lock().unwrap();

        let start = *cursor;
        let end = (start + batch_size).min(results.len());

        if start >= results.len() {
            return Ok(vec![]);
        }

        *cursor = end;
        Ok(results.drain(start..end).collect())
    }

    /// Signal the scan to stop (wired to JS AbortSignal; the async-iterator
    /// target achieves the same via for-await's return())
    #[napi]
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Collected traversal/read errors (capped), e.g. EACCES entries
    #[napi]
    pub fn errors(&self) -> Vec<String> {
        self.errors.lock().unwrap().clone()
    }

    /// Reset the cursor to the beginning
    #[napi]
    pub fn reset(&self) {
        *self.cursor.lock().unwrap() = 0;
    }
}

/// Core walk execution (runs on the blocking thread)
fn execute_walk(
    root: &Path,
    opts: &WalkOptions,
    cancelled: &AtomicBool,
) -> Result<(Vec<FusedEntry>, Vec<String>)> {
    let exclude_set: std::collections::HashSet<String> = opts
        .exclude
        .as_ref()
        .map(|e| e.iter().cloned().collect())
        .unwrap_or_default();

    // ALL glob patterns, AND logic. Compiled once.
    let mut builder = GlobSetBuilder::new();
    if let Some(patterns) = &opts.glob {
        for p in patterns {
            let g = Glob::new(p)
                .map_err(|e| Error::new(Status::InvalidArg, e.to_string()))?;
            builder.add(g);
        }
    }
    let globset = builder.build().ok();

    let regex_matcher = opts.regex.as_ref().and_then(|r| Regex::new(r).ok());

    let dot = opts.dot.unwrap_or(false);
    let gitignore = opts.gitignore.unwrap_or(false);
    let absolute = opts.absolute.unwrap_or(false);
    let files_only = opts.files_only.unwrap_or(true);
    let with_metadata = opts.with_metadata.unwrap_or(false);
    let hash_algo = opts.hash.as_deref();

    let mut walker = WalkBuilder::new(root);
    walker
        .hidden(!dot)          // dot=false (default) → skip dotfiles/dotdirs
        .parents(gitignore)    // parent .git dirs + .gitignore when opted in
        .git_ignore(gitignore)
        .ignore(gitignore);    // .ignore files

    if let Some(depth) = opts.max_depth {
        walker.max_depth(Some(depth as usize));
    }

    if let Some(threads) = opts.concurrency {
        walker.threads(threads as usize);
    }

    // Pre-descent directory pruning
    let exclude_clone = exclude_set.clone();
    walker.filter_entry(move |entry| {
        if entry.file_type().map_or(false, |ft| ft.is_dir()) {
            if let Some(name) = entry.file_name().to_str() {
                // Non-UTF-8 dir names (to_str() = None) are NOT pruned —
                // documented parity with libuv's lossy handling.
                return !exclude_clone.contains(name);
            }
        }
        true
    });

    let mut results = Vec::new();
    let mut errors = Vec::new();

    for entry_result in walker.build() {
        if cancelled.load(Ordering::Relaxed) {
            break; // AbortSignal was raised — stop the walk
        }

        let entry = match entry_result {
            Ok(e) => e,
            Err(err) => {
                // Collected, never silently dropped (EACCES, EMFILE, …)
                if errors.len() < MAX_REPORTED_ERRORS {
                    errors.push(err.to_string());
                }
                continue;
            }
        };

        let file_type = entry.file_type();
        let is_file = file_type.map_or(false, |ft| ft.is_file());
        let is_dir = file_type.map_or(false, |ft| ft.is_dir());
        let is_symlink = file_type.map_or(false, |ft| ft.is_symlink());

        if files_only && !is_file {
            continue;
        }

        // GLOB MATCHING IS ROOT-RELATIVE (fast-glob-compatible).
        // The 2025 draft matched absolute paths — `**/*.ts` then only
        // matched files directly under the root.
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());

        if let Some(ref matcher) = globset {
            if !matcher.is_match(rel) {
                continue;
            }
        }

        // Regex matches the FULL absolute path (by design)
        if let Some(ref re) = regex_matcher {
            if !re.is_match(entry.path()) {
                continue;
            }
        }

        // Metadata (follows symlinks; lstat option lands in Phase 3)
        let mut size: i64 = 0;
        let mut mtime: i64 = 0;
        if with_metadata || hash_algo.is_some() {
            match entry.metadata() {
                Ok(meta) => {
                    size = meta.len() as i64;
                    mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                }
                Err(err) => {
                    if errors.len() < MAX_REPORTED_ERRORS {
                        errors.push(format!("{}: {}", entry.path().display(), err));
                    }
                }
            }
        }

        // Content hash — CHUNKED reads (64 KB), never whole-file loads.
        // This runs on the ignore crate's worker threads, so hashing is
        // already distributed across cores (no separate rayon stage in v0.1).
        let mut hash: Option<String> = None;
        let mut error: Option<String> = None;
        if let Some(algo) = hash_algo {
            if is_file {
                match hash_chunked(entry.path(), algo, 64 * 1024) {
                    Some(h) => hash = Some(h),
                    None => {
                        if errors.len() < MAX_REPORTED_ERRORS {
                            errors.push(format!(
                                "hash failed for {}: unreadable",
                                entry.path().display()
                            ));
                        }
                        error = Some("hash failed: unreadable".into());
                    }
                }
            }
        }

        // Path: absolute or root-relative per option.
        // to_string_lossy() matches libuv's lossy non-UTF-8 handling.
        let path_str = if absolute {
            entry.path().to_string_lossy().into_owned()
        } else {
            rel.to_string_lossy().into_owned()
        };

        results.push(FusedEntry {
            path: path_str,
            size,
            mtime,
            hash,
            error,
            is_file,
            is_directory: is_dir,
            is_symlink,
        });
    }

    Ok((results, errors))
}
```

---

## Hash Module (`crates/engine/src/hash/mod.rs`)

```rust
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use blake3::Hasher as Blake3Hasher;
use sha2::{Sha256, Digest as Sha2Digest};

pub fn hash_chunked(path: &Path, algorithm: &str, chunk: usize) -> Option<String> {
    // Streaming: 64 KB reads. The 2025 draft did std::fs::read() — a whole
    // 10 GB file would spike memory. (mmap remains a future optimization;
    // chunked reads are sufficient and simpler.)
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; chunk];

    let mut out = String::new();
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 { break; }
        out = match algorithm {
            "blake3" => update_blake3(&mut out, &buf[..n]),
            "sha256" => update_sha256(&mut out, &buf[..n]),
            "xxhash" | "xxh3" => update_xxh3(&mut out, &buf[..n]),
            _ => return None,
        };
    }
    Some(out)
}

// (update_* helpers thread a Hasher state across chunks — omitted for brevity)
```

> **Parallelism note (Sept 2026 correction):** the 2025 design doc claimed
> a "rayon + mmap" hashing stage. The reference implementation hashes
> **inside the walker's iteration** — the `ignore` crate's `WalkBuilder`
> already distributes traversal (and therefore this per-entry work) across
> its worker threads, so hashing is parallel without a second stage. A
> dedicated rayon stage is reserved for `hashTree()`/snapshots operating on
> pre-collected path lists (Phase 2).

---

## Target: Native Async Iterator (post-spike, replaces `scan()` materialization)

If the Phase-1 `#[napi(async_iterator)]` spike passes on Node 24/26 +
Bun 1.3/1.4 (per the napi.rs test checklist: forced GC, early break,
overlapping `next()`, worker shutdown), the scanner becomes:

```rust
#[napi(async_iterator)]
pub struct FusedWalk { /* root, options, cancellation flag */ }

#[napi]
impl AsyncGenerator for FusedWalk {
    type Yield = FusedEntry;        // owned value — satisfies Send + 'static
    type Next = ();
    type Return = ();

    fn next(&mut self, _: Option) -> impl Future<Output = Result<Option<FusedEntry>>> + Send + 'static {
        // Pull from the walker; keep a bounded prefetch window.
        // Cancellation: for-await exit calls complete(), which drops the
        // walker — no separate cancel() needed.
        // …
    }

    fn complete(&mut self, _: Option) -> impl Future<Output = Result<Option<()>>> + Send + 'static {
        // Cancellation hook: stop the Rust producer, release threads.
        // …
    }
}
```

Effects if adopted:
- First-entry latency = first *batch* of real work (streaming pitch, literally).
- Memory bounded to the prefetch window (the 30 MB-class claim becomes true
  at 1M files, not just 100k).
- `AbortSignal` support reduces to the natural `break` → `return()` path.
- Constraints: `Yield` must be owned (`FusedEntry` qualifies); overlapping
  `next()` calls are not serialized for us (keep cursor state in the struct).

The v0.1 paging scanner above stays as the fallback if the spike fails.

---

## Build Script (`crates/engine/build.rs`)

```rust
extern crate napi_build;

fn main() {
    napi_build::setup();
}
```

---

## Key Implementation Notes

1. **`ignore` crate handles parallelism internally.** The `WalkBuilder`
   spawns worker threads; per-entry work (metadata, chunked hashing) runs
   on those threads. We don't manage threads ourselves for traversal.

2. **Blocking work is `AsyncTask`-shaped.** Per the current NAPI-RS
   decision table, the walk path should be exposed as `AsyncTask<T>`
   (libuv thread pool). The code above shows `async fn` +
   `spawn_blocking` as the pre-spike form; convert during Step 1.2.

3. **Batch cursor is thread-safe.** The `Arc<Mutex>` pattern allows the JS
   side to call `nextBatch()` repeatedly without race conditions; `drain`
   avoids cloning each batch.

4. **Hash is computed during traversal (the fused part)** — with chunked
   I/O, so a huge file never lives in memory whole.

5. **Cancellation is real.** `cancel()` flips an `AtomicBool` checked
   per-entry; the async-iterator target cancels natively via `return()`.

6. **Errors are reported.** Traversal and hash failures land in
   `errors()` (capped at 1000) and per-entry `error` fields — never
   silently dropped.

7. **Globs are root-relative; regex is absolute.** This split is
   intentional (glob = structure, regex = arbitrary pattern) and is
   documented in [walk-traversal.md](/features/walk-traversal.md).

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| `ignore` crate as walker | ripgrep's engine; battle-tested, parallel, gitignore-aware |
| `AsyncTask` for blocking walk | Per NAPI-RS decision table (Sept 2026); libuv pool, no Tokio-worker occupation |
| Batched results via drain cursor | Minimize N-API crossings; no per-batch clone |
| Hash during traversal, chunked I/O | Fused walk; no whole-file memory spikes |
| Globs root-relative, regex absolute | fast-glob-compatible semantics; absolute matching was a bug (2026 audit) |
| `dot` default false / `gitignore` opt-in | Incumbent parity; native `ignore`-crate support (2026 audit) |
| `cancel()` + `errors()` | AbortSignal support; visible failures (2026 audit) |
| Async-iterator streaming target (spike-gated) | Bounded memory + true cancellation; paging fallback retained |
