//! Directory snapshots: capture, persist, diff.
//!
//! A snapshot is one fused walk folded into a sorted, comparable value. The
//! point of putting it here rather than in either surface is that *the fold is
//! the format*: a diff is only meaningful if two snapshots taken on different
//! machines, by different surfaces, at different times agree byte for byte on
//! what "the same tree" looks like. Two implementations of that rule would be
//! two formats.
//!
//! ## Why nanoseconds, and why as strings on the wire
//!
//! Build systems write files inside the same millisecond, so a millisecond
//! snapshot reports "unchanged" for a file that changed — the failure mode is
//! a stale incremental build, which is worse than a slow one. The core keeps
//! `i128` nanoseconds end to end, and the persisted document writes them as
//! decimal *strings*: a JSON number is an IEEE-754 double in every mainstream
//! parser, and 2026 in nanoseconds does not fit in 53 bits of mantissa.
//!
//! ## Why sorted
//!
//! `ignore` traverses in parallel, so capture order varies by thread
//! scheduling. Entries are held in a [`BTreeMap`] keyed by the slash-normalised
//! relative path, so `to_json` emits the same bytes for the same tree on every
//! run and on every platform. A build cache keyed by a snapshot is only
//! correct if that holds.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::walk::{NativeScanner, ScanOptions};

/// The `format` tag written into every persisted snapshot.
///
/// Read before anything else on load: a document without it, or with a
/// different one, is refused rather than best-effort decoded into a diff that
/// would silently claim the whole tree changed.
///
/// # Examples
///
/// ```
/// assert_eq!(pathway_fs_core::snapshot::FORMAT, "pathway-snapshot-v1");
/// ```
pub const FORMAT: &str = "pathway-snapshot-v1";

/// What a snapshot records about one file.
///
/// `hash` is optional because hashing is opt-in: a snapshot taken without it
/// is still useful (and much cheaper), it just compares on `size` and
/// `modified_nanos` instead of on content.
///
/// # Examples
///
/// ```
/// use pathway_fs_core::snapshot::SnapshotEntry;
///
/// let entry = SnapshotEntry { size: 12, modified_nanos: 1_700_000_000_123_456_789, hash: None };
/// assert_eq!(entry.size, 12);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotEntry {
    /// Size in bytes at capture time.
    pub size: u64,
    /// Modification time in nanoseconds since the Unix epoch.
    ///
    /// `i128` rather than `u64`: a pre-epoch mtime is unusual but legal, and a
    /// wrapped negative timestamp would read as "modified in the far future",
    /// which no diff can recover from.
    pub modified_nanos: i128,
    /// Lowercase hex content digest, when the capture asked for one.
    pub hash: Option<String>,
}

impl SnapshotEntry {
    /// Whether this entry and `other` describe the same content.
    ///
    /// Content wins when both sides have it: two snapshots that carry hashes
    /// compare hashes, so a file rewritten with identical bytes is *unchanged*
    /// even though its mtime moved. When either side lacks a hash the
    /// comparison falls back to `size` plus `modified_nanos`, because a
    /// hash-to-nothing comparison has no defensible answer.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::SnapshotEntry;
    ///
    /// let touched = SnapshotEntry { size: 3, modified_nanos: 1, hash: Some("ab".into()) };
    /// let rewritten = SnapshotEntry { size: 3, modified_nanos: 999, hash: Some("ab".into()) };
    /// assert!(touched.same_content_as(&rewritten), "identical bytes, newer mtime");
    ///
    /// let unhashed = SnapshotEntry { size: 3, modified_nanos: 999, hash: None };
    /// assert!(!touched.same_content_as(&unhashed), "falls back to size + mtime");
    /// ```
    #[must_use]
    pub fn same_content_as(&self, other: &Self) -> bool {
        match (&self.hash, &other.hash) {
            (Some(left), Some(right)) => left == right,
            _ => self.size == other.size && self.modified_nanos == other.modified_nanos,
        }
    }
}

