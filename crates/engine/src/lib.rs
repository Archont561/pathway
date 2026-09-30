//! The N-API bridge. Glue only.
//!
//! Everything a caller can do through this crate is implemented in
//! [`myorg_path_core`]. If logic appears here it is in the wrong crate: the core
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
//! on the libuv thread pool rather than on the JavaScript thread. Whether that
//! stays the mechanism or is replaced by the experimental
//! `#[napi(async_iterator)]` is the open question in
//! `.knowledge/implementation/phase-plan.md` Step 0; both are behind the same
//! TypeScript API, which is the point of freezing the API before the spike.

#![deny(missing_docs)]

use napi_derive::napi;

/// The engine's version, so the TypeScript surface can refuse to run against a
/// stale `.node` file left over from an earlier build.
#[napi]
pub fn engine_version() -> String {
    myorg_path_core::VERSION.to_owned()
}

/// The Node-API version this addon was compiled against.
///
/// `packages/path` asserts this against its own `engines` floor at load time. A
/// mismatch here means the addon was built for a newer runtime than the one
/// running it, which otherwise surfaces as a missing-symbol error at the first
/// call rather than as a clear message at import.
#[napi]
pub fn napi_version() -> u32 {
    myorg_path_core::NAPI_VERSION
}
