//! The `ignore`-backed walker.
//!
//! Wraps the `ignore` crate (ripgrep's walker) and owns the options that are
//! properties of the *walk* rather than of the filter: `dot` (default `false`,
//! which means `hidden(true)` — hidden entries are skipped unless asked for)
//! and `gitignore` (`.gitignore` and `.ignore` honoured, plus parent-directory
//! lookups, all of which `ignore` gives for free).
//!
//! The filter is [`super::matcher`]'s job. Keeping the split there means
//! adding a filter never has to re-derive `hidden`/`gitignore`, which is where
//! a traversal silently misses files.
//!
//! Spec: `.knowledge/implementation/code-rust-walker.md`.
//!
//! # Two deviations from the reference implementation, on purpose
//!
//! **1. There is no `reset()`, and iteration is destructive.** The reference
//! keeps a cursor *and* drains: `*cursor = end; results.drain(start..end)`.
//! Those two cannot both be right — draining shifts every later index down, so
//! the second call starts reading at `512` in a vector whose element `512` is
//! now what used to be element `1024`, and half the walk is silently skipped.
//! The fix is to pick one, and the type system picks for us: [`FusedEntry`] is
//! deliberately not `Clone` (see `entry.rs` — [`crate::error::Error`] wraps
//! `io::Error`, which is neither `Clone` nor `PartialEq`), so a batch has to be
//! *moved* out. Rewinding would mean re-walking, so there is nothing for a
//! `reset()` to do.
//!
//! **2. The walk is parallel.** The reference calls `WalkBuilder::build()`,
//! which is the single-threaded iterator, while its own notes claim "the
//! `WalkBuilder` spawns worker threads". Only [`ignore::WalkParallel`] does
//! that, and the fused-walk claim depends on it: hashing happens per entry, so
//! a single-threaded walk hashes a tree on one core and the `concurrency`
//! option would be a lie. This uses `build_parallel()`.
//!
//! The cost of parallelism is that **yield order is unspecified**. Nothing in
//! the fused walk's contract promises an order, and the streaming target
//! (task-1/task-3) could not honour one anyway, so this is a property to rely
//! on rather than a regression: callers that need an order sort.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use ignore::{WalkBuilder, WalkState};

use super::entry::FusedEntry;
use super::matcher::Matcher;
use crate::error::{Error, Result};
use crate::hash::{hash_file, hash_reader, Algorithm};

/// How many traversal failures a single walk will report.
///
/// A walk of an unreadable tree can produce one error per entry, and a million
/// `EACCES` strings is not a diagnostic — it is a second memory problem on top
/// of the first. The first thousand say everything a caller needs.
///
/// # Examples
///
/// ```
/// assert_eq!(pathway_fs_core::walk::MAX_REPORTED_ERRORS, 1_000);
/// ```
pub const MAX_REPORTED_ERRORS: usize = 1_000;

/// The default number of entries in one batch.
///
/// 512, and it is the number every benchmark in
/// `.knowledge/architecture/fused-walk.md` is quoted at. Tuning it is task-4's
/// job, with evidence.
///
/// # Examples
///
/// ```
/// use pathway_fs_core::walk::{ScanOptions, DEFAULT_BATCH_SIZE};
///
/// assert_eq!(ScanOptions::default().batch_size, DEFAULT_BATCH_SIZE);
/// ```
pub const DEFAULT_BATCH_SIZE: usize = 512;

/// Everything that configures one walk.
///
/// A plain struct with a [`Default`], not a builder: the engine bridge
/// (task-3) constructs this from a decoded JavaScript object, where every field
/// arrives at once and a builder would be ceremony around a struct literal.
///
/// `clippy::struct_excessive_bools` fires on the five flags and the advice —
/// group them into an enum or a bitflags type — is wrong for this type. These
/// are not a state machine: each flag is independent, every one of them is a
/// named field in the public `WalkOptions` object a TypeScript caller writes,
/// and inventing a grouping here would mean the engine bridge translating a
/// flat JS object into a shape that exists only to satisfy a lint.
///
/// # Examples
///
/// A struct literal over [`Default`], which is exactly how the engine bridge
/// builds it from a decoded JavaScript object:
///
/// ```
/// use pathway_fs_core::walk::ScanOptions;
///
/// let options = ScanOptions {
///     glob: vec!["**/*.ts".to_owned()],
///     files_only: false,
///     ..ScanOptions::default()
/// };
///
/// assert!(!options.dot, "hidden entries are skipped unless asked for");
/// assert!(!options.gitignore);
/// ```
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Glob patterns, AND semantics, matched root-relative. `!` negates.
    pub glob: Vec<String>,
    /// A regular expression, matched against the full absolute path.
    pub regex: Option<String>,
    /// Directory *names* never descended into.
    pub exclude: Vec<String>,
    /// Include dotfiles and dot-directories. Default `false`.
    pub dot: bool,
    /// Honour `.gitignore`, `.ignore` and parent-directory ignore files.
    /// Default `false`.
    pub gitignore: bool,
    /// Yield absolute paths instead of root-relative ones. Default `false`.
    pub absolute: bool,
    /// Maximum traversal depth, where the root is depth 0.
    pub max_depth: Option<usize>,
    /// Skip directories, yielding only files. Default `true`.
    pub files_only: bool,
    /// Populate `size` and `modified_nanos`. Default `false`.
    pub with_metadata: bool,
    /// Hash every file's contents while traversing.
    pub hash: Option<Algorithm>,
    /// Entries per batch. Default [`DEFAULT_BATCH_SIZE`].
    pub batch_size: usize,
    /// Worker threads. `None` lets `ignore` choose from the CPU count.
    pub concurrency: Option<usize>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            glob: Vec::new(),
            regex: None,
            exclude: Vec::new(),
            dot: false,
            gitignore: false,
            absolute: false,
            max_depth: None,
            files_only: true,
            with_metadata: false,
            hash: None,
            batch_size: DEFAULT_BATCH_SIZE,
            concurrency: None,
        }
    }
}

