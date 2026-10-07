//! Native filesystem core — the engine that both public surfaces sit on.
//!
//! This crate is the whole of D1's layer 3, and the reason D7 splits it out of
//! `crates/engine`: it depends on nothing from the N-API world, so
//! `cargo test -p pathway-fs-core` exercises the engine without Node, without a
//! built addon, and without a JavaScript runtime in the loop at all.
//!
//! # Layering
//!
//! ```text
//!   @archont561/pathway (TypeScript)  ergonomic Path objects, Serializer<T>, iterators
//!   pathway-fs (Rust)                 ergonomic Path objects, walk builder, serde sugar
//!         │                            │
//!         └──────────────┬─────────────┘
//!                        ▼
//!   pathway-fs-core  ← this crate: traversal, hashing, codecs, atomic IO
//!                        ▲
//!                        │
//!   crates/engine (NAPI-RS cdylib) — glue only, never a dependency of this crate
//! ```
//!
//! Both surfaces are *thin*: the same behaviour, two ergonomics. A rule that
//! lands here must not be implemented twice, and a difference between the two
//! surfaces is a bug, not a feature.
//!
//! # The fused walk
//!
//! The moat is [walk] combined with [hash]: traversal, `stat`, content hashing
//! and filtering happen in a single syscall pass, so an application that needs
//! all four does not pay a JavaScript↔native boundary crossing per file. That is
//! the claim the benchmark harness exists to prove or refute — see
//! `.knowledge/architecture/fused-walk.md`.
//!
//! # Status
//!
//! The fused walker, chunked hashers, atomic-write primitive, and Unix
//! descriptor-anchored sandbox open primitive are implemented and covered by
//! the core's Rust tests. Locking and temporary directories remain staged for
//! later phases. The phase plan is `backlog/docs/phase-plan.md`.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]

pub mod error;
pub mod fs;
pub mod hash;
pub mod serializers;
pub mod snapshot;
pub mod walk;

/// The version of this crate, as a string.
///
/// Read by the engine's `#[napi]` bindings so the TypeScript surface can assert
/// that the addon it loaded is the addon it was compiled against — a stale
/// `.node` file next to a fresh `dist/` is otherwise a silent mismatch.
///
/// # Examples
///
/// ```
/// // One workspace version for everything — this is the same string the
/// // npm package's manifest must carry, and a semver one at that.
/// assert_eq!(pathway_fs_core::VERSION.split('.').count(), 3);
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Node-API version this engine is built against.
///
/// The bindings target the highest `napi` feature the crate enables. The
/// TypeScript `engines.node` range and the CI matrix have to be a subset of
/// what this constant implies, or the addon will load on a runtime that cannot
/// satisfy it.
///
/// # Examples
///
/// ```
/// // The TypeScript loader refuses a runtime whose `process.versions.napi`
/// // is below this floor, at import rather than at the first missing symbol.
/// assert!(pathway_fs_core::NAPI_VERSION >= 8);
/// ```
pub const NAPI_VERSION: u32 = 8;

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_the_manifest_version() {
        assert_eq!(super::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
