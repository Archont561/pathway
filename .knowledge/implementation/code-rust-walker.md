---
title: "Rust NativeScanner: ignore Crate, Chunked Batching, Fused Walk"
domain: implementation
status: decided
created: 2025-07-11
updated: 2025-07-11
source: conversation
depends_on:
  - architecture/fused-walk
  - architecture/napi-boundary
  - features/walk-traversal
  - implementation/repo-structure
tags: [rust, code, walker, ignore, napi, scanner, fused]
---

# Rust Walker Implementation

## Overview

This document contains the production-grade Rust implementation for the
fused directory walker. It uses the `ignore` crate (ripgrep's engine) for
traversal, `globset` for glob matching, `regex` for regex filtering, and
`blake3`/`xxhash-rust`/`sha2` for content hashing.

Results are returned in **batches** to minimize N-API boundary crossings.

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
    /// Glob pattern(s) for file matching
    pub glob: Option<String>,
    /// Regex pattern for file matching (applied to full path)
    pub regex: Option<String>,
    /// Directory names to prune (never descend into)
    pub exclude: Option<Vec<String>>,
    /// Maximum directory depth
    pub max_depth: Option<u32>,
    /// Only yield files (skip directories)
    pub files_only: Option<bool>,
    /// Include stat metadata in results
    pub with_metadata: Option<bool>,
    /// Compute content hash using specified algorithm
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
    /// Absolute file path
    pub path: String,
    /// File size in bytes (0 if with_metadata is false)
    pub size: i64,
    /// Last modified time in milliseconds since epoch (0 if not requested)
    pub mtime: i64,
    /// Content hash hex string (empty if hash not requested)
    pub hash: String,
    /// Whether this entry is a file
    pub is_file: bool,
    /// Whether this entry is a directory
    pub is_directory: bool,
    /// Whether this entry is a symlink
    pub is_symlink: bool,
}
```

---

## Native Scanner (`crates/engine/src/walk/scanner.rs`)

```rust
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use ignore::WalkBuilder;
use globset::Glob;
use regex::Regex;

use super::{WalkOptions, FusedEntry};
use crate::hash::compute_hash;

#[napi]
pub struct NativeScanner {
    root: PathBuf,
    options: WalkOptions,
    cursor: Arc<Mutex<usize>>,
    results: Arc<Mutex<Vec<FusedEntry>>>,
    scanned: Arc<Mutex<bool>>,
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
            scanned: Arc::new(Mutex::new(false)),
        }
    }

    /// Perform the full scan (called lazily on first nextBatch)
    #[napi]
    pub async fn scan(&self) -> Result<u32> {
        let root = self.root.clone();
        let options = WalkOptions {
            glob: self.options.glob.clone(),
            regex: self.options.regex.clone(),
            exclude: self.options.exclude.clone(),
            max_depth: self.options.max_depth,
            files_only: self.options.files_only,
            with_metadata: self.options.with_metadata,
            hash: self.options.hash.clone(),
            batch_size: self.options.batch_size,
            concurrency: self.options.concurrency,
        };
        let results = self.results.clone();
        let scanned = self.scanned.clone();

        tokio::task::spawn_blocking(move || {
            let entries = execute_walk(&root, &options)?;
            let count = entries.len() as u32;
            *results.lock().unwrap() = entries;
            *scanned.lock().unwrap() = true;
            Ok(count)
        })
        .await
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?
    }

    /// Get the next batch of results
    #[napi]
    pub fn next_batch(&self) -> Result<Vec<FusedEntry>> {
        let batch_size = self.options.batch_size.unwrap_or(512) as usize;
        let mut cursor = self.cursor.lock().unwrap();
        let results = self.results.lock().unwrap();

        let start = *cursor;
        let end = (start + batch_size).min(results.len());

        if start >= results.len() {
            return Ok(vec![]);
        }

        let batch = results[start..end].to_vec();
        *cursor = end;
        Ok(batch)
    }

    /// Reset the cursor to the beginning
    #[napi]
    pub fn reset(&self) {
        *self.cursor.lock().unwrap() = 0;
    }
}

