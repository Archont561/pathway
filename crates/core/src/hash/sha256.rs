//! SHA-256 — the interoperability hasher.
//!
//! Slower than the other two by design, and present because everything else that
//! has ever hashed a file agrees on this one. When the output has to be compared
//! against a lockfile, a `go.sum`, a CI cache key or a `sha256sum` printed by a
//! human, matching the algorithm is the requirement and the speed is not.
//!
//! It is also the hash the benchmark's competitor baseline uses (`crypto` in
//! JavaScript, `sha256sum` on the command line), so the comparison in
//! `backlog/docs/phase-plan.md` is only fair if this one is
//! implemented rather than skipped.

/// The identifier this hasher is selected by.
pub const NAME: &str = "sha256";

use sha2::Digest as _;

/// SHA-256 over a stream of chunks.
#[derive(Debug)]
pub struct Hasher(sha2::Sha256);

impl Hasher {
    /// Starts a new digest.
    #[must_use]
    pub fn new() -> Self {
        Self(sha2::Sha256::new())
    }

    /// Absorbs one chunk.
    pub fn update(&mut self, chunk: &[u8]) {
        self.0.update(chunk);
    }

    /// Finalises the digest as lowercase hex.
    #[must_use]
    pub fn finish(self) -> String {
        // Hex by hand rather than through another dependency: 32 bytes, and
        // `hex` would be a crate in the published graph for one format string.
        self.0
            .finalize()
            .iter()
            .fold(String::with_capacity(64), |mut out, byte| {
                use std::fmt::Write as _;
                let _ = write!(out, "{byte:02x}");
                out
            })
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha256_is_named_by_the_algorithm_string_the_api_exposes() {
        assert_eq!(super::NAME, "sha256");
    }
}
