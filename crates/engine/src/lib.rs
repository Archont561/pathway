//! The N-API bridge. Glue only.
//!
//! Everything a caller can do through this crate is implemented in
//! [`pathway_fs_core`]. If logic appears here it is in the wrong crate: the core
//! is what `cargo test` runs without Node, and logic in the bridge is logic no
//! Rust-only test can reach.
//!
//! # The boundary rule (D2, and the reason this crate is worth its cost)
//!
//! This crate exists for *bulk* operations — walk, hash, read, write, copy. It
//! must never exist for a single string. `path.join("a").join("b")` crossing
//! N-API three times is not a slow path, it is an architecture that has given up:
//! each crossing costs more than the `pathe` call it wrapped, so the native
//! dependency would be pure loss for the operations that happen most often.
//!
//! # Concurrency
//!
//! A walk is blocking and CPU-bound, so it runs through `napi::bindgen_prelude::AsyncTask`
//! on the libuv thread pool rather than on the JavaScript thread. That *is*
//! the frozen mechanism: the Step 0 spike (task-1) measured chunked paging
//! against the experimental `#[napi(async_iterator)]` and froze paging — the
//! record, with the measurements, is in
//! `.knowledge/architecture/napi-boundary.md`.

#![deny(missing_docs)]

pub mod walk;

use napi::bindgen_prelude::Buffer;
use napi::Result as NapiResult;
use pathway_fs_core::hash::{hash_file, hash_reader, Algorithm};
use std::io::Cursor;
use std::path::Path;

/// Hash bytes with a built-in streaming algorithm.
#[napi]
pub fn hash_bytes_native(bytes: Buffer, algorithm: String) -> NapiResult<String> {
    let algorithm = Algorithm::from_name(&algorithm)
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    hash_reader(
        Cursor::new(bytes.to_vec()),
        algorithm,
        Path::new("<memory>"),
    )
    .map_err(|error| napi::Error::from_reason(error.to_string()))
}

/// Hash one file with a built-in streaming algorithm.
#[napi]
pub fn hash_file_native(path: String, algorithm: String) -> NapiResult<String> {
    let algorithm = Algorithm::from_name(&algorithm)
        .map_err(|error| napi::Error::from_reason(error.to_string()))?;
    hash_file(Path::new(&path), algorithm)
        .map_err(|error| napi::Error::from_reason(error.to_string()))
}

use napi_derive::napi;

/// The engine's version, so the TypeScript surface can refuse to run against a
/// stale `.node` file left over from an earlier build.
#[napi]
pub fn engine_version() -> String {
    pathway_fs_core::VERSION.to_owned()
}

/// The Node-API version this addon was compiled against.
///
/// `packages/path` asserts this against its own `engines` floor at load time. A
/// mismatch here means the addon was built for a newer runtime than the one
/// running it, which otherwise surfaces as a missing-symbol error at the first
/// call rather than as a clear message at import.
#[napi]
pub fn napi_version() -> u32 {
    pathway_fs_core::NAPI_VERSION
}