/// The fused walker.
///
/// Call [`scan`](Self::scan) once, then [`next_batch`](Self::next_batch) until
/// it returns an empty vector.
///
/// # Examples
///
/// ```
/// use pathway_fs_core::walk::{NativeScanner, ScanOptions};
///
/// let dir = tempfile::tempdir()?;
/// std::fs::create_dir(dir.path().join("src"))?;
/// std::fs::write(dir.path().join("src/main.ts"), b"export {};")?;
/// std::fs::write(dir.path().join("README.md"), b"# hi")?;
///
/// let options = ScanOptions {
///     glob: vec!["**/*.ts".to_owned()],
///     ..ScanOptions::default()
/// };
/// let scanner = NativeScanner::new(dir.path(), options)?;
///
/// assert_eq!(scanner.scan()?, 1);
/// let batch = scanner.next_batch();
/// assert_eq!(batch.len(), 1);
/// assert_eq!(batch[0].path, std::path::Path::new("src/main.ts"));
/// assert!(scanner.next_batch().is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct NativeScanner {
    root: PathBuf,
    options: ScanOptions,
    matcher: Arc<Matcher>,
    results: Arc<Mutex<VecDeque<FusedEntry>>>,
    errors: Arc<Mutex<Vec<String>>>,
    cancelled: Arc<AtomicBool>,
    scanned: AtomicBool,
}

impl NativeScanner {
    /// Compiles the filter and resolves the root.
    ///
    /// Compilation happens here rather than inside the walk so a bad pattern is
    /// an error the caller gets *before* any traversal begins, instead of a
    /// walk that silently matches nothing.
    ///
    /// The root is canonicalised, which is what makes the regex half of the
    /// filter meaningful: the specified semantics are "the full absolute path",
    /// and a relative root would otherwise yield relative entry paths. Symlink
    /// resolution is a side effect of that; symlink *policy* is task-18.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the root cannot be resolved, or [`Error::Glob`] /
    /// [`Error::Regex`] if a pattern does not compile.
    ///
    /// # Examples
    ///
    /// Deliberately not repeated here: the [`NativeScanner`] type-level
    /// example begins with this constructor.
    pub fn new(root: impl AsRef<Path>, options: ScanOptions) -> Result<Self> {
        let root = root.as_ref();
        let canonical = std::fs::canonicalize(root)
            .map_err(|source| Error::io("canonicalize", root, source))?;

        let matcher = Matcher::new(&options.glob, options.regex.as_deref())?;

        Ok(Self {
            root: canonical,
            options,
            matcher: Arc::new(matcher),
            results: Arc::new(Mutex::new(VecDeque::new())),
            errors: Arc::new(Mutex::new(Vec::new())),
            cancelled: Arc::new(AtomicBool::new(false)),
            scanned: AtomicBool::new(false),
        })
    }

