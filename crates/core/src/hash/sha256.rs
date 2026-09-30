//! SHA-256 — the interoperability hasher.
//!
//! Slower than the other two by design, and present because everything else that
//! has ever hashed a file agrees on this one. When the output has to be compared
//! against a lockfile, a `go.sum`, a CI cache key or a `sha256sum` printed by a
//! human, matching the algorithm is the requirement and the speed is not.
//!
//! It is also the hash the benchmark's competitor baseline uses (`crypto` in
//! JavaScript, `sha256sum` on the command line), so the comparison in
//! `.knowledge/implementation/phase-plan.md` is only fair if this one is
//! implemented rather than skipped.

/// The identifier this hasher is selected by.
pub const NAME: &str = "sha256";
