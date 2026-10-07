//! Temporary files and directories.
//!
//! Cleanup is tiered by what the platform can actually guarantee, and the
//! guarantee is measured rather than assumed — each row below is a test in
//! this module:
//!
//! | Tier | Mechanism | Survives |
//! |------|-----------|----------|
//! | 1 | `Drop`, plus [`cleanup_live_temp_dirs`] called from the host's exit flush | a return, a throw, `process.exit()`, and a `SIGINT`/`SIGTERM` for which the host installed a handler that exits |
//! | 2 | `O_TMPFILE` (Linux) / `FILE_FLAG_DELETE_ON_CLOSE` (Windows) | `SIGKILL` — **for temp *files* only**. The kernel ignores `O_DIRECTORY` for `O_TMPFILE`, so an anonymous *directory* does not exist; see `linux_cannot_make_a_temp_directory_anonymous` |
//! | 3 | documented only | nothing else: a `SIGKILL`, a `SIGINT`/`SIGTERM` under the default disposition, or a power loss leaves the tree and its contents on disk until a reaper removes them |
//!
//! Two corrections that the 2025 draft got wrong, both established by the
//! probes in this module rather than by reading `tempfile`'s README:
//!
//! - **A temp directory is never tier 2.** [`TempDir`]'s whole point is a path
//!   the caller can hand to child processes and inspect, and a path implies a
//!   directory entry; `O_TMPFILE` provides anonymity by having no entry at all.
//!   The hard guarantee is a property of unnamed temp *files*, which this
//!   module does not create yet.
//! - **`Drop` is not an exit hook.** A `process.exit()` runs no destructors,
//!   so tier 1 needs the registry and the host's flush; a `SIGINT` runs none
//!   either unless the host installed a handler that exits cleanly.
//!
//! `tempfile` implements the directory creation and the tier-1 destructor, and
//! it is already a dependency. Reimplementing it would ship a weaker version of
//! a solved problem, so this module is the registry, the option plumbing and
//! the tier documentation — not another temp dir implementation.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use crate::error::{Error, Result};

/// How a temporary directory is named and where it is created.
///
/// Every field is optional and means "the platform default" when absent, so
/// `TempOptions::default()` is the same request a bare `tempfile::tempdir()`
/// makes — with the cleanup guarantees of [`TempDir`] on top.
#[derive(Debug, Default, Clone)]
pub struct TempOptions {
    /// Prefix for the directory's name. `tempfile`'s own default is `.tmp`.
    pub prefix: Option<String>,
    /// Suffix for the directory's name.
    pub suffix: Option<String>,
    /// The directory to create it inside. Absent means the system temp dir.
    pub parent: Option<PathBuf>,
}

/// A temporary directory that is removed when this value is dropped.
///
/// Removal is a `std::fs::remove_dir_all` driven by `tempfile`'s guard, not a
/// JavaScript `finally` block, so it runs while a panic unwinds as well as on
/// a normal return. Every live value is additionally registered for
/// [`cleanup_live_temp_dirs`], which is what covers a `process.exit()` — the
/// exit path that runs no destructors at all. What none of that covers, and
/// what the module docs spell out, is a `SIGKILL`.
///
/// # Examples
///
/// ```
/// use pathway_fs_core::fs::temp::{TempDir, TempOptions};
///
/// let dir = TempDir::new(TempOptions::default())?;
/// let scratch = dir.path().join("build");
/// std::fs::create_dir(&scratch)?;
/// assert!(scratch.is_dir());
/// // `dir` goes out of scope here and the tree is removed.
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct TempDir {
    /// Kept separately because `tempfile`'s guard owns the tree, not the path.
    path: PathBuf,
    /// `None` once the guard has been consumed by `close`/`keep`/`Drop`.
    inner: Option<tempfile::TempDir>,
}

/// Every live temporary directory in this process, by path.
///
/// This is what makes tier 1 cover `process.exit()`. `tempfile`'s guarantee is
/// its destructor, and a `process.exit()` runs no destructors; an exit flush
/// needs a list that does not depend on unwinding. It holds paths rather than
/// guards because it has to be reachable from a plain `extern "C"` callback:
/// no unwinding, no user code, no allocations it has to trust.
///
/// A path stays registered exactly as long as its [`TempDir`] does, so a
/// directory that was already removed — by `Drop` during unwind, by `close`,
/// or by the exit flush itself — is not removed twice.
static LIVE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Registers a path for the exit flush.
fn register(path: &Path) {
    LIVE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(path.to_owned());
}

/// Removes a path from the exit-flush registry.
///
/// A path may already have been removed by [`cleanup_live_temp_dirs`]; that is
/// not an error and not a double removal, so the missing entry is ignored.
fn unregister(path: &Path) {
    LIVE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|candidate| candidate != path);
}

