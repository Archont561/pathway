//! The snapshot bridge: [`pathway_fs_core::snapshot`] over N-API.
//!
//! Glue only, and glue of one shape: **the JSON document is the transport**.
//! A snapshot crosses the boundary as the same persisted document `save()`
//! writes to disk, and a diff crosses as the four arrays the core computed.
//! Nothing here folds, sorts, compares or re-times anything.
//!
//! That choice is what keeps the TypeScript surface a view. The alternative —
//! marshalling a map of entry objects and diffing them in JavaScript — would
//! put the comparison rule (hashes when both sides have them, `size` plus
//! nanosecond mtime otherwise) in two places, and a snapshot format whose two
//! implementations disagree is worse than no format. It is also the coarse
//! boundary D2 asks for: one crossing per snapshot, never one per entry.

use napi::bindgen_prelude::AsyncTask;
use napi::{Env, Task};
use napi_derive::napi;

use pathway_fs_core::error::Error;
use pathway_fs_core::snapshot::Snapshot;
use pathway_fs_core::walk::ScanOptions;

use crate::walk::{scan_options, WalkerOptions};

/// Captures a snapshot of `root` on the libuv thread pool.
///
/// Takes the same options object a walk does, because a snapshot *is* a walk:
/// globs, exclusions, `.gitignore` and `hash` all mean here what they mean
/// there. `filesOnly` and `withMetadata` are forced on in core.
///
/// # Errors
///
/// Rejects with the core's message if the root cannot be resolved, a pattern
/// does not compile, or the walk reported a traversal failure.
#[napi(ts_return_type = "Promise<string>")]
pub fn snapshot_capture_native(
    root: String,
    options: Option<WalkerOptions>,
) -> napi::Result<AsyncTask<CaptureSnapshotTask>> {
    Ok(AsyncTask::new(CaptureSnapshotTask::new(root, options)?))
}

/// Diffs two persisted snapshot documents, returning the diff document.
///
/// Synchronous: comparing two already-materialised maps is cheap, and a
/// promise would add a microtask to something that cannot block.
///
/// # Errors
///
/// Returns the core's message if either document is not a snapshot of the
/// supported format.
#[napi]
pub fn snapshot_diff_native(before: String, after: String) -> napi::Result<String> {
    let before = Snapshot::from_json(&before).map_err(bridge_error)?;
    let after = Snapshot::from_json(&after).map_err(bridge_error)?;
    before.diff(&after).to_json().map_err(bridge_error)
}

/// Parses a document and renders it back, canonically.
///
/// This is how a snapshot read from disk is admitted: the core decides what a
/// valid document is (format tag, decimal-integer nanoseconds) and re-emits it
/// in sorted order, so a hand-edited or foreign file is refused here rather
/// than diffed into a tree that never existed.
///
/// # Errors
///
/// Returns the core's message if the payload is not a snapshot document of the
/// supported format.
#[napi]
pub fn snapshot_validate_native(json: String) -> napi::Result<String> {
    Snapshot::from_json(&json)
        .map_err(bridge_error)?
        .to_json()
        .map_err(bridge_error)
}

/// `snapshotCaptureNative()` as a libuv-pool task.
pub struct CaptureSnapshotTask {
    root: String,
    options: ScanOptions,
}

impl CaptureSnapshotTask {
    /// Decodes the options eagerly, so a bad glob or hasher name fails at the
    /// call site instead of inside the pool.
    ///
    /// # Errors
    ///
    /// Returns the core's message if the options do not decode.
    pub fn new(root: String, options: Option<WalkerOptions>) -> napi::Result<Self> {
        Ok(Self {
            root,
            options: scan_options(options.unwrap_or_default())?,
        })
    }
}

#[napi]
impl Task for CaptureSnapshotTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        let snapshot = Snapshot::capture(&self.root, self.options.clone()).map_err(bridge_error)?;
        snapshot.to_json().map_err(bridge_error)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

/// Core error → N-API rejection, keeping the core's message verbatim.
fn bridge_error(error: Error) -> napi::Error {
    napi::Error::from_reason(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{snapshot_diff_native, snapshot_validate_native, CaptureSnapshotTask};
    use napi::Task;
    use std::path::PathBuf;

    /// A scratch directory of this crate's own, so the suite never uses the
    /// checkout as a fixture. `tempfile` is a core dependency, not an engine
    /// one, and the bridge has no business growing dependencies for tests.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "pathway-engine-snapshot-{}-{name}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }

        fn capture(&self) -> String {
            CaptureSnapshotTask::new(self.0.to_string_lossy().into_owned(), None)
                .unwrap()
                .compute()
                .unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_capture_crosses_as_the_cores_json_document() {
        let dir = Scratch::new("capture");
        std::fs::write(dir.path().join("a.ts"), b"x").unwrap();

        let json = dir.capture();

        assert!(json.contains("pathway-snapshot-v1"));
        assert!(json.contains("\"a.ts\""));
    }

    #[test]
    fn a_diff_is_computed_in_core_and_crosses_as_four_arrays() {
        let dir = Scratch::new("diff");
        std::fs::write(dir.path().join("a.ts"), b"x").unwrap();
        let before = dir.capture();

        std::fs::write(dir.path().join("b.ts"), b"y").unwrap();
        let after = dir.capture();

        let diff = snapshot_diff_native(before, after).unwrap();

        assert!(diff.contains(r#""added":["b.ts"]"#), "{diff}");
        assert!(diff.contains(r#""unchanged":["a.ts"]"#), "{diff}");
    }

    #[test]
    fn validation_refuses_a_foreign_document_at_the_boundary() {
        let error = snapshot_validate_native(
            r#"{"format":"not-a-snapshot","root":"/tmp","takenAtNanos":"1","entries":[]}"#
                .to_owned(),
        )
        .unwrap_err();

        assert!(error.reason.contains("not-a-snapshot"), "{}", error.reason);
    }

    #[test]
    fn validation_returns_the_canonical_document() {
        let json = r#"{"format":"pathway-snapshot-v1","root":"/tmp/x","takenAtNanos":"7","entries":[{"path":"b.ts","size":1,"modifiedNanos":"2"},{"path":"a.ts","size":1,"modifiedNanos":"3"}]}"#;

        let canonical = snapshot_validate_native(json.to_owned()).unwrap();

        assert!(
            canonical.find("\"a.ts\"").unwrap() < canonical.find("\"b.ts\"").unwrap(),
            "a loaded document is re-folded in sorted order: {canonical}"
        );
    }
}