/// A directory tree as one fused walk saw it.
///
/// # Examples
///
/// ```
/// use pathway_fs_core::snapshot::Snapshot;
/// use pathway_fs_core::walk::ScanOptions;
///
/// let dir = tempfile::tempdir()?;
/// std::fs::write(dir.path().join("a.ts"), b"export {};")?;
///
/// let snapshot = Snapshot::capture(dir.path(), ScanOptions::default())?;
/// assert_eq!(snapshot.paths(), vec!["a.ts"]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    root: PathBuf,
    taken_at_nanos: i128,
    entries: BTreeMap<String, SnapshotEntry>,
}

impl Snapshot {
    /// Walks `root` and folds the result into a snapshot.
    ///
    /// The walk is the existing fused one, so every filter a walk understands
    /// — globs, regex, exclusions, `.gitignore`, hashing — configures a
    /// snapshot too. `files_only` and `with_metadata` are forced on: a
    /// directory has no content to compare and a snapshot without `stat` data
    /// could not diff at all.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the root cannot be resolved or the walk reported a
    /// traversal failure, [`Error::Glob`] / [`Error::Regex`] for a bad
    /// pattern. A walk that failed on individual entries fails the whole
    /// capture: a snapshot silently missing the files it could not read would
    /// diff them as *removed*.
    ///
    /// # Examples
    ///
    /// Deliberately not repeated: the [`Snapshot`] type-level example is a
    /// capture.
    pub fn capture(root: impl AsRef<Path>, options: ScanOptions) -> Result<Self> {
        let options = ScanOptions {
            files_only: true,
            with_metadata: true,
            ..options
        };
        let scanner = NativeScanner::new(root, options)?;
        scanner.scan()?;
        if let Some(message) = scanner.errors().into_iter().next() {
            return Err(Error::io(
                "walk",
                scanner.root(),
                std::io::Error::other(message),
            ));
        }

        let mut entries = BTreeMap::new();
        loop {
            let batch = scanner.next_batch();
            if batch.is_empty() {
                break;
            }
            for entry in batch {
                if entry.error.is_some() || entry.is_dir {
                    continue;
                }
                entries.insert(
                    normalise(&entry.path),
                    SnapshotEntry {
                        size: entry.size.unwrap_or_default(),
                        modified_nanos: entry.modified_nanos.unwrap_or_default(),
                        hash: entry.hash,
                    },
                );
            }
        }

        Ok(Self {
            root: scanner.root().to_path_buf(),
            taken_at_nanos: now_nanos(),
            entries,
        })
    }