/// Core walk execution (runs on blocking thread)
fn execute_walk(root: &PathBuf, opts: &WalkOptions) -> Result<Vec<FusedEntry>> {
    let exclude_set: std::collections::HashSet<String> = opts
        .exclude
        .as_ref()
        .map(|e| e.iter().cloned().collect())
        .unwrap_or_default();

    let glob_matcher = opts.glob.as_ref().and_then(|g| {
        Glob::new(g).ok().map(|g| g.compile_matcher())
    });

    let regex_matcher = opts.regex.as_ref().and_then(|r| {
        Regex::new(r).ok()
    });

    let files_only = opts.files_only.unwrap_or(true);
    let with_metadata = opts.with_metadata.unwrap_or(false);
    let hash_algo = opts.hash.as_deref();

    let mut walker = WalkBuilder::new(root);
    walker
        .hidden(false)
        .parents(false)
        .ignore(false)
        .git_global(false)
        .git_ignore(false);

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
                return !exclude_clone.contains(name);
            }
        }
        true
    });

    let mut results = Vec::new();

    for entry_result in walker.build() {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => continue,
        };

        let file_type = entry.file_type();
        let is_file = file_type.map_or(false, |ft| ft.is_file());
        let is_dir = file_type.map_or(false, |ft| ft.is_dir());
        let is_symlink = file_type.map_or(false, |ft| ft.is_symlink());

        if files_only && !is_file {
            continue;
        }

        let path = entry.path();
        let path_str = path.to_string_lossy();

        // Glob filter
        if let Some(ref matcher) = glob_matcher {
            if !matcher.is_match(path_str.as_ref()) {
                continue;
            }
        }

        // Regex filter
        if let Some(ref re) = regex_matcher {
            if !re.is_match(&path_str) {
                continue;
            }
        }

        // Metadata
        let (size, mtime) = if with_metadata || hash_algo.is_some() {
            match entry.metadata() {
                Ok(meta) => {
                    let size = meta.len() as i64;
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    (size, mtime)
                }
                Err(_) => (0, 0),
            }
        } else {
            (0, 0)
        };

        // Content hash
        let hash = if let Some(algo) = hash_algo {
            if is_file {
                compute_hash(path, algo).unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        results.push(FusedEntry {
            path: path_str.into_owned(),
            size,
            mtime,
            hash,
            is_file,
            is_directory: is_dir,
            is_symlink,
        });
    }

    Ok(results)
}
```

---

## Hash Module (`crates/engine/src/hash/mod.rs`)

```rust
use std::path::Path;
use blake3::Hasher as Blake3Hasher;
use sha2::{Sha256, Digest as Sha2Digest};
use xxhash_rust::xxh3::Xxh3;

pub fn compute_hash(path: &Path, algorithm: &str) -> Option<String> {
    let data = std::fs::read(path).ok()?;

    let hex = match algorithm {
        "blake3" => {
            let mut hasher = Blake3Hasher::new();
            hasher.update(&data);
            hasher.finalize().to_hex().to_string()
        }
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(&data);
            format!("{:x}", hasher.finalize())
        }
        "xxhash" | "xxh3" => {
            let hash = xxhash_rust::xxh3::xxh3_64(&data);
            format!("{:016x}", hash)
        }
        _ => return None,
    };

    Some(hex)
}
```

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
   spawns worker threads via `rayon`. We don't need to manage threads
   ourselves for traversal.

2. **`spawn_blocking` is critical.** The walk is CPU+I/O bound and would
   block the Node/Bun event loop if run on the main thread. `tokio::task::
   spawn_blocking` moves it to a dedicated thread pool.

3. **Batch cursor is thread-safe.** The `Arc<Mutex<usize>>` pattern allows
   the JS side to call `nextBatch()` repeatedly from the async iterator
   without race conditions.

4. **Hash is computed during traversal.** This is the "fused" part — we
   read the file content and hash it while we're already visiting the
   entry, avoiding a second pass.

5. **Large file hashing should use streaming.** The current implementation
   reads the entire file into memory. For files >100MB, we should switch
   to a streaming hash (read in 64KB chunks). This is a v0.2 optimization.

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| `ignore` crate as walker | ripgrep's engine; battle-tested, parallel, gitignore-aware |
| `spawn_blocking` for walk | Prevents event loop blocking |
| Batched results via cursor | Minimizes N-API crossings; clean async iterator on JS side |
| Hash during traversal | Fused walk; avoids second pass |
| `Arc<Mutex>` for state | Thread-safe cursor for repeated `nextBatch()` calls |
| In-memory hash (v0.1) | Simple; streaming optimization deferred to v0.2 |
