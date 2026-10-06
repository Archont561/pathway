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
///
/// # Examples
///
/// ```
/// assert_eq!(pathway_fs_core::hash::sha256::NAME, "sha256");
/// ```
pub const NAME: &str = "sha256";

use sha2::Digest as _;

/// SHA-256 over a stream of chunks.
///
/// # Examples
///
/// The example is on the type rather than repeated on `new`/`update`/`finish`
/// deliberately: it exercises the whole lifecycle, and the invariant it shows
/// — the digest is independent of how the input was chunked — is the property
/// the fused walk relies on.
///
/// ```
/// use pathway_fs_core::hash::sha256::Hasher;
///
/// let mut chunked = Hasher::new();
/// chunked.update(b"he");
/// chunked.update(b"llo");
///
/// let mut whole = Hasher::new();
/// whole.update(b"hello");
///
/// assert_eq!(chunked.finish(), whole.finish());
/// ```
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

impl crate::hash::Hasher for Hasher {
    fn update(&mut self, chunk: &[u8]) {
        Self::update(self, chunk);
    }

    fn finish(self: Box<Self>) -> String {
        (*self).finish()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha256_is_named_by_the_algorithm_string_the_api_exposes() {
        assert_eq!(super::NAME, "sha256");
    }
}