    /// The canonical root the snapshot was taken of.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::Snapshot;
    /// use pathway_fs_core::walk::ScanOptions;
    ///
    /// let dir = tempfile::tempdir()?;
    /// let snapshot = Snapshot::capture(dir.path(), ScanOptions::default())?;
    /// assert_eq!(snapshot.root(), std::fs::canonicalize(dir.path())?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// When the capture finished, in nanoseconds since the Unix epoch.
    ///
    /// Recorded for the caller's bookkeeping only. It is deliberately *not*
    /// part of a diff: two snapshots of an unchanged tree taken a day apart
    /// are equal in every way that matters to a build cache.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::Snapshot;
    /// use pathway_fs_core::walk::ScanOptions;
    ///
    /// let dir = tempfile::tempdir()?;
    /// let snapshot = Snapshot::capture(dir.path(), ScanOptions::default())?;
    /// assert!(snapshot.taken_at_nanos() > 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn taken_at_nanos(&self) -> i128 {
        self.taken_at_nanos
    }

    /// Every recorded entry, keyed by slash-normalised relative path, in
    /// sorted order.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::Snapshot;
    /// use pathway_fs_core::walk::ScanOptions;
    ///
    /// let dir = tempfile::tempdir()?;
    /// std::fs::write(dir.path().join("a.ts"), b"x")?;
    /// let snapshot = Snapshot::capture(dir.path(), ScanOptions::default())?;
    ///
    /// assert_eq!(snapshot.entries()["a.ts"].size, 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn entries(&self) -> &BTreeMap<String, SnapshotEntry> {
        &self.entries
    }

    /// The recorded paths, sorted.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::Snapshot;
    /// use pathway_fs_core::walk::ScanOptions;
    ///
    /// let dir = tempfile::tempdir()?;
    /// std::fs::write(dir.path().join("b.ts"), b"x")?;
    /// std::fs::write(dir.path().join("a.ts"), b"x")?;
    /// let snapshot = Snapshot::capture(dir.path(), ScanOptions::default())?;
    ///
    /// // Sorted, not in traversal order: `ignore` walks in parallel.
    /// assert_eq!(snapshot.paths(), vec!["a.ts", "b.ts"]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn paths(&self) -> Vec<&str> {
        self.entries.keys().map(String::as_str).collect()
    }

    /// Compares this snapshot (the *before*) with `other` (the *after*).
    ///
    /// Each bucket is sorted, and every path appears in exactly one of them.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::{Snapshot, SnapshotDiff};
    /// use pathway_fs_core::walk::ScanOptions;
    ///
    /// let dir = tempfile::tempdir()?;
    /// std::fs::write(dir.path().join("kept.ts"), b"x")?;
    /// let before = Snapshot::capture(dir.path(), ScanOptions::default())?;
    ///
    /// std::fs::write(dir.path().join("new.ts"), b"y")?;
    /// let after = Snapshot::capture(dir.path(), ScanOptions::default())?;
    ///
    /// let diff: SnapshotDiff = before.diff(&after);
    /// assert_eq!(diff.added, vec!["new.ts"]);
    /// assert_eq!(diff.unchanged, vec!["kept.ts"]);
    /// assert!(diff.removed.is_empty() && diff.modified.is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn diff(&self, other: &Self) -> SnapshotDiff {
        let mut diff = SnapshotDiff::default();
        for (path, before) in &self.entries {
            match other.entries.get(path) {
                None => diff.removed.push(path.clone()),
                Some(after) if before.same_content_as(after) => diff.unchanged.push(path.clone()),
                Some(_) => diff.modified.push(path.clone()),
            }
        }
        for path in other.entries.keys() {
            if !self.entries.contains_key(path) {
                diff.added.push(path.clone());
            }
        }
        diff
    }

    /// Renders the snapshot as its persisted JSON document.
    ///
    /// Deterministic: entries are emitted in sorted path order and nanosecond
    /// fields are decimal strings, so the same tree produces the same bytes on
    /// every run and every platform.
    ///
    /// # Errors
    ///
    /// [`Error::Codec`] if serialization fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::{Snapshot, FORMAT};
    /// use pathway_fs_core::walk::ScanOptions;
    ///
    /// let dir = tempfile::tempdir()?;
    /// let json = Snapshot::capture(dir.path(), ScanOptions::default())?.to_json()?;
    /// assert!(json.contains(FORMAT));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(&Document::from(self)).map_err(|error| Error::Codec {
            format: "snapshot",
            message: error.to_string(),
        })
    }

    /// Parses a persisted JSON document.
    ///
    /// # Errors
    ///
    /// [`Error::Codec`] if the payload is not valid JSON, carries a different
    /// `format` tag, or holds a nanosecond field that is not a decimal
    /// integer.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::Snapshot;
    /// use pathway_fs_core::walk::ScanOptions;
    ///
    /// let dir = tempfile::tempdir()?;
    /// std::fs::write(dir.path().join("a.ts"), b"x")?;
    /// let snapshot = Snapshot::capture(dir.path(), ScanOptions::default())?;
    ///
    /// let restored = Snapshot::from_json(&snapshot.to_json()?)?;
    /// assert_eq!(restored, snapshot);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_json(json: &str) -> Result<Self> {
        let document: Document = serde_json::from_str(json).map_err(|error| Error::Codec {
            format: "snapshot",
            message: error.to_string(),
        })?;
        if document.format != FORMAT {
            return Err(Error::Codec {
                format: "snapshot",
                message: format!(
                    "unsupported snapshot format {:?}; expected {FORMAT}",
                    document.format
                ),
            });
        }
        document.into_snapshot()
    }

    /// Writes the snapshot to `path` atomically.
    ///
    /// Atomic because a snapshot is a cache key: a half-written document that
    /// parses is worse than no document at all, since it diffs as a tree that
    /// never existed.
    ///
    /// # Errors
    ///
    /// [`Error::Codec`] if serialization fails, [`Error::Io`] if the write
    /// does.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::Snapshot;
    /// use pathway_fs_core::walk::ScanOptions;
    ///
    /// let dir = tempfile::tempdir()?;
    /// let snapshot = Snapshot::capture(dir.path(), ScanOptions::default())?;
    ///
    /// let file = dir.path().join("snapshot.json");
    /// snapshot.save(&file)?;
    /// assert_eq!(Snapshot::load(&file)?, snapshot);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        crate::fs::atomic::write_atomic(path.as_ref(), self.to_json()?.as_bytes())
    }

    /// Reads a snapshot written by [`save`](Self::save).
    ///
    /// # Errors
    ///
    /// [`Error::Io`] if the file cannot be read, [`Error::Codec`] if its
    /// contents are not a snapshot document of this format.
    ///
    /// # Examples
    ///
    /// Deliberately not repeated: the [`save`](Self::save) example is the
    /// round trip.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let json =
            std::fs::read_to_string(path).map_err(|source| Error::io("read", path, source))?;
        Self::from_json(&json)
    }
}