    /// The canonical root this scanner walks.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::walk::{NativeScanner, ScanOptions};
    ///
    /// let dir = tempfile::tempdir()?;
    /// let scanner = NativeScanner::new(dir.path(), ScanOptions::default())?;
    ///
    /// // Canonicalised at construction — symlinks in the root are resolved
    /// // here, which is what makes the regex-vs-absolute-path rule coherent.
    /// assert_eq!(scanner.root(), std::fs::canonicalize(dir.path())?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Runs the walk, returning how many entries it collected.
    ///
    /// Blocking and CPU-bound. The engine bridge runs it on the libuv pool via
    /// `AsyncTask`; nothing in this crate knows that.
    ///
    /// # Errors
    ///
    /// Does not fail on an unreadable entry — those are collected and reported
    /// by [`errors`](Self::errors). The `Result` is reserved for a failure that
    /// invalidates the whole walk.
    ///
    /// # Examples
    ///
    /// Deliberately not repeated here: the [`NativeScanner`] type-level
    /// example is the `scan` → `next_batch` lifecycle, and a second copy
    /// would be a second place for it to rot.
    pub fn scan(&self) -> Result<usize> {
        let mut builder = WalkBuilder::new(&self.root);
        builder
            // `dot = false` (the default) means "skip hidden", which is
            // `hidden(true)`. The inversion is the whole reason this option is
            // not passed straight through.
            .hidden(!self.options.dot)
            .parents(self.options.gitignore)
            .git_ignore(self.options.gitignore)
            .git_global(self.options.gitignore)
            .git_exclude(self.options.gitignore)
            .ignore(self.options.gitignore)
            // Without this, `ignore` applies .gitignore rules only inside a git
            // repository. A walk of an extracted tarball that ships a
            // .gitignore would silently not honour it, which is not what
            // `gitignore: true` says.
            .require_git(false)
            .follow_links(false)
            .max_depth(self.options.max_depth);

        if let Some(threads) = self.options.concurrency {
            builder.threads(threads.max(1));
        }

        // Pre-descent pruning (AC #5): an excluded directory is never descended
        // into, as opposed to being walked and then filtered out of the
        // results. On a tree with node_modules that is the difference between
        // reading a hundred thousand inodes and reading none.
        if !self.options.exclude.is_empty() {
            let excluded: Arc<HashSet<String>> =
                Arc::new(self.options.exclude.iter().cloned().collect());
            builder.filter_entry(move |entry| {
                // Depth 0 is the root itself. Pruning it would make a walk of
                // `target/` with `exclude: ["target"]` return nothing, which
                // reads as a bug at every call site.
                if entry.depth() == 0 {
                    return true;
                }
                if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                    if let Some(name) = entry.file_name().to_str() {
                        return !excluded.contains(name);
                    }
                    // A non-UTF-8 directory name cannot equal any exclusion,
                    // which are `String`s. Documented parity with libuv's lossy
                    // handling: it is walked, not pruned.
                }
                true
            });
        }

        builder.build_parallel().run(|| {
            let mut sink = Sink::new(Arc::clone(&self.results), Arc::clone(&self.errors));
            let cancelled = Arc::clone(&self.cancelled);
            let matcher = Arc::clone(&self.matcher);
            let root = self.root.clone();
            let options = self.options.clone();

            Box::new(move |entry| {
                if cancelled.load(Ordering::Relaxed) {
                    return WalkState::Quit;
                }

                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        // Collected, never silently dropped. `Err(_) => continue`
                        // is what the 2026 audit removed.
                        sink.errors.push(error.to_string());
                        return WalkState::Continue;
                    }
                };

                if let Some(built) = build_entry(&entry, &root, &matcher, &options, &mut sink) {
                    sink.entries.push(built);
                }

                WalkState::Continue
            })
        });

        self.scanned.store(true, Ordering::Release);
        Ok(lock(&self.results).len())
    }

    /// Whether [`scan`](Self::scan) has completed.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::walk::{NativeScanner, ScanOptions};
    ///
    /// let dir = tempfile::tempdir()?;
    /// let scanner = NativeScanner::new(dir.path(), ScanOptions::default())?;
    ///
    /// assert!(!scanner.has_scanned());
    /// scanner.scan()?;
    /// assert!(scanner.has_scanned());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn has_scanned(&self) -> bool {
        self.scanned.load(Ordering::Acquire)
    }

    /// Moves the next batch out of the scanner.
    ///
    /// Returns an empty vector once the results are exhausted. Entries are
    /// *moved*, not copied — see this module's header for why there is no way
    /// to rewind.
    ///
    /// # Examples
    ///
    /// Deliberately not repeated here: the [`NativeScanner`] type-level
    /// example drains a batch and asserts the exhaustion sentinel.
    #[must_use]
    pub fn next_batch(&self) -> Vec<FusedEntry> {
        let size = self.options.batch_size.max(1);
        let mut results = lock(&self.results);
        let take = size.min(results.len());
        results.drain(..take).collect()
    }

    /// Signals the walk to stop at the next entry.
    ///
    /// An `AtomicBool` checked per entry, which is what task-3 wires a JS
    /// `AbortSignal` to. A cancelled walk is not an error: it yields what it
    /// had collected when the flag was seen.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::walk::{NativeScanner, ScanOptions};
    ///
    /// let dir = tempfile::tempdir()?;
    /// std::fs::write(dir.path().join("a.ts"), b"")?;
    /// let scanner = NativeScanner::new(dir.path(), ScanOptions::default())?;
    ///
    /// scanner.cancel();
    /// assert!(scanner.is_cancelled());
    ///
    /// // Cancelled before the walk began: nothing is collected, no error.
    /// assert_eq!(scanner.scan()?, 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Whether cancellation has been requested.
    ///
    /// # Examples
    ///
    /// Deliberately not repeated here: the [`cancel`](Self::cancel) example
    /// asserts both sides of the flag.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// The traversal and hash failures this walk collected, capped at
    /// [`MAX_REPORTED_ERRORS`].
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::walk::{NativeScanner, ScanOptions};
    ///
    /// let dir = tempfile::tempdir()?;
    /// std::fs::write(dir.path().join("a.ts"), b"")?;
    /// let scanner = NativeScanner::new(dir.path(), ScanOptions::default())?;
    ///
    /// scanner.scan()?;
    /// assert!(scanner.errors().is_empty(), "a clean walk reports nothing");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn errors(&self) -> Vec<String> {
        lock(&self.errors).clone()
    }
}

