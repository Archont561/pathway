//! One item yielded by the fused walk.
//!
//! A `FusedEntry` is the whole point of D3: traversal, `stat`, content hashing
//! and filtering have already happened, so this is a populated value rather
//! than a path the caller has to go and populate. That is what turns N paths of
//! N boundary crossings into N/512 of them.
//!
//! ## Failure is a field, not a batch
//!
//! `error` is per-entry. One unreadable file in a 500 000-file walk does not
//! invalidate the walk, and a design where it did would make the engine useless
//! for its main job — scanning a working tree that contains a file another
//! process is writing to. A walk also reports the errors it collected, so a
//! caller can require a clean run without re-walking to find out what failed.

/// A single result of a fused walk.
///
/// `Debug` only, deliberately: `Clone`/`PartialEq` would need
/// [`crate::error::Error`] to be both, and `io::Error` is neither. That is not
/// an inconvenience to work around with `Arc` — it is a hint about the shape
/// this type wants. A walk yields a batch that is consumed and dropped, not a
/// value that is cloned and compared, so the fields a caller actually branches
/// on are the cheap ones (`is_dir`, `size`) and the expensive one (`hash`) is
/// already an owned `String`.
#[derive(Debug)]
pub struct FusedEntry {
    /// The path, relative to the walk root unless `absolute` was set.
    pub path: std::path::PathBuf,
    /// Whether this entry is a directory.
    ///
    /// A directory is still yielded: the caller asked to walk, and filtering
    /// directories out here would make `walkDirs` a second traversal.
    pub is_dir: bool,
    /// File size in bytes, when metadata was requested and the `stat` succeeded.
    pub size: Option<u64>,
    /// Modification time as nanoseconds since the Unix epoch.
    ///
    /// Nanoseconds, not milliseconds. Two files written inside the same
    /// millisecond are indistinguishable in a millisecond-precision snapshot,
    /// which silently breaks incremental builds (Phase 2 acceptance criteria).
    pub modified_nanos: Option<i128>,
    /// Lowercase hex digest, when a hash algorithm was requested and the read
    /// succeeded.
    pub hash: Option<String>,
    /// Why this entry could not be completed, if it could not.
    ///
    /// A partially populated entry with an `error` is still yielded: the path
    /// and the stat are known, and discarding the entry would hide a file that
    /// exists.
    pub error: Option<crate::error::Error>,
}

#[cfg(test)]
mod tests {
    use super::FusedEntry;

    #[test]
    fn a_failed_entry_still_carries_its_path_and_stat() {
        let entry = FusedEntry {
            path: "a/b.ts".into(),
            is_dir: false,
            size: Some(12),
            modified_nanos: Some(1),
            hash: None,
            error: Some(crate::error::Error::io(
                "read",
                "a/b.ts",
                std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            )),
        };

        assert!(entry.error.is_some());
        assert_eq!(entry.path.to_str(), Some("a/b.ts"));
        assert_eq!(entry.size, Some(12));
    }
}
