//! Descriptor-anchored path containment.
//!
//! A sandboxed path must not escape its root. The obvious implementation —
//! `canonicalize` the candidate, `canonicalize` the root, check
//! `starts_with` — is wrong because it leaves a TOCTOU window and follows
//! intermediate symlinks while resolving names.
//!
//! On Unix, [`Sandbox::open_read`] opens the trusted root once, then walks one
//! component at a time with `openat(O_NOFOLLOW)` from the directory descriptor
//! returned by the previous component. No later operation re-resolves the
//! candidate from the process working directory. Symlinks are rejected rather
//! than followed, which is the strictest safe policy for an untrusted path.
//!
//! The descriptor is held by the [`Sandbox`] value, so replacing the root path
//! after construction does not retarget an existing sandbox. The API is read
//! oriented for now; write and create modes need their own policy before they
//! are exposed. On platforms without `openat`, the fallback uses canonicalize
//! plus a containment check and therefore retains a TOCTOU limitation; callers
//! requiring race-free containment must use a platform with the descriptor
//! implementation until an equivalent platform primitive is added.

use std::fs::File;
use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};

/// A filesystem root held as a stable descriptor where the platform supports it.
#[derive(Debug)]
pub struct Sandbox {
    root: PathBuf,
    #[cfg(unix)]
    root_file: File,
}

impl Sandbox {
    /// Opens a directory as a containment root.
    ///
    /// On Unix, a symlink root is rejected so the descriptor and the named
    /// root cannot disagree. The caller should keep the root directory itself
    /// trusted; the untrusted portion belongs in [`Self::open_read`].
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the root cannot be opened or is not a directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = absolute_path(root.as_ref())?;
        #[cfg(unix)]
        {
            let root_file = open_directory(&root, &root)?;
            Ok(Self { root, root_file })
        }
        #[cfg(not(unix))]
        {
            let metadata =
                std::fs::metadata(&root).map_err(|source| Error::io("stat", &root, source))?;
            if !metadata.is_dir() {
                return Err(Error::io(
                    "open sandbox root",
                    &root,
                    std::io::Error::from(std::io::ErrorKind::NotADirectory),
                ));
            }
            Ok(Self { root })
        }
    }

    /// Returns the path used to name this sandbox root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Opens a path below the root without following symlinks.
    ///
    /// `relative` must be relative and must not contain a parent component.
    /// This deliberately rejects `..` instead of normalising it: callers can
    /// normalise untrusted URL syntax before entering the API, but the native
    /// boundary must not turn an escape attempt into an ambiguous name.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Escaped`] for absolute, parent, or symlink components;
    /// other failures retain the underlying I/O error and component.
    pub fn open_read(&self, relative: impl AsRef<Path>) -> Result<File> {
        let relative = relative.as_ref();
        let components = safe_components(relative, &self.root)?;

        #[cfg(unix)]
        {
            let mut current = self
                .root_file
                .try_clone()
                .map_err(|source| Error::io("clone sandbox root", &self.root, source))?;
            if components.is_empty() {
                return Ok(current);
            }

            for (index, component) in components.iter().enumerate() {
                let last = index + 1 == components.len();
                let next = open_component(current.as_raw_fd(), component, &self.root)?;
                if last {
                    return Ok(next);
                }
                if !next
                    .metadata()
                    .map_err(|source| Error::io("stat sandbox component", component, source))?
                    .is_dir()
                {
                    return Err(Error::io(
                        "open sandbox component",
                        component,
                        std::io::Error::from(std::io::ErrorKind::NotADirectory),
                    ));
                }
                current = next;
            }
            unreachable!("non-empty component list always returns its final descriptor");
        }

        #[cfg(not(unix))]
        {
            let candidate = self.root.join(relative);
            let real_root = std::fs::canonicalize(&self.root)
                .map_err(|source| Error::io("canonicalize sandbox root", &self.root, source))?;
            let real_candidate = std::fs::canonicalize(&candidate)
                .map_err(|source| Error::io("canonicalize sandbox path", &candidate, source))?;
            if !real_candidate.starts_with(&real_root) {
                return Err(Error::Escaped {
                    path: candidate,
                    root: self.root.clone(),
                });
            }
            File::open(&real_candidate).map_err(|source| Error::io("open", &real_candidate, source))
        }
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|source| Error::io("get current directory", path, source))
    }
}