/// A per-worker buffer that flushes into the shared collections when the
/// worker's visitor is dropped.
///
/// Locking the shared `Mutex` once per entry would serialise the walk on it and
/// undo the parallelism: the lock would be taken a hundred thousand times for a
/// hundred-thousand-file tree. `ignore` constructs one visitor per worker
/// thread and drops it when that worker finishes, so one flush per thread is
/// enough — and `Drop` is the only hook available, because `FnMut` cannot be
/// implemented by hand on stable Rust.
struct Sink {
    entries: Vec<FusedEntry>,
    errors: Vec<String>,
    shared_entries: Arc<Mutex<VecDeque<FusedEntry>>>,
    shared_errors: Arc<Mutex<Vec<String>>>,
}

impl Sink {
    fn new(
        shared_entries: Arc<Mutex<VecDeque<FusedEntry>>>,
        shared_errors: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self {
            entries: Vec::new(),
            errors: Vec::new(),
            shared_entries,
            shared_errors,
        }
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        if !self.entries.is_empty() {
            lock(&self.shared_entries).extend(self.entries.drain(..));
        }
        if !self.errors.is_empty() {
            let mut shared = lock(&self.shared_errors);
            let room = MAX_REPORTED_ERRORS.saturating_sub(shared.len());
            shared.extend(self.errors.drain(..).take(room));
        }
    }
}

/// Hashes a tree using one fused traversal and a canonical, sorted encoding.
///
/// Each file contributes its root-relative path length, path bytes, and content
/// digest. Sorting before encoding makes the result independent of parallel
/// walker scheduling and filesystem enumeration order.
///
/// # Errors
///
/// Returns an error if the root cannot be traversed, an entry cannot be
/// hashed, or the final canonical stream cannot be read.
pub fn hash_tree(
    root: impl AsRef<Path>,
    mut options: ScanOptions,
    algorithm: Algorithm,
) -> Result<String> {
    options.hash = Some(algorithm);
    let scanner = NativeScanner::new(root, options)?;
    scanner.scan()?;
    if let Some(error) = scanner.errors().into_iter().next() {
        return Err(Error::io(
            "walk",
            scanner.root(),
            std::io::Error::other(error),
        ));
    }

    let mut entries = Vec::new();
    loop {
        let batch = scanner.next_batch();
        if batch.is_empty() {
            break;
        }
        entries.extend(batch.into_iter().filter(|entry| entry.error.is_none()));
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));

    let mut canonical = Vec::new();
    for entry in entries {
        let path = entry.path.to_string_lossy();
        canonical.extend((path.len() as u64).to_le_bytes());
        canonical.extend(path.as_bytes());
        let digest = entry.hash.unwrap_or_default();
        canonical.extend((digest.len() as u64).to_le_bytes());
        canonical.extend(digest.as_bytes());
    }
    hash_reader(canonical.as_slice(), algorithm, scanner.root())
}

/// Takes a lock, treating poisoning as recoverable.
///
/// A panic in one worker must not turn every later access into a second panic:
/// the data behind these locks is a plain collection of results, so the worst a
/// poisoned lock means here is that one worker's batch is incomplete.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Turns one `ignore` entry into a [`FusedEntry`], or `None` if it is filtered
/// out.
///
/// This is the fused part: the filter, the `stat` and the hash all happen here,
/// on the worker thread that produced the entry, in one visit.
fn build_entry(
    entry: &ignore::DirEntry,
    root: &Path,
    matcher: &Matcher,
    options: &ScanOptions,
    sink: &mut Sink,
) -> Option<FusedEntry> {
    let file_type = entry.file_type()?;
    let is_dir = file_type.is_dir();
    let is_file = file_type.is_file();

    if options.files_only && !is_file {
        return None;
    }

    let absolute = entry.path();
    // The walk root itself has an empty relative path and is not a result.
    let relative = absolute.strip_prefix(root).unwrap_or(absolute);
    if relative.as_os_str().is_empty() {
        return None;
    }

    if !matcher.is_unfiltered() && !matcher.accepts(relative, absolute) {
        return None;
    }

    let mut size = None;
    let mut modified_nanos = None;
    let mut error = None;

    if options.with_metadata {
        match entry.metadata() {
            Ok(metadata) => {
                size = Some(metadata.len());
                modified_nanos = metadata.modified().ok().map(to_unix_nanos);
            }
            Err(source) => {
                sink.errors
                    .push(format!("{}: {source}", absolute.display()));
                error = Some(Error::io(
                    "stat",
                    absolute,
                    std::io::Error::other(source.to_string()),
                ));
            }
        }
    }

    // Chunked, never a whole-file read — see `crate::hash`.
    let mut hash = None;
    if let Some(algorithm) = options.hash {
        if is_file {
            match hash_file(absolute, algorithm) {
                Ok(digest) => hash = Some(digest),
                Err(failure) => {
                    sink.errors.push(failure.to_string());
                    error = Some(failure);
                }
            }
        }
    }

    Some(FusedEntry {
        path: if options.absolute {
            absolute.to_path_buf()
        } else {
            relative.to_path_buf()
        },
        is_dir,
        size,
        modified_nanos,
        hash,
        error,
    })
}

