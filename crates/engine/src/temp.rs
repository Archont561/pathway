//! The temp-directory bridge: [`pathway_fs_core::fs::temp`] over N-API.
//!
//! Glue only, like the walk bridge. What is *not* glue is the exit flush: the
//! cleanup guarantee the TypeScript surface documents needs a hook that runs
//! when the process exits, and the hook belongs to the host, not to the core —
//! a library that installs `SIGINT`/`SIGTERM` handlers changes the host
//! process's semantics behind its back. So the core keeps the registry and this
//! crate exposes [`flush_temp_dirs`], which the JavaScript side calls from a
//! `process.on("exit")` hook installed on first use.
//!
//! The tier story, and why a temp *directory* can never be tier 2, is in the
//! core module's docs: [`pathway_fs_core::fs::temp`].

use napi_derive::napi;

use pathway_fs_core::fs::temp::{
    cleanup_live_temp_dirs, TempDir as CoreTempDir, TempOptions as CoreTempOptions,
};

/// How a native temp directory is named and where it is created.
///
/// Mirrors `TempOptions` in `packages/path/src/temp.ts`; every field is
/// optional and absence means the platform default, so there is no second
/// default table on this side. The name is the public one: the engine's types
/// are the JavaScript-facing names, like `Walker` is for the scanner.
#[napi(object)]
#[derive(Default)]
pub struct TempOptions {
    /// Prefix for the directory's name.
    pub prefix: Option<String>,
    /// Suffix for the directory's name.
    pub suffix: Option<String>,
    /// The directory to create it inside. Absent means the system temp dir.
    pub parent: Option<String>,
}

/// A temporary directory whose cleanup is driven by the OS, not by `finally`.
///
/// Held by JavaScript as the handle `Path.temp()` returns: `remove()` is the
/// explicit form of the drop, `keep()` disarms cleanup and hands the caller a
/// path they own. A value that is simply garbage-collected is removed too.
#[napi]
pub struct TempDir {
    /// `None` once `remove`/`keep` consumed the guard.
    inner: Option<CoreTempDir>,
}

#[napi]
impl TempDir {
    /// Creates the directory.
    #[napi(constructor)]
    pub fn new(options: Option<TempOptions>) -> napi::Result<Self> {
        let options = options.unwrap_or_default();
        let created = CoreTempDir::new(CoreTempOptions {
            prefix: options.prefix,
            suffix: options.suffix,
            parent: options.parent.map(Into::into),
        })
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;

        Ok(Self {
            inner: Some(created),
        })
    }

    /// The directory's absolute path.
    #[napi]
    pub fn path(&self) -> napi::Result<String> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("this temp directory is no longer live"))?;
        Ok(inner.path().to_string_lossy().into_owned())
    }

    /// Removes the tree now, consuming the handle.
    #[napi]
    pub fn remove(&mut self) -> napi::Result<()> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| napi::Error::from_reason("this temp directory was already consumed"))?;
        inner
            .close()
            .map_err(|error| napi::Error::from_reason(error.to_string()))
    }

    /// Disarms cleanup and returns the path, which the caller now owns.
    #[napi]
    pub fn keep(&mut self) -> napi::Result<String> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| napi::Error::from_reason("this temp directory was already consumed"))?;
        Ok(inner.keep().to_string_lossy().into_owned())
    }
}

/// Removes every live temp directory registered in this process.
///
/// Called from the JavaScript `process.on("exit")` hook, so a `process.exit()`
/// — which runs no destructors — still leaves no scratch tree behind. Returns
/// how many directories it removed.
#[napi]
pub fn flush_temp_dirs() -> u32 {
    u32::try_from(cleanup_live_temp_dirs()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::{flush_temp_dirs, TempDir, TempOptions};

    /// A parent directory unique to this process; nextest isolates the test
    /// binaries, so a pid-suffixed name cannot collide with a parallel run.
    fn probe_root(name: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("pathway-engine-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn options(parent: &std::path::Path, prefix: Option<&str>) -> TempOptions {
        TempOptions {
            prefix: prefix.map(str::to_owned),
            suffix: None,
            parent: Some(parent.to_string_lossy().into_owned()),
        }
    }

    #[test]
    fn a_temp_dir_is_created_removed_and_can_be_kept() {
        let root = probe_root("temp");
        let mut dir = TempDir::new(Some(options(&root, Some("pathway-")))).unwrap();
        let path = dir.path().unwrap();

        let name = std::path::Path::new(&path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            name.starts_with("pathway-"),
            "the prefix reaches tempfile: {name}"
        );
        assert!(std::path::Path::new(&path).is_dir(), "{path}");
        assert!(std::path::Path::new(&path).starts_with(&root));

        dir.remove().unwrap();
        assert!(
            !std::path::Path::new(&path).exists(),
            "remove deletes the tree"
        );
        assert!(
            dir.path().is_err(),
            "a consumed handle cannot report a path it no longer owns"
        );

        let mut kept = TempDir::new(Some(options(&root, None))).unwrap();
        let kept_path = kept.keep().unwrap();
        assert!(
            std::path::Path::new(&kept_path).is_dir(),
            "keep hands the path over without deleting it"
        );
        std::fs::remove_dir_all(&kept_path).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_flush_removes_live_directories_and_reports_how_many() {
        let root = probe_root("flush");
        let _live = TempDir::new(Some(options(&root, None))).unwrap();

        let flushed = flush_temp_dirs();

        assert_eq!(flushed, 1, "the one live directory was removed");
        assert!(
            std::fs::read_dir(&root).unwrap().next().is_none(),
            "nothing is left under the probe root"
        );
        assert_eq!(flush_temp_dirs(), 0, "a second flush has nothing to do");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn creation_failures_name_the_parent_directory() {
        let missing = std::env::temp_dir().join(format!(
            "pathway-engine-missing-{}-does-not-exist",
            std::process::id()
        ));
        let error = match TempDir::new(Some(options(&missing, None))) {
            Ok(_) => panic!("creating a directory inside a missing parent should fail"),
            Err(error) => error,
        };

        let message = error.to_string();
        assert!(message.contains("create temporary directory"), "{message}");
        assert!(message.contains("missing"), "{message}");
    }
}