/// What changed between two snapshots.
///
/// Four buckets, every path in exactly one. `unchanged` is carried rather than
/// implied: an incremental build needs to know which outputs it may keep, and
/// recomputing that from the other three means re-walking.
///
/// # Examples
///
/// ```
/// use pathway_fs_core::snapshot::SnapshotDiff;
///
/// let diff = SnapshotDiff::default();
/// assert!(diff.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotDiff {
    /// Paths present only in the *after* snapshot.
    pub added: Vec<String>,
    /// Paths present only in the *before* snapshot.
    pub removed: Vec<String>,
    /// Paths in both whose content differs.
    pub modified: Vec<String>,
    /// Paths in both whose content is the same.
    pub unchanged: Vec<String>,
}

impl SnapshotDiff {
    /// Whether nothing at all was recorded on either side.
    ///
    /// Note this is *not* "nothing changed": a diff of two identical non-empty
    /// trees has a full `unchanged` bucket and is not empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::SnapshotDiff;
    ///
    /// let mut diff = SnapshotDiff::default();
    /// assert!(diff.is_empty());
    /// diff.unchanged.push("a.ts".to_owned());
    /// assert!(!diff.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.modified.is_empty()
            && self.unchanged.is_empty()
    }

    /// Whether the tree changed in any way.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::SnapshotDiff;
    ///
    /// let mut diff = SnapshotDiff::default();
    /// diff.unchanged.push("a.ts".to_owned());
    /// assert!(!diff.has_changes(), "unchanged entries are not changes");
    ///
    /// diff.modified.push("b.ts".to_owned());
    /// assert!(diff.has_changes());
    /// ```
    #[must_use]
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }

    /// Renders the diff as a JSON document.
    ///
    /// The shape the engine bridge hands to the TypeScript surface, which is
    /// why it lives here: the buckets are computed once, in the core, and both
    /// surfaces read the same four arrays.
    ///
    /// # Errors
    ///
    /// [`Error::Codec`] if serialization fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use pathway_fs_core::snapshot::SnapshotDiff;
    ///
    /// let mut diff = SnapshotDiff::default();
    /// diff.added.push("a.ts".to_owned());
    /// assert_eq!(
    ///     diff.to_json()?,
    ///     r#"{"added":["a.ts"],"removed":[],"modified":[],"unchanged":[]}"#
    /// );
    /// # Ok::<(), pathway_fs_core::error::Error>(())
    /// ```
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(&DiffDocument {
            added: &self.added,
            removed: &self.removed,
            modified: &self.modified,
            unchanged: &self.unchanged,
        })
        .map_err(|error| Error::Codec {
            format: "snapshot-diff",
            message: error.to_string(),
        })
    }
}