fn safe_components<'a>(path: &'a Path, root: &Path) -> Result<Vec<&'a std::ffi::OsStr>> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(Error::Escaped {
                    path: path.to_path_buf(),
                    root: root.to_path_buf(),
                });
            }
            Component::Normal(name) => components.push(name),
        }
    }
    Ok(components)
}

#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};

#[cfg(unix)]
fn open_directory(path: &Path, root: &Path) -> Result<File> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let name = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        Error::io(
            "open sandbox root",
            path,
            std::io::Error::from(std::io::ErrorKind::InvalidInput),
        )
    })?;
    let flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW;
    // SAFETY: `name` is a valid, NUL-terminated path and the returned fd is
    // immediately wrapped in `File`, transferring ownership exactly once.
    let fd = unsafe { libc::open(name.as_ptr(), flags) };
    if fd < 0 {
        return Err(open_error(path, root));
    }
    // SAFETY: fd is a newly-owned descriptor from open and is not used again.
    let file = unsafe { File::from_raw_fd(fd) };
    if !file
        .metadata()
        .map_err(|source| Error::io("stat sandbox root", path, source))?
        .is_dir()
    {
        return Err(Error::io(
            "open sandbox root",
            path,
            std::io::Error::from(std::io::ErrorKind::NotADirectory),
        ));
    }
    Ok(file)
}

#[cfg(unix)]
fn open_component(parent_fd: i32, component: &std::ffi::OsStr, root: &Path) -> Result<File> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let name = CString::new(component.as_bytes()).map_err(|_| {
        Error::io(
            "open sandbox component",
            component,
            std::io::Error::from(std::io::ErrorKind::InvalidInput),
        )
    })?;
    let flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW;
    // SAFETY: parent_fd is held by `Sandbox` or the current loop iteration;
    // `name` is NUL-terminated and contains one path component only.
    let fd = unsafe { libc::openat(parent_fd, name.as_ptr(), flags) };
    if fd < 0 {
        return Err(open_error(component, root));
    }
    // SAFETY: fd is a newly-owned descriptor from openat and is not used again.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(unix)]
fn open_error(path: impl AsRef<Path>, root: &Path) -> Error {
    let source = std::io::Error::last_os_error();
    if source.raw_os_error() == Some(libc::ELOOP) {
        Error::Escaped {
            path: path.as_ref().to_path_buf(),
            root: root.to_path_buf(),
        }
    } else {
        Error::io("open sandbox component", path.as_ref(), source)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn opens_a_file_below_the_root() {
        let directory = tempdir().unwrap();
        std::fs::write(directory.path().join("inside.txt"), b"inside").unwrap();
        let sandbox = Sandbox::new(directory.path()).unwrap();
        let mut file = sandbox.open_read("inside.txt").unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        assert_eq!(content, "inside");
    }

    #[test]
    fn rejects_a_non_directory_root() {
        let directory = tempdir().unwrap();
        let file = directory.path().join("file");
        std::fs::write(&file, b"not a directory").unwrap();
        let error = Sandbox::new(&file).unwrap_err();
        assert!(matches!(error, Error::Io { .. }), "{error}");
    }

    #[test]
    fn rejects_parent_components() {
        let directory = tempdir().unwrap();
        let sandbox = Sandbox::new(directory.path()).unwrap();
        let error = sandbox.open_read("../outside").unwrap_err();
        assert!(matches!(error, Error::Escaped { .. }), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_components_without_following_them() {
        let directory = tempdir().unwrap();
        let outside = tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"secret").unwrap();
        std::os::unix::fs::symlink(outside.path(), directory.path().join("link")).unwrap();
        let sandbox = Sandbox::new(directory.path()).unwrap();

        let error = sandbox.open_read("link/secret.txt").unwrap_err();
        assert!(matches!(error, Error::Escaped { .. }), "{error}");
    }
}
