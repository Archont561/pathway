//! Error types shared by every module in the core.
//!
//! One enum, not one per module: a walk that fails on a single unreadable file
//! has to report that file's `io::Error` *and* keep walking (D3 — the fused
//! walk yields partially populated batches plus the failures, so a caller can
//! decide whether one bad file invalidates the run). Modelling that needs the
//! walk's error to carry an `io::Error` while the walk itself is not an error,
//! which a per-module enum with `#[from]` conversions cannot express without
//! either losing the variant or adding `Other(Box<dyn Error>)` to paper over it.

use std::io;
use std::path::PathBuf;

/// Everything the core can fail with.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A filesystem operation failed.
    ///
    /// `path` is the entry the failure is attributed to, which for a walk is
    /// the individual file and not the walk root: reporting the root would make
    /// a partial walk's error useless to the caller that has to decide whether
    /// to retry that one file.
    #[error("{operation} failed for {path}: {source}")]
    Io {
        /// The operation that was attempted, e.g. `"stat"` or `"read"`.
        operation: &'static str,
        /// The entry the operation was attempted on.
        path: PathBuf,
        /// The underlying operating-system error.
        #[source]
        source: io::Error,
    },

    /// A glob pattern could not be compiled.
    #[error("invalid glob pattern {pattern:?}: {message}")]
    Glob {
        /// The pattern as the caller wrote it.
        pattern: String,
        /// Why it is not a valid glob.
        message: String,
    },

    /// A regular expression could not be compiled.
    #[error("invalid regex {pattern:?}: {message}")]
    Regex {
        /// The pattern as the caller wrote it.
        pattern: String,
        /// Why it is not a valid regular expression.
        message: String,
    },

    /// No hash algorithm by that name is registered.
    #[error("unknown hash algorithm {0:?}; expected one of blake3, xxhash, sha256")]
    UnknownHasher(String),

    /// No serializer by that name is registered in this instance's registry.
    ///
    /// Per-instance rather than global, by decision D6: a global registry is
    /// mutable process-wide state, so two `FileSystem` instances in one process
    /// could not disagree about what `read("json")` means.
    #[error("unknown serializer {0:?} in this FileSystem's registry")]
    UnknownSerializer(String),

    /// A path escaped the root it was sandboxed to.
    ///
    /// The check is per component with `openat(O_NOFOLLOW)`, not a final-path
    /// `canonicalize`: the latter misses an intermediate symlink escape and is
    /// a TOCTOU window. See `.knowledge/features/killer-features.md`.
    #[error("{path} escapes the sandbox root {root}")]
    Escaped {
        /// The path that was refused.
        path: PathBuf,
        /// The root it must stay inside.
        root: PathBuf,
    },

    /// A serialized payload did not round-trip.
    #[error("{format} codec failed: {message}")]
    Codec {
        /// The codec that failed, e.g. `"toml"`.
        format: &'static str,
        /// The codec's own error message.
        message: String,
    },
}

/// The result type every fallible core function returns.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Attributes an [`io::Error`] to the entry it happened on.
    ///
    /// Every `io::Error` crossing into this enum goes through here, so the
    /// `operation` label and the path are recorded at the point of failure
    /// rather than reconstructed by the caller that catches it.
    pub fn io(operation: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.into(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_errors_name_the_entry_not_the_walk_root() {
        let err = Error::io(
            "stat",
            "a/b/c.ts",
            io::Error::from(io::ErrorKind::PermissionDenied),
        );
        let message = err.to_string();
        assert!(message.contains("stat"), "{message}");
        assert!(message.contains("a/b/c.ts"), "{message}");
    }

    #[test]
    fn io_errors_expose_the_source_for_downcasting() {
        use std::error::Error as _;
        let err = Error::io("read", "x", io::Error::from(io::ErrorKind::NotFound));
        assert_eq!(
            err.source()
                .and_then(|e| e.downcast_ref::<io::Error>())
                .map(io::Error::kind),
            Some(io::ErrorKind::NotFound)
        );
    }
}