/// Nanoseconds since the Unix epoch, negative for times before it.
///
/// Nanoseconds and not milliseconds: two files written inside the same
/// millisecond are indistinguishable in a millisecond-precision snapshot, which
/// silently breaks incremental builds. The JS surface narrows this to `Date`
/// range at the boundary; the core keeps what the filesystem gave it.
fn to_unix_nanos(time: SystemTime) -> i128 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(since) => i128::try_from(since.as_nanos()).unwrap_or(i128::MAX),
        Err(before) => -i128::try_from(before.duration().as_nanos()).unwrap_or(i128::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::{hash_tree, NativeScanner, ScanOptions, DEFAULT_BATCH_SIZE};
    use crate::hash::Algorithm;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    /// Builds a tree and returns its temp dir, which must stay alive.
    fn tree(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for file in files {
            let path = dir.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, format!("contents of {file}")).unwrap();
        }
        dir
    }

    /// Drains every batch, sorted — the walk is parallel, so order is
    /// unspecified and an order-dependent assertion would be flaky.
    fn collect(scanner: &NativeScanner) -> Vec<PathBuf> {
        let mut all = Vec::new();
        loop {
            let batch = scanner.next_batch();
            if batch.is_empty() {
                break;
            }
            all.extend(batch.into_iter().map(|entry| entry.path));
        }
        all.sort();
        all
    }

    fn paths(expected: &[&str]) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = expected.iter().map(PathBuf::from).collect();
        out.sort();
        out
    }

    fn scan(dir: &Path, options: ScanOptions) -> (NativeScanner, Vec<PathBuf>) {
        let scanner = NativeScanner::new(dir, options).unwrap();
        scanner.scan().unwrap();
        let found = collect(&scanner);
        (scanner, found)
    }

    // ── traversal basics ────────────────────────────────────────────────────

    #[test]
    fn a_default_walk_yields_every_file_root_relative() {
        let dir = tree(&["a.ts", "src/b.ts", "src/deep/c.ts"]);
        let (_scanner, found) = scan(dir.path(), ScanOptions::default());

        assert_eq!(found, paths(&["a.ts", "src/b.ts", "src/deep/c.ts"]));
    }

    #[test]
    fn files_only_is_the_default_and_directories_can_be_asked_for() {
        let dir = tree(&["src/b.ts"]);

        let (_s, files) = scan(dir.path(), ScanOptions::default());
        assert_eq!(files, paths(&["src/b.ts"]));

        let (_s, everything) = scan(
            dir.path(),
            ScanOptions {
                files_only: false,
                ..ScanOptions::default()
            },
        );
        assert_eq!(everything, paths(&["src", "src/b.ts"]));
    }

    /// The root is never yielded as an entry of its own: its relative path is
    /// empty, which would surface as a `""` result.
    #[test]
    fn the_walk_root_is_not_one_of_its_own_results() {
        let dir = tree(&["a.ts"]);
        let (_s, found) = scan(
            dir.path(),
            ScanOptions {
                files_only: false,
                ..ScanOptions::default()
            },
        );

        assert!(!found.iter().any(|p| p.as_os_str().is_empty()));
        assert_eq!(found, paths(&["a.ts"]));
    }

    #[test]
    fn absolute_yields_full_paths_and_is_not_the_default() {
        let dir = tree(&["a.ts"]);

        let (_s, relative) = scan(dir.path(), ScanOptions::default());
        assert_eq!(relative, paths(&["a.ts"]));

        let scanner = NativeScanner::new(
            dir.path(),
            ScanOptions {
                absolute: true,
                ..ScanOptions::default()
            },
        )
        .unwrap();
        scanner.scan().unwrap();
        let found = collect(&scanner);

        assert_eq!(found.len(), 1);
        assert!(found[0].is_absolute(), "{:?}", found[0]);
        assert_eq!(found[0], scanner.root().join("a.ts"));
    }

    #[test]
    fn max_depth_counts_the_root_as_zero() {
        let dir = tree(&["a.ts", "one/b.ts", "one/two/c.ts"]);

        let (_s, depth_one) = scan(
            dir.path(),
            ScanOptions {
                max_depth: Some(1),
                ..ScanOptions::default()
            },
        );
        assert_eq!(depth_one, paths(&["a.ts"]));

        let (_s, depth_two) = scan(
            dir.path(),
            ScanOptions {
                max_depth: Some(2),
                ..ScanOptions::default()
            },
        );
        assert_eq!(depth_two, paths(&["a.ts", "one/b.ts"]));
    }

    // ── dotfiles and ignore files (AC #6) ───────────────────────────────────

    #[test]
    fn dotfiles_are_skipped_unless_asked_for() {
        let dir = tree(&["a.ts", ".env", ".config/b.ts"]);

        let (_s, without) = scan(dir.path(), ScanOptions::default());
        assert_eq!(without, paths(&["a.ts"]));

        let (_s, with) = scan(
            dir.path(),
            ScanOptions {
                dot: true,
                ..ScanOptions::default()
            },
        );
        assert_eq!(with, paths(&[".config/b.ts", ".env", "a.ts"]));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_reported_without_following_them_outside_the_root() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"secret").unwrap();
        symlink(outside.path(), root.path().join("outside")).unwrap();

        let scanner = NativeScanner::new(
            root.path(),
            ScanOptions {
                files_only: false,
                ..ScanOptions::default()
            },
        )
        .unwrap();
        scanner.scan().unwrap();
        let found = collect(&scanner);

        assert_eq!(found, paths(&["outside"]));
        assert!(scanner.errors().is_empty());
    }

    /// `gitignore: true` must work outside a git repository — a walk of an
    /// extracted tarball that ships a `.gitignore` is the ordinary case, and
    /// `ignore` requires `require_git(false)` for it.
    #[test]
    fn gitignore_is_opt_in_and_does_not_need_a_git_repository() {
        let dir = tree(&["a.ts", "build/out.js"]);
        std::fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();

        let (_s, ignored_off) = scan(dir.path(), ScanOptions::default());
        assert_eq!(ignored_off, paths(&["a.ts", "build/out.js"]));

        let (_s, ignored_on) = scan(
            dir.path(),
            ScanOptions {
                gitignore: true,
                ..ScanOptions::default()
            },
        );
        assert_eq!(ignored_on, paths(&["a.ts"]));
    }

    // ── pruning (AC #5) ─────────────────────────────────────────────────────

    #[test]
    fn an_excluded_directory_is_absent_from_the_results() {
        let dir = tree(&["src/a.ts", "node_modules/pkg/index.js"]);
        let (_s, found) = scan(
            dir.path(),
            ScanOptions {
                exclude: vec!["node_modules".to_owned()],
                ..ScanOptions::default()
            },
        );

        assert_eq!(found, paths(&["src/a.ts"]));
    }

    /// Absence from the results is also what post-filtering would produce, so
    /// it does not prove pruning. This does: an unreadable directory yields an
    /// `EACCES` the moment anything tries to descend into it, so a *pruned*
    /// walk reports no errors and a post-filtered one reports an error it
    /// could not have avoided.
    #[test]
    fn exclusion_prunes_before_descending_rather_than_filtering_after() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tree(&["src/a.ts", "secret/hidden.ts"]);
        let secret = dir.path().join("secret");
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();

        // Root ignores directory permissions, which would make this vacuous.
        let enforced = std::fs::read_dir(&secret).is_err();

        let pruned = NativeScanner::new(
            dir.path(),
            ScanOptions {
                exclude: vec!["secret".to_owned()],
                ..ScanOptions::default()
            },
        )
        .unwrap();
        pruned.scan().unwrap();
        let pruned_paths = collect(&pruned);
        let pruned_errors = pruned.errors();

        if enforced {
            let descended = NativeScanner::new(dir.path(), ScanOptions::default()).unwrap();
            descended.scan().unwrap();
            let _ = collect(&descended);
            assert!(
                !descended.errors().is_empty(),
                "the unreadable directory should fail a walk that descends into it"
            );
        }

        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(pruned_paths, paths(&["src/a.ts"]));
        assert!(
            pruned_errors.is_empty(),
            "a pruned walk never touched it: {pruned_errors:?}"
        );
    }

    #[test]
    fn excluding_the_root_by_name_does_not_empty_the_walk() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("target");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.ts"), "x").unwrap();

        let (_s, found) = scan(
            &root,
            ScanOptions {
                exclude: vec!["target".to_owned()],
                ..ScanOptions::default()
            },
        );

        assert_eq!(found, paths(&["a.ts"]));
    }

    // ── filtering, through the scanner (AC #2, #4) ──────────────────────────

    #[test]
    fn globs_are_matched_root_relative_through_the_scanner() {
        let dir = tree(&["a.ts", "src/b.ts", "src/c.rs"]);
        let (_s, found) = scan(
            dir.path(),
            ScanOptions {
                glob: vec!["**/*.ts".to_owned()],
                ..ScanOptions::default()
            },
        );

        assert_eq!(found, paths(&["a.ts", "src/b.ts"]));
    }

    #[test]
    fn the_regex_is_matched_against_the_absolute_path() {
        let dir = tree(&["src/a.ts", "lib/a.ts"]);
        let absolute_src = format!(
            "^{}/src/",
            regex::escape(&dir.path().canonicalize().unwrap().to_string_lossy())
        );

        let (_s, found) = scan(
            dir.path(),
            ScanOptions {
                regex: Some(absolute_src),
                ..ScanOptions::default()
            },
        );

        assert_eq!(found, paths(&["src/a.ts"]));
    }

    #[test]
    fn a_bad_pattern_fails_before_the_walk_starts() {
        let dir = tree(&["a.ts"]);
        let error = NativeScanner::new(
            dir.path(),
            ScanOptions {
                regex: Some("(unclosed".to_owned()),
                ..ScanOptions::default()
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("(unclosed"));
    }

    // ── metadata (AC #9) ────────────────────────────────────────────────────

    #[test]
    fn metadata_is_absent_unless_requested() {
        let dir = tree(&["a.ts"]);

        let scanner = NativeScanner::new(dir.path(), ScanOptions::default()).unwrap();
        scanner.scan().unwrap();
        let batch = scanner.next_batch();
        assert_eq!(batch[0].size, None);
        assert_eq!(batch[0].modified_nanos, None);

        let scanner = NativeScanner::new(
            dir.path(),
            ScanOptions {
                with_metadata: true,
                ..ScanOptions::default()
            },
        )
        .unwrap();
        scanner.scan().unwrap();
        let batch = scanner.next_batch();
        assert_eq!(batch[0].size, Some("contents of a.ts".len() as u64));
        assert!(batch[0].modified_nanos.unwrap() > 0);
    }

    /// Nanoseconds, not milliseconds — a millisecond-precision mtime cannot
    /// distinguish two files written in the same millisecond, which is what
    /// breaks incremental builds.
    #[test]
    fn modification_time_is_recorded_in_nanoseconds() {
        let dir = tree(&["a.ts"]);
        let scanner = NativeScanner::new(
            dir.path(),
            ScanOptions {
                with_metadata: true,
                ..ScanOptions::default()
            },
        )
        .unwrap();
        scanner.scan().unwrap();

        let nanos = scanner.next_batch()[0].modified_nanos.unwrap();
        let as_millis = nanos / 1_000_000;
        // A plausible wall clock, and far larger than a millisecond count
        // would be if the units had been confused.
        assert!(as_millis > 1_600_000_000_000, "{nanos}");
    }

    // ── hashing (AC #10) ────────────────────────────────────────────────────

    #[test]
    fn hashing_happens_during_the_walk_and_matches_a_standalone_hash() {
        let dir = tree(&["a.ts", "src/b.ts"]);
        let scanner = NativeScanner::new(
            dir.path(),
            ScanOptions {
                hash: Some(Algorithm::Blake3),
                ..ScanOptions::default()
            },
        )
        .unwrap();
        scanner.scan().unwrap();

        for entry in scanner.next_batch() {
            let expected =
                crate::hash::hash_file(&scanner.root().join(&entry.path), Algorithm::Blake3)
                    .unwrap();
            assert_eq!(entry.hash.as_deref(), Some(expected.as_str()));
        }
    }

    #[test]
    fn a_hash_is_absent_rather_than_empty_when_not_requested() {
        let dir = tree(&["a.ts"]);
        let scanner = NativeScanner::new(dir.path(), ScanOptions::default()).unwrap();
        scanner.scan().unwrap();

        assert_eq!(scanner.next_batch()[0].hash, None);
    }

    #[test]
    fn every_algorithm_is_reachable_through_the_walk() {
        let dir = tree(&["a.ts"]);
        let mut digests = BTreeSet::new();

        for algorithm in [Algorithm::Blake3, Algorithm::Xxhash, Algorithm::Sha256] {
            let scanner = NativeScanner::new(
                dir.path(),
                ScanOptions {
                    hash: Some(algorithm),
                    ..ScanOptions::default()
                },
            )
            .unwrap();
            scanner.scan().unwrap();
            digests.insert(scanner.next_batch()[0].hash.clone().unwrap());
        }

        assert_eq!(digests.len(), 3, "three algorithms, three distinct digests");
    }

    // ── errors are collected, not fatal (AC #11) ────────────────────────────

    #[test]
    fn an_unreadable_file_is_reported_without_aborting_the_walk() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tree(&["readable.ts", "locked.ts"]);
        let locked = dir.path().join("locked.ts");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

        if std::fs::File::open(&locked).is_ok() {
            return; // running as root; the premise does not hold
        }

        let scanner = NativeScanner::new(
            dir.path(),
            ScanOptions {
                hash: Some(Algorithm::Blake3),
                ..ScanOptions::default()
            },
        )
        .unwrap();
        scanner.scan().unwrap();
        let batch = scanner.next_batch();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();

        // Both files are still yielded: the walk did not abort, and the entry
        // that failed is present with its path intact.
        assert_eq!(batch.len(), 2, "the walk kept going");

        let failed = batch
            .iter()
            .find(|e| e.path == Path::new("locked.ts"))
            .expect("the unreadable entry is still yielded");
        assert!(failed.error.is_some(), "it carries its own failure");
        assert!(failed.hash.is_none(), "and no hash");

        let ok = batch
            .iter()
            .find(|e| e.path == Path::new("readable.ts"))
            .unwrap();
        assert!(ok.hash.is_some(), "the readable one is unaffected");

        assert!(!scanner.errors().is_empty(), "and the walk reports it");
    }

    // ── batching (AC #8) ────────────────────────────────────────────────────

    #[test]
    fn the_default_batch_size_is_five_hundred_and_twelve() {
        assert_eq!(DEFAULT_BATCH_SIZE, 512);
        assert_eq!(ScanOptions::default().batch_size, DEFAULT_BATCH_SIZE);
    }

    #[test]
    fn results_are_yielded_in_batches_until_exhausted() {
        let files: Vec<String> = (0..5).map(|i| format!("f{i}.ts")).collect();
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        let dir = tree(&refs);

        let scanner = NativeScanner::new(
            dir.path(),
            ScanOptions {
                batch_size: 2,
                ..ScanOptions::default()
            },
        )
        .unwrap();
        assert_eq!(scanner.scan().unwrap(), 5);

        let sizes: Vec<usize> = std::iter::repeat_with(|| scanner.next_batch().len())
            .take_while(|len| *len > 0)
            .collect();

        assert_eq!(sizes, vec![2, 2, 1]);
        assert!(scanner.next_batch().is_empty(), "and stays empty");
    }

    /// The reference kept a cursor *and* drained, which skips half the results
    /// from the second batch on. Five files in batches of two must yield five
    /// distinct paths.
    #[test]
    fn draining_batches_loses_no_entries() {
        let files: Vec<String> = (0..5).map(|i| format!("f{i}.ts")).collect();
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        let dir = tree(&refs);

        let scanner = NativeScanner::new(
            dir.path(),
            ScanOptions {
                batch_size: 2,
                ..ScanOptions::default()
            },
        )
        .unwrap();
        scanner.scan().unwrap();

        let found: BTreeSet<PathBuf> = collect(&scanner).into_iter().collect();
        assert_eq!(found.len(), 5, "{found:?}");
    }

    #[test]
    fn tree_hash_is_stable_across_parallel_traversals_and_file_order() {
        let dir = tree(&["z.txt", "a.txt", "nested/m.txt"]);
        let first = hash_tree(dir.path(), ScanOptions::default(), Algorithm::Sha256).unwrap();
        let second = hash_tree(
            dir.path(),
            ScanOptions {
                concurrency: Some(1),
                ..ScanOptions::default()
            },
            Algorithm::Sha256,
        )
        .unwrap();
        assert_eq!(first, second);

        std::fs::write(dir.path().join("a.txt"), "changed").unwrap();
        let changed = hash_tree(dir.path(), ScanOptions::default(), Algorithm::Sha256).unwrap();
        assert_ne!(first, changed);
    }

    // ── cancellation (AC #12) ───────────────────────────────────────────────

    #[test]
    fn a_cancelled_walk_collects_nothing_and_is_not_an_error() {
        let dir = tree(&["a.ts", "b.ts", "src/c.ts"]);
        let scanner = NativeScanner::new(dir.path(), ScanOptions::default()).unwrap();

        scanner.cancel();
        assert!(scanner.is_cancelled());

        let count = scanner.scan().expect("cancellation is not a failure");
        assert_eq!(count, 0);
        assert!(scanner.next_batch().is_empty());
    }

    #[test]
    fn has_scanned_reports_whether_the_walk_ran() {
        let dir = tree(&["a.ts"]);
        let scanner = NativeScanner::new(dir.path(), ScanOptions::default()).unwrap();

        assert!(!scanner.has_scanned());
        scanner.scan().unwrap();
        assert!(scanner.has_scanned());
    }

    // ── concurrency ─────────────────────────────────────────────────────────

    /// The `concurrency` option has to be real: the reference set `threads()`
    /// but called the single-threaded `build()`, where it does nothing.
    #[test]
    fn a_walk_finds_the_same_tree_at_every_concurrency() {
        let files: Vec<String> = (0..40).map(|i| format!("d{}/f{i}.ts", i % 7)).collect();
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        let dir = tree(&refs);

        let baseline = scan(dir.path(), ScanOptions::default()).1;
        assert_eq!(baseline.len(), 40);

        for threads in [1, 2, 8] {
            let (_s, found) = scan(
                dir.path(),
                ScanOptions {
                    concurrency: Some(threads),
                    ..ScanOptions::default()
                },
            );
            assert_eq!(found, baseline, "with {threads} threads");
        }
    }
}
