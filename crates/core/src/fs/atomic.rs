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

use std::io::Write;
use std::path::Path;

/// Writes `bytes` to `target` such that a reader sees either the old contents
/// or the new ones, never a partial file.
///
/// The temporary file is created by `tempfile` with exclusive creation in the
/// target's directory. The contents are synced before the rename, and the
/// directory sync is best effort because Windows does not expose an equivalent
/// directory handle.
///
/// # Errors
///
/// Returns [`crate::error::Error::Io`] attributed to `target` if creating,
/// writing, syncing, or renaming the temporary file fails.
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
pub fn write_atomic(target: &Path, bytes: &[u8]) -> crate::error::Result<()> {
    let directory = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut temporary = tempfile::Builder::new()
        .prefix(".pathway-")
        .tempfile_in(directory)
        .map_err(|source| crate::error::Error::io("create temporary file", target, source))?;

    temporary
        .write_all(bytes)
        .map_err(|source| crate::error::Error::io("write temporary file", target, source))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|source| crate::error::Error::io("sync temporary file", target, source))?;

    temporary.persist(target).map_err(|failure| {
        crate::error::Error::io("rename temporary file", target, failure.error)
    })?;

    sync_directory(directory);
    Ok(())
}

/// Flushes the directory entry update when the platform permits it.
fn sync_directory(directory: &Path) {
    let Ok(handle) = std::fs::File::open(directory) else {
        return;
    };
    let _ = handle.sync_all();
}

#[cfg(test)]
mod tests {
    use super::write_atomic;
    use std::fs::{read, read_dir, read_link, symlink_metadata};
    use tempfile::tempdir;

    #[test]
    fn writes_the_new_contents_and_leaves_no_temporary_file() {
        let directory = tempdir().unwrap();
        let target = directory.path().join("config.json");
        std::fs::write(&target, b"old").unwrap();

        write_atomic(&target, b"new").unwrap();

        assert_eq!(read(&target).unwrap(), b"new");
        let entries: Vec<_> = read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![std::ffi::OsString::from("config.json")]);
    }

    #[test]
    fn failed_rename_keeps_the_existing_target_and_cleans_up() {
        let directory = tempdir().unwrap();
        let target = directory.path().join("existing-directory");
        std::fs::create_dir(&target).unwrap();

        assert!(write_atomic(&target, b"not a directory").is_err());

        assert!(target.is_dir());
        assert!(read_dir(directory.path())
            .unwrap()
            .all(|entry| { entry.unwrap().file_name() == "existing-directory" }));
    }

    #[cfg(unix)]
    #[test]
    fn replaces_a_symlink_without_modifying_its_target() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().unwrap();
        let outside = directory.path().join("outside.txt");
        let link = directory.path().join("link.txt");
        std::fs::write(&outside, b"outside").unwrap();
        symlink(&outside, &link).unwrap();

        write_atomic(&link, b"inside").unwrap();

        assert_eq!(read(&outside).unwrap(), b"outside");
        assert_eq!(read(&link).unwrap(), b"inside");
        assert!(!symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(
            read_link(&link).err().map(|error| error.kind()),
            Some(std::io::ErrorKind::InvalidInput)
        );
    }
}
