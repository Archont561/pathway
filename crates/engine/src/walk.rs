//! The walk bridge: [`pathway_fs_core::walk::NativeScanner`] over N-API.
//!
//! The transport is the one task-1 froze: **chunked paging over
//! [`AsyncTask`]** (`.knowledge/architecture/napi-boundary.md`, Step 0 spike
//! record). A [`Walker`] is constructed synchronously — so a bad glob fails
//! before any traversal, at the call site — and then paged from JavaScript:
//! `scan()` once, `nextBatch()` until it returns an empty array, `cancel()`
//! at any point. Each call is one boundary crossing; each crossing carries a
//! whole batch (D2: bulk operations only).
//!
//! Glue only. Everything here is a type translation; the walk itself, the
//! filter semantics, the batching and the cancellation flag live in
//! `pathway-fs-core`, where `cargo test` reaches them without Node.

use std::sync::Arc;

use napi::bindgen_prelude::{AsyncTask, BigInt};
use napi::{Env, Task};
use napi_derive::napi;

use pathway_fs_core::error::Error;
use pathway_fs_core::hash::Algorithm;
use pathway_fs_core::walk::{FusedEntry, NativeScanner, ScanOptions, DEFAULT_BATCH_SIZE};

/// Why one entry could not be completed, as the TypeScript surface spells it.
///
/// The `kind` strings are the `EntryErrorKind` union in
/// `packages/path/src/types.ts`; the mapping lives in [`error_kind`].
#[napi(object)]
pub struct WalkEntryError {
    /// One of `stat`, `read`, `hasher`, `escaped`, `codec`.
    pub kind: String,
    /// The core error, rendered.
    pub message: String,
}

/// One fused-walk result, shaped exactly like `PathEntry` in
/// `packages/path/src/types.ts` so the TypeScript side yields it without
/// re-mapping.
#[napi(object)]
pub struct WalkEntry {
    /// The path, root-relative unless `absolute` was requested.
    pub value: String,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// Size in bytes, when metadata was requested and the `stat` succeeded.
    pub size: Option<f64>,
    /// Modification time in nanoseconds since the Unix epoch, as a BigInt.
    ///
    /// The core keeps `i128`; it crosses as napi's `BigInt` wrapper because
    /// `#[napi(object)]` fields must convert in both directions and the raw
    /// `i128` only converts outward. BigInt support is why the workspace
    /// enables the `napi8` feature (gated on napi6+).
    pub modified_nanos: Option<BigInt>,
    /// Lowercase hex digest, when a hash algorithm was requested.
    pub hash: Option<String>,
    /// Why this entry could not be completed, if it could not.
    pub error: Option<WalkEntryError>,
}

/// The options object a TypeScript caller passes, every field optional.
///
/// Mirrors `WalkOptions` in `packages/path/src/walk.ts` plus the two fields
/// the TypeScript wrappers set themselves (`filesOnly`, `maxDepth` is public
/// in core but not yet surfaced). Defaults are applied here, in one place,
/// so core's `ScanOptions::default()` and the JS docs cannot drift apart
/// silently.
#[napi(object)]
#[derive(Default)]
pub struct WalkerOptions {
    /// Glob patterns, AND semantics, matched root-relative. `!` negates.
    pub glob: Option<Vec<String>>,
    /// A regular expression, matched against the full absolute path.
    pub regex: Option<String>,
    /// Directory *names* never descended into.
    pub exclude: Option<Vec<String>>,
    /// Include dotfiles and dot-directories. Default `false`.
    pub dot: Option<bool>,
    /// Honour `.gitignore`, `.ignore` and parent lookups. Default `false`.
    pub gitignore: Option<bool>,
    /// Yield absolute paths instead of root-relative ones. Default `false`.
    pub absolute: Option<bool>,
    /// Maximum traversal depth, where the root is depth 0.
    pub max_depth: Option<u32>,
    /// Skip directories, yielding only files. Default `false` here —
    /// `walk()` yields everything and the TypeScript wrappers narrow.
    pub files_only: Option<bool>,
    /// Populate `size` and `modifiedNanos`. Default `true` here: the fused
    /// pass is the reason to cross the boundary at all.
    pub with_metadata: Option<bool>,
    /// Hash algorithm name: `blake3`, `xxhash` or `sha256`.
    pub hash: Option<String>,
    /// Entries per batch. Default [`DEFAULT_BATCH_SIZE`].
    pub batch_size: Option<u32>,
    /// Worker threads. Absent lets the walker choose from the CPU count.
    pub concurrency: Option<u32>,
}

/// The paged walk handle the TypeScript `walk()` generator drives.
#[napi]
pub struct Walker {
    inner: Arc<NativeScanner>,
}