/// The persisted wire shape. Private: the document is an encoding of
/// [`Snapshot`], not a second public type that could drift from it.
#[derive(Serialize, Deserialize)]
struct Document {
    format: String,
    root: String,
    #[serde(rename = "takenAtNanos")]
    taken_at_nanos: String,
    entries: Vec<EntryDocument>,
}

#[derive(Serialize, Deserialize)]
struct EntryDocument {
    path: String,
    size: u64,
    #[serde(rename = "modifiedNanos")]
    modified_nanos: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    hash: Option<String>,
}

#[derive(Serialize)]
struct DiffDocument<'a> {
    added: &'a [String],
    removed: &'a [String],
    modified: &'a [String],
    unchanged: &'a [String],
}

impl From<&Snapshot> for Document {
    fn from(snapshot: &Snapshot) -> Self {
        Self {
            format: FORMAT.to_owned(),
            root: normalise_root(&snapshot.root),
            taken_at_nanos: snapshot.taken_at_nanos.to_string(),
            entries: snapshot
                .entries
                .iter()
                .map(|(path, entry)| EntryDocument {
                    path: path.clone(),
                    size: entry.size,
                    modified_nanos: entry.modified_nanos.to_string(),
                    hash: entry.hash.clone(),
                })
                .collect(),
        }
    }
}

impl Document {
    fn into_snapshot(self) -> Result<Snapshot> {
        let taken_at_nanos = parse_nanos(&self.taken_at_nanos, "takenAtNanos")?;
        let mut entries = BTreeMap::new();
        for entry in self.entries {
            let modified_nanos = parse_nanos(&entry.modified_nanos, "modifiedNanos")?;
            entries.insert(
                entry.path,
                SnapshotEntry {
                    size: entry.size,
                    modified_nanos,
                    hash: entry.hash,
                },
            );
        }
        Ok(Snapshot {
            root: PathBuf::from(self.root),
            taken_at_nanos,
            entries,
        })
    }
}

fn parse_nanos(value: &str, field: &str) -> Result<i128> {
    value.parse::<i128>().map_err(|error| Error::Codec {
        format: "snapshot",
        message: format!("{field} {value:?} is not a decimal integer: {error}"),
    })
}

