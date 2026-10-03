//! Atomic writes.
//!
//! The sequence is `O_EXCL` temp file in the *same directory* as the target,
//! write, `fsync` the file, `rename` over the target, then a best-effort
//! `fsync` of the parent directory.
//!
//! Every step is load-bearing and each one is a bug that has shipped somewhere:
//!
//! - The temp file is in the same directory because `rename` is only atomic
//!   within a filesystem. A temp file in `/tmp` degrades a crash-safe write into
//!   a copy that a crash can truncate halfway.
//! - `O_EXCL` because the alternative, truncating a predictable name, is a
//!   symlink attack.
//! - `fsync` before the rename because the rename can be durable while the file
//!   contents are not, which produces a correctly-named empty file after a crash.
//! - `fsync` of the directory because the *rename* is the thing that must
//!   survive. This is the step that is almost always omitted, and it is
//!   best-effort: Windows has no equivalent handle to sync, so there the
//!   guarantee is one level weaker and the docs say so.

/// Writes `bytes` to `target` such that a reader sees either the old contents
/// or the new ones, never a partial file.
///
/// # Errors
///
/// Returns [`crate::error::Error::Io`] attributed to `target` if the write
/// fails, including when it fails partway — a reader of `target` may therefore
/// see a truncated file until this function is implemented properly, which is
/// the reason it is a scaffold and not the finished behaviour.
///
/// # Examples
///
/// ```
/// use pathway_fs_core::fs::atomic::write_atomic;
///
/// let dir = tempfile::tempdir()?;
/// let target = dir.path().join("config.json");
///
/// write_atomic(&target, b"{}")?;
/// assert_eq!(std::fs::read(&target)?, b"{}");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn write_atomic(target: &std::path::Path, bytes: &[u8]) -> crate::error::Result<()> {
    // Phase 2. Until then, writing in place is honest about what it is.
    std::fs::write(target, bytes).map_err(|source| crate::error::Error::io("write", target, source))
}
