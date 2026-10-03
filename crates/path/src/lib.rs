//! The Rust surface of pathway: pathlib's convenience, over the same core.
//!
//! Decision D7 says one engine has two ergonomic surfaces. The TypeScript one is
//! the primary product; this one exists so a Rust project gets the same
//! behaviour without going through Node. Both are thin, and a rule implemented
//! here instead of in [`pathway_fs_core`] is a bug waiting to happen — the
//! surfaces are supposed to agree.
//!
//! # Rules that differ from the TypeScript surface, on purpose
//!
//! - **`std::path`, not `pathe`.** The TypeScript surface normalises to POSIX
//!   separators because a JavaScript string is a string and Windows paths would
//!   otherwise leak `\` into every comparison. A Rust `PathBuf` already has
//!   platform-native semantics and a `Display` that does the right thing, so
//!   "normalising" here would be a second, wrong idea of what a path is.
//! - **Independent semver and MSRV.** This crate is not released in lockstep
//!   with the npm package and does not have to be. The one rule that does bind
//!   it: a gap in the Rust surface must never block an npm release, because npm
//!   is the primary product.
//!
//! # Timeline
//!
//! A preview at v0.3, 1.0 at v1.0 with docs.rs coverage and an MSRV enforced in
//! CI. Nothing here is written yet — see
//! `.knowledge/architecture/rust-crate-surface.md` for the full surface and
//! `backlog/docs/phase-plan.md` for the phase it lands in.

#![deny(missing_docs)]
#![warn(clippy::pedantic)]

/// The version of this crate, as a string.
///
/// # Examples
///
/// ```
/// // Phase 1 releases everything in lockstep from the workspace version;
/// // independent semver begins when the preview ships (see the timeline).
/// assert_eq!(pathway_fs::VERSION, pathway_fs::core::VERSION);
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The core engine this surface is a view over.
///
/// Re-exported so a consumer that depends only on `pathway-fs` can still reach
/// the low-level API — the curated core surface is the escape hatch from the
/// ergonomic one, and hiding it would make the ergonomic surface the only door.
///
/// # Examples
///
/// ```
/// use pathway_fs::core::hash::Algorithm;
///
/// // The whole engine is reachable through the re-export.
/// assert_eq!(Algorithm::from_name("blake3")?.name(), "blake3");
/// # Ok::<(), pathway_fs::core::error::Error>(())
/// ```
pub use pathway_fs_core as core;

#[cfg(test)]
mod tests {
    #[test]
    fn the_crate_binds_to_the_workspace_core() {
        // Not a tautology: it fails at compile time if the dependency is
        // renamed, which is the moment the re-export above would break.
        assert_eq!(super::core::VERSION, super::VERSION);
    }
}