#[napi]
impl Walker {
    /// Compiles the filter and resolves the root — synchronously, so a bad
    /// glob or a missing root fails at the call site, before any traversal.
    ///
    /// # Errors
    ///
    /// Rejects with the core error (which names the pattern as the caller
    /// wrote it) if the root cannot be resolved or a pattern does not
    /// compile.
    #[napi(constructor)]
    pub fn new(root: String, options: Option<WalkerOptions>) -> napi::Result<Self> {
        let options = options.unwrap_or_default();

        let hash = options
            .hash
            .as_deref()
            .map(Algorithm::from_name)
            .transpose()
            .map_err(bridge_error)?;

        let scan_options = ScanOptions {
            glob: options.glob.unwrap_or_default(),
            regex: options.regex,
            exclude: options.exclude.unwrap_or_default(),
            dot: options.dot.unwrap_or(false),
            gitignore: options.gitignore.unwrap_or(false),
            absolute: options.absolute.unwrap_or(false),
            max_depth: options.max_depth.map(|depth| depth as usize),
            files_only: options.files_only.unwrap_or(false),
            with_metadata: options.with_metadata.unwrap_or(true),
            hash,
            batch_size: options
                .batch_size
                .map(|size| size as usize)
                .filter(|size| *size > 0)
                .unwrap_or(DEFAULT_BATCH_SIZE),
            concurrency: options.concurrency.map(|threads| threads as usize),
        };

        let inner = NativeScanner::new(&root, scan_options).map_err(bridge_error)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Runs the traversal on the libuv thread pool, resolving with the number
    /// of entries collected.
    ///
    /// Blocking and CPU-bound, which is exactly what `AsyncTask` exists for;
    /// the JavaScript thread never sees the walk itself.
    #[napi(ts_return_type = "Promise<number>")]
    pub fn scan(&self) -> AsyncTask<ScanTask> {
        AsyncTask::new(ScanTask {
            inner: Arc::clone(&self.inner),
        })
    }

    /// Resolves with the next batch, or an empty array when the walk is
    /// drained. One boundary crossing per call.
    #[napi(ts_return_type = "Promise<Array<WalkEntry>>")]
    pub fn next_batch(&self) -> AsyncTask<NextBatchTask> {
        AsyncTask::new(NextBatchTask {
            inner: Arc::clone(&self.inner),
        })
    }

    /// Stops the walk at the next cancellation check. Synchronous and cheap:
    /// it sets the core's atomic flag, which the worker threads observe.
    #[napi]
    pub fn cancel(&self) {
        self.inner.cancel();
    }

    /// The traversal failures collected so far, capped in core at
    /// `MAX_REPORTED_ERRORS` (1,000).
    ///
    /// The TypeScript `walk()` generator reads this once, after the last
    /// batch, and throws `WalkError` when it is non-empty — one synchronous
    /// bulk crossing per walk, never a crossing per failure.
    #[napi]
    pub fn errors(&self) -> Vec<String> {
        self.inner.errors()
    }
}

/// `scan()` as a libuv-pool task.
pub struct ScanTask {
    inner: Arc<NativeScanner>,
}

#[napi]
impl Task for ScanTask {
    type Output = usize;
    type JsValue = u32;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        self.inner.scan().map_err(bridge_error)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        u32::try_from(output).map_err(|_| {
            napi::Error::from_reason(format!(
                "walk collected {output} entries, which overflows the JS count"
            ))
        })
    }
}

/// `nextBatch()` as a libuv-pool task.
///
/// The `FusedEntry` → [`WalkEntry`] translation happens in `compute`, off the
/// JavaScript thread: every field is a plain owned value, so only the final
/// object construction (in napi's own marshaling) touches the JS heap.
pub struct NextBatchTask {
    inner: Arc<NativeScanner>,
}

#[napi]
impl Task for NextBatchTask {
    type Output = Vec<WalkEntry>;
    type JsValue = Vec<WalkEntry>;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        Ok(self
            .inner
            .next_batch()
            .into_iter()
            .map(to_walk_entry)
            .collect())
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

/// One `FusedEntry`, translated. Pure data movement — no logic.
fn to_walk_entry(entry: FusedEntry) -> WalkEntry {
    WalkEntry {
        // Public paths use `/` on every host, matching `pathe` and the
        // root-relative glob contract. Native filesystem access still uses
        // the original platform path internally; this is only the boundary
        // representation.
        value: entry.path.to_string_lossy().replace('\\', "/"),
        is_dir: entry.is_dir,
        // `f64` loses nothing until 2^53 bytes (8 PiB), far beyond a file a
        // walk can hash; `size` stays a plain JS number on purpose.
        size: entry.size.map(|size| size as f64),
        modified_nanos: entry.modified_nanos.map(BigInt::from),
        hash: entry.hash,
        error: entry.error.map(|error| WalkEntryError {
            kind: error_kind(&error).to_owned(),
            message: error.to_string(),
        }),
    }
}

/// Maps a core error onto the `EntryErrorKind` union.
///
/// Per-entry failures can only be `stat` (metadata) or `open`/`read`
/// (hashing) today — `UnknownHasher` fails the constructor and `Escaped` is
/// task-15 — but the match is total so a new core variant cannot silently
/// become a wrong kind.
fn error_kind(error: &Error) -> &'static str {
    match error {
        Error::Io { operation, .. } if *operation == "stat" => "stat",
        Error::Io { .. } => "read",
        Error::UnknownHasher(_) => "hasher",
        Error::Escaped { .. } => "escaped",
        _ => "codec",
    }
}

/// Core error → N-API rejection, keeping the core's message verbatim: it
/// already names the operation, the path and the pattern as written.
fn bridge_error(error: Error) -> napi::Error {
    napi::Error::from_reason(error.to_string())
}