/// Removes every registered temporary directory that is still on disk.
///
/// The host's exit flush calls this: the N-API bridge installs a JS
/// `process.on("exit")` hook that forwards here, and the Rust surface calls it
/// from its own `atexit` hook. Returns how many directories it removed.
///
/// It is deliberately not a destructor. Tier 1 covers a return, a throw, a
/// normal `process.exit()` and a `SIGINT` for which the host installed a
/// handler that exits; it does not cover a `SIGKILL` (nothing runs) and it does
/// not cover a `SIGINT`/`SIGTERM` whose default disposition terminates the
/// process without running anything, which is why those two are documented
/// rather than claimed. See the module docs for the full table.
pub fn cleanup_live_temp_dirs() -> usize {
    let paths: Vec<PathBuf> = {
        let live = LIVE.lock().unwrap_or_else(PoisonError::into_inner);
        live.clone()
    };

    let mut removed = 0;
    for path in paths {
        let gone = std::fs::remove_dir_all(&path).is_ok();
        if gone {
            removed += 1;
        }
        unregister(&path);
    }
    removed
}

impl TempDir {
    /// Creates a temporary directory.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the parent does not exist, is not writable,
    /// or the underlying `mkdtemp` call fails.
    pub fn new(options: TempOptions) -> Result<Self> {
        let TempOptions {
            prefix,
            suffix,
            parent,
        } = options;

        let mut builder = tempfile::Builder::new();
        if let Some(prefix) = prefix.as_deref() {
            builder.prefix(prefix);
        }
        if let Some(suffix) = suffix.as_deref() {
            builder.suffix(suffix);
        }

        let created = match parent.as_deref() {
            Some(parent) => builder.tempdir_in(parent),
            None => builder.tempdir(),
        };
        let inner = created.map_err(|source| {
            Error::io(
                "create temporary directory",
                parent.unwrap_or_else(std::env::temp_dir),
                source,
            )
        })?;

        let path = inner.path().to_owned();
        register(&path);
        Ok(Self {
            path,
            inner: Some(inner),
        })
    }

    /// The directory's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Removes the tree now, consuming the guard.
    ///
    /// This is the explicit form of what `Drop` does, for a caller that wants
    /// the `io::Error` instead of a best-effort attempt. It is idempotent with
    /// respect to `Drop`: once the guard is consumed, dropping the value does
    /// nothing. Like `Drop`, it unregisters the path only once the tree is
    /// gone, so a removal that fails stays queued for the host's exit flush.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the tree could not be removed.
    pub fn close(mut self) -> Result<()> {
        let path = self.path.clone();
        match self.inner.take() {
            Some(inner) => {
                let removed = inner
                    .close()
                    .map_err(|source| Error::io("remove temporary directory", path, source));
                if removed.is_ok() {
                    unregister(&self.path);
                }
                removed
            }
            None => Ok(()),
        }
    }

    /// Disarms cleanup and hands the directory to the caller.
    ///
    /// Returns the path, which is now the caller's to remove: the guard no
    /// longer deletes it on drop or on the host's exit flush.
    #[must_use]
    pub fn keep(mut self) -> PathBuf {
        unregister(&self.path);
        if let Some(inner) = self.inner.take() {
            let _ = inner.keep();
        }
        self.path.clone()
    }
}

