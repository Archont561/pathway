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
//! Scaffold. The module tree below is the intended shape; the modules are
//! declared empty and each carries the spec it will be built from. The phase
//! plan is `backlog/docs/phase-plan.md`.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]

pub mod error;
pub mod fs;
pub mod hash;
pub mod serializers;
pub mod walk;

/// The version of this crate, as a string.
///
/// Read by the engine's `#[napi]` bindings so the TypeScript surface can assert
/// that the addon it loaded is the addon it was compiled against — a stale
/// `.node` file next to a fresh `dist/` is otherwise a silent mismatch.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Node-API version this engine is built against.
///
/// The bindings target the highest `napi` feature the crate enables. The
/// TypeScript `engines.node` range and the CI matrix have to be a subset of
/// what this constant implies, or the addon will load on a runtime that cannot
/// satisfy it.
pub const NAPI_VERSION: u32 = 8;

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_the_manifest_version() {
        assert_eq!(super::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