/// Slash-normalises a path so a snapshot taken on Windows and one taken on
/// Linux key the same file identically. The TypeScript surface already
/// normalises for the same reason.
fn normalise(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Slash-normalises a *root* and drops a Windows extended-length prefix.
///
/// The root is canonicalised by the walk, and on Windows `canonicalize`
/// returns a verbatim path (`\\?\C:\project`). That prefix is correct and
/// almost unusable: it is not what the caller passed, it does not compare
/// equal to anything they hold, and it would appear in every `Path` a diff
/// hands back. Dropping it is the same courtesy `dunce` exists to provide,
/// done here in four lines rather than as a dependency.
fn normalise_root(path: &Path) -> String {
    strip_verbatim(&normalise(path))
}

fn strip_verbatim(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("//?/") {
        // `//?/UNC/server/share` is a network path, whose usable spelling is
        // `//server/share`; anything else is a drive path.
        return rest
            .strip_prefix("UNC/")
            .map_or_else(|| rest.to_owned(), |unc| format!("//{unc}"));
    }
    path.to_owned()
}

fn now_nanos() -> i128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| i128::try_from(elapsed.as_nanos()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use crate::snapshot::{Snapshot, FORMAT};
    use crate::walk::ScanOptions;

    #[test]
    fn a_windows_verbatim_root_is_recorded_without_its_prefix() {
        // `std::fs::canonicalize` returns an extended-length path on Windows
        // (`\\?\C:\project`). It is correct and almost unusable: it leaks into
        // every path a diff hands back, and `C:/project` is what the caller
        // passed. Pure string logic, so the rule is pinned on every platform
        // and not only where the prefix occurs.
        assert_eq!(super::strip_verbatim("//?/C:/project"), "C:/project");
        assert_eq!(
            super::strip_verbatim("//?/UNC/server/share"),
            "//server/share"
        );
        assert_eq!(
            super::strip_verbatim("/home/user/project"),
            "/home/user/project"
        );
        assert_eq!(super::strip_verbatim("C:/project"), "C:/project");
    }

    #[test]
    fn a_capture_folds_the_tree_in_sorted_path_order() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/b.ts"), b"bbb").unwrap();
        std::fs::write(dir.path().join("a.ts"), b"a").unwrap();

        let snapshot = Snapshot::capture(dir.path(), ScanOptions::default()).unwrap();

        assert_eq!(snapshot.paths(), vec!["a.ts", "src/b.ts"]);
        assert_eq!(snapshot.entries()["a.ts"].size, 1);
        assert_eq!(snapshot.entries()["src/b.ts"].size, 3);
        assert!(snapshot.entries()["a.ts"].modified_nanos > 0);
    }

    #[test]
    fn a_diff_sorts_every_path_into_exactly_one_bucket() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("kept.ts"), b"same").unwrap();
        std::fs::write(dir.path().join("gone.ts"), b"bye").unwrap();
        let before = Snapshot::capture(dir.path(), hashed()).unwrap();

        std::fs::remove_file(dir.path().join("gone.ts")).unwrap();
        std::fs::write(dir.path().join("fresh.ts"), b"new").unwrap();
        std::fs::write(dir.path().join("kept.ts"), b"same").unwrap();
        let after = Snapshot::capture(dir.path(), hashed()).unwrap();

        let diff = before.diff(&after);

        assert_eq!(diff.added, vec!["fresh.ts"]);
        assert_eq!(diff.removed, vec!["gone.ts"]);
        assert!(diff.modified.is_empty(), "rewritten with identical bytes");
        assert_eq!(diff.unchanged, vec!["kept.ts"]);
        assert!(diff.has_changes());
    }

    #[test]
    fn content_decides_the_modified_bucket_when_both_sides_are_hashed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.ts"), b"before").unwrap();
        let before = Snapshot::capture(dir.path(), hashed()).unwrap();

        std::fs::write(dir.path().join("a.ts"), b"after!").unwrap();
        let after = Snapshot::capture(dir.path(), hashed()).unwrap();

        let diff = before.diff(&after);
        assert_eq!(diff.modified, vec!["a.ts"], "same size, different content");
        assert!(diff.unchanged.is_empty());
    }

    #[test]
    fn a_captured_mtime_is_not_truncated_to_milliseconds() {
        // The independent source of truth is the kernel's own stat, read
        // through std rather than recomputed the way `capture` does it.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.ts"), b"one").unwrap();

        let snapshot = Snapshot::capture(dir.path(), ScanOptions::default()).unwrap();
        let expected = std::fs::metadata(dir.path().join("a.ts"))
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        assert_eq!(
            snapshot.entries()["a.ts"].modified_nanos,
            i128::try_from(expected).unwrap(),
            "every digit the filesystem reported is kept"
        );
    }

    #[test]
    fn mtimes_differing_only_below_the_millisecond_diff_as_modified() {
        // The build-system case: two states of one file whose mtimes share
        // their millisecond. A millisecond-precision snapshot would call this
        // unchanged and keep a stale output; nanoseconds make it modified.
        //
        // Built from documents rather than from two writes on purpose: the
        // local filesystem was measured (2026-10-07, linux-64 overlayfs)
        // handing two back-to-back writes the *identical* nanosecond stamp, so
        // a write-write test proves the filesystem's clock tick, not the
        // format's precision.
        let before = Snapshot::from_json(&document("1700000000123000001")).unwrap();
        let after = Snapshot::from_json(&document("1700000000123999999")).unwrap();

        assert_eq!(
            before.entries()["a.ts"].modified_nanos / 1_000_000,
            after.entries()["a.ts"].modified_nanos / 1_000_000,
            "the same millisecond"
        );
        assert_eq!(before.diff(&after).modified, vec!["a.ts"]);
    }

    #[test]
    fn the_persisted_document_round_trips_and_is_byte_stable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.ts"), b"bb").unwrap();
        std::fs::write(dir.path().join("a.ts"), b"a").unwrap();
        let snapshot = Snapshot::capture(dir.path(), hashed()).unwrap();

        let json = snapshot.to_json().unwrap();
        assert_eq!(json, snapshot.to_json().unwrap(), "same value, same bytes");
        assert!(json.contains(FORMAT));
        assert!(
            json.find("\"a.ts\"").unwrap() < json.find("\"b.ts\"").unwrap(),
            "entries are emitted in sorted path order"
        );

        let restored = Snapshot::from_json(&json).unwrap();
        assert_eq!(restored, snapshot);
        assert!(!restored.diff(&snapshot).has_changes());
    }

    #[test]
    fn nanoseconds_survive_json_as_decimal_strings() {
        // 1 700 000 000 123 456 789 ns needs 61 bits; a JSON *number* would be
        // read back as a double and lose the last digits.
        let nanos = 1_700_000_000_123_456_789_i128;
        let json = format!(
            r#"{{"format":"{FORMAT}","root":"/tmp/x","takenAtNanos":"{nanos}","entries":[{{"path":"a.ts","size":1,"modifiedNanos":"{nanos}"}}]}}"#
        );

        let snapshot = Snapshot::from_json(&json).unwrap();

        assert_eq!(snapshot.entries()["a.ts"].modified_nanos, nanos);
        assert_eq!(snapshot.taken_at_nanos(), nanos);
        assert_eq!(snapshot.entries()["a.ts"].hash, None);
    }

    #[test]
    fn a_document_of_another_format_is_refused_by_name() {
        let error = Snapshot::from_json(
            r#"{"format":"pathway-snapshot-v0","root":"/tmp/x","takenAtNanos":"1","entries":[]}"#,
        )
        .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("pathway-snapshot-v0"), "{message}");
        assert!(message.contains(FORMAT), "{message}");
    }

    #[test]
    fn save_and_load_round_trip_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.ts"), b"a").unwrap();
        let snapshot = Snapshot::capture(dir.path(), hashed()).unwrap();

        let file = dir.path().join("cache").join("snapshot.json");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        snapshot.save(&file).unwrap();

        assert_eq!(Snapshot::load(&file).unwrap(), snapshot);
    }

    #[test]
    fn walk_filters_configure_a_capture() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("node_modules")).unwrap();
        std::fs::write(dir.path().join("node_modules/dep.ts"), b"x").unwrap();
        std::fs::write(dir.path().join("a.ts"), b"x").unwrap();
        std::fs::write(dir.path().join("a.md"), b"x").unwrap();

        let options = ScanOptions {
            glob: vec!["**/*.ts".to_owned()],
            exclude: vec!["node_modules".to_owned()],
            ..ScanOptions::default()
        };
        let snapshot = Snapshot::capture(dir.path(), options).unwrap();

        assert_eq!(snapshot.paths(), vec!["a.ts"]);
    }

    fn document(nanos: &str) -> String {
        format!(
            r#"{{"format":"{FORMAT}","root":"/tmp/x","takenAtNanos":"{nanos}","entries":[{{"path":"a.ts","size":3,"modifiedNanos":"{nanos}"}}]}}"#
        )
    }

    fn hashed() -> ScanOptions {
        ScanOptions {
            hash: Some(crate::hash::Algorithm::Blake3),
            ..ScanOptions::default()
        }
    }
}