impl Drop for TempDir {
    /// Removes the tree, best effort: a `Drop` that cannot report an error
    /// still has to try, and it unregisters the path only once the tree is
    /// actually gone — so a removal that fails (a busy file, a permission)
    /// stays in the registry and the host's exit flush retries it instead of
    /// leaving the directory behind silently.
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            if inner.close().is_ok() {
                unregister(&self.path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{cleanup_live_temp_dirs, TempDir, TempOptions};

    #[test]
    fn creates_a_directory_in_the_requested_parent_with_prefix_and_suffix() {
        let parent = tempfile::tempdir().unwrap();
        let dir = TempDir::new(TempOptions {
            prefix: Some("pathway-build-".to_owned()),
            suffix: Some("-scratch".to_owned()),
            parent: Some(parent.path().to_owned()),
        })
        .unwrap();

        let name = dir
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(name.starts_with("pathway-build-"), "{name}");
        assert!(name.ends_with("-scratch"), "{name}");
        assert!(dir.path().is_dir());
        assert_eq!(dir.path().parent().unwrap(), parent.path());
    }

    // ── the tier-2 boundary, pinned ────────────────────────────────────────

    /// Why the tier table says "temp *files*" and not "temp directories".
    ///
    /// Linux can make a file anonymous — `O_TMPFILE` returns an inode with no
    /// directory entry, and the kernel reclaims it when the last descriptor
    /// closes, even after a `SIGKILL`. It cannot do that for a directory: the
    /// `O_DIRECTORY` bit is ignored for `O_TMPFILE`, so what comes back is an
    /// unnamed *regular* file, and any attempt to create something inside it
    /// fails with `ENOTDIR`. A `Path.temp(callback)` directory therefore
    /// **cannot** be tier 2, and no amount of flag plumbing changes that.
    ///
    /// This test exists so the day a future release documents a
    /// SIGKILL-surviving temp *directory*, this fails and names the reason.
    /// On a kernel that rejects `O_TMPFILE` outright the test returns early:
    /// there is nothing to observe.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_cannot_make_a_temp_directory_anonymous() {
        use std::ffi::CString;

        let parent = tempfile::tempdir().unwrap();
        let parent_c = CString::new(parent.path().to_str().unwrap()).unwrap();
        let fd = unsafe {
            libc::open(
                parent_c.as_ptr(),
                libc::O_TMPFILE | libc::O_DIRECTORY | libc::O_RDWR | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            let errno = std::io::Error::last_os_error();
            eprintln!("O_TMPFILE unsupported on this filesystem ({errno}); nothing to observe");
            return;
        }

        // `O_DIRECTORY` was ignored: the descriptor is an unnamed regular file.
        let mut status: libc::stat = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { libc::fstat(fd, &raw mut status) }, 0);
        assert_eq!(
            status.st_mode & libc::S_IFMT,
            libc::S_IFREG,
            "O_TMPFILE produced a directory, not a file"
        );
        assert_eq!(status.st_nlink, 0, "an anonymous inode has no links");

        // And it cannot hold an entry: this is the part `Path.temp(callback)`
        // would need, and the part the kernel refuses.
        let child = CString::new("scratch.txt").unwrap();
        let child_fd = unsafe {
            libc::openat(
                fd,
                child.as_ptr(),
                libc::O_CREAT | libc::O_RDWR | libc::O_CLOEXEC,
                0o600,
            )
        };
        let child_errno = std::io::Error::last_os_error();
        if child_fd >= 0 {
            unsafe { libc::close(child_fd) };
        }
        unsafe { libc::close(fd) };

        assert_eq!(
            child_errno.raw_os_error(),
            Some(libc::ENOTDIR),
            "creating an entry inside an O_TMPFILE inode should fail: {child_errno}"
        );
        assert!(
            std::fs::read_dir(parent.path()).unwrap().next().is_none(),
            "neither the descriptor nor its failed child leaves a directory entry"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_removal_that_fails_stays_registered_for_the_exit_flush() {
        use std::os::unix::fs::PermissionsExt as _;

        let parent = tempfile::tempdir().unwrap();
        let dir = TempDir::new(TempOptions {
            parent: Some(parent.path().to_owned()),
            ..TempOptions::default()
        })
        .unwrap();
        let path = dir.path().to_owned();
        std::fs::write(path.join("scratch.txt"), b"contents").unwrap();

        // An unwritable parent makes the unlink of the directory entry fail.
        std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
        let enforced = std::fs::remove_dir_all(&path).is_err();
        if !enforced {
            // Running as root (or on a filesystem without the mode bit):
            // the premise does not hold; nothing to prove.
            std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o755))
                .unwrap();
            return;
        }

        let failure = dir.close();
        std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(failure.is_err(), "the removal could not have succeeded");
        assert!(path.is_dir(), "the tree is still there");
        assert_eq!(
            cleanup_live_temp_dirs(),
            1,
            "a failed removal stays registered, so the exit flush retries it"
        );
        assert!(!path.exists());
    }

    // ── the process-lifetime probes ─────────────────────────────────────────
    //
    // `Drop` cannot be observed at a process exit from inside the process: the
    // probes spawn this test binary again with PATHWAY_TEMP_CHILD_MODE set, and
    // the parent asserts on what is left on disk afterwards. Unix-only because
    // the probes use `libc::exit`/`raise`; the mechanism being probed (a
    // registry flushed by the host) is not platform-specific.

    /// The child half of the probes. Returns immediately — and passes — when
    /// the parent did not ask for one, so it is an ordinary test in every run.
    #[cfg(unix)]
    #[test]
    fn child_probe() {
        let Ok(mode) = std::env::var("PATHWAY_TEMP_CHILD_MODE") else {
            return;
        };
        let root = std::path::PathBuf::from(
            std::env::var("PATHWAY_TEMP_CHILD_ROOT").expect("the parent passes a probe root"),
        );
        let options = || super::TempOptions {
            parent: Some(root.clone()),
            ..super::TempOptions::default()
        };

        match mode.as_str() {
            "bare-exit" => {
                let _dir = super::TempDir::new(options()).unwrap();
                unsafe { libc::exit(0) };
            }
            "flushed-exit" => {
                let _dir = super::TempDir::new(options()).unwrap();
                assert_eq!(
                    super::cleanup_live_temp_dirs(),
                    1,
                    "the live directory is registered"
                );
                unsafe { libc::exit(0) };
            }
            "sigkill" => {
                let dir = super::TempDir::new(options()).unwrap();
                std::fs::write(dir.path().join("scratch.txt"), b"contents").unwrap();
                unsafe { libc::raise(libc::SIGKILL) };
            }
            "flush-with-kept" => {
                let kept = super::TempDir::new(options()).unwrap().keep();
                let registered = super::TempDir::new(options()).unwrap();
                assert_eq!(
                    super::cleanup_live_temp_dirs(),
                    1,
                    "only the registered directory is flushed"
                );
                assert!(kept.is_dir(), "a kept directory is not in the registry");
                std::fs::remove_dir_all(&kept).unwrap();
                drop(registered);
            }
            other => panic!("unknown probe mode {other:?}"),
        }
    }

    /// Runs [`child_probe`] in a fresh process and returns its exit status.
    #[cfg(unix)]
    fn run_child_probe(mode: &str, root: &std::path::Path) -> std::process::ExitStatus {
        let executable = std::env::current_exe().expect("the test binary is a real path");
        std::process::Command::new(executable)
            .args(["--exact", "fs::temp::tests::child_probe", "--nocapture"])
            .env("PATHWAY_TEMP_CHILD_MODE", mode)
            .env("PATHWAY_TEMP_CHILD_ROOT", root)
            .status()
            .expect("the probe child spawns")
    }

    /// Entry names directly under `dir`, sorted.
    fn entries(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[cfg(unix)]
    #[test]
    fn an_exit_that_skips_drop_leaves_the_directory_for_the_host_flush() {
        let root = tempfile::tempdir().unwrap();

        let status = run_child_probe("bare-exit", root.path());

        assert!(status.success(), "{status}");
        assert_eq!(
            entries(root.path()).len(),
            1,
            "libc::exit skips Drop, so the directory is still there; the host's \
             flush is what covers an exit"
        );
    }

    #[cfg(unix)]
    #[test]
    fn flushing_live_directories_removes_them_before_the_process_goes_away() {
        let root = tempfile::tempdir().unwrap();

        let status = run_child_probe("flushed-exit", root.path());

        assert!(status.success(), "{status}");
        assert!(
            entries(root.path()).is_empty(),
            "the flush removed the tree"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_sigkill_leaves_the_directory_and_its_contents_behind() {
        let root = tempfile::tempdir().unwrap();

        let status = run_child_probe("sigkill", root.path());

        assert!(status.code().is_none(), "killed by a signal: {status}");
        let survivors = entries(root.path());
        assert_eq!(survivors.len(), 1, "{survivors:?}");
        let victim = root.path().join(&survivors[0]);
        assert_eq!(
            std::fs::read(victim.join("scratch.txt")).unwrap(),
            b"contents",
            "tier 3: the OS reclaims nothing until a reaper does"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_kept_directory_is_not_flushed_but_a_registered_one_is() {
        let root = tempfile::tempdir().unwrap();

        let status = run_child_probe("flush-with-kept", root.path());

        assert!(status.success(), "{status}");
        assert!(
            entries(root.path()).is_empty(),
            "the child cleaned up its kept tree"
        );
    }

    #[test]
    fn close_removes_a_non_empty_tree_and_reports_removal() {
        let parent = tempfile::tempdir().unwrap();
        let dir = TempDir::new(TempOptions {
            parent: Some(parent.path().to_owned()),
            ..TempOptions::default()
        })
        .unwrap();
        let path = dir.path().to_owned();
        std::fs::write(path.join("scratch.txt"), b"contents").unwrap();

        dir.close().unwrap();

        assert!(!path.exists(), "close removes the tree and its contents");
    }

    #[test]
    fn keep_disarms_cleanup_and_returns_the_path() {
        let parent = tempfile::tempdir().unwrap();
        let path = {
            let dir = TempDir::new(TempOptions {
                parent: Some(parent.path().to_owned()),
                ..TempOptions::default()
            })
            .unwrap();
            dir.keep()
        };

        assert!(path.is_dir(), "a kept directory outlives its guard");
        std::fs::remove_dir(&path).unwrap();
    }

    #[test]
    fn the_directory_exists_until_it_is_dropped_and_is_gone_afterwards() {
        let parent = tempfile::tempdir().unwrap();
        let path = {
            let dir = TempDir::new(TempOptions {
                parent: Some(parent.path().to_owned()),
                ..TempOptions::default()
            })
            .unwrap();
            assert!(dir.path().is_dir());
            dir.path().to_owned()
        };

        assert!(!path.exists(), "the guard's Drop removes the tree");
    }
}
