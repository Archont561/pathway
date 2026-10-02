//! XXH3 — the fast non-cryptographic hasher.
//!
//! For cache keys and change detection, where the input is not adversarial and
//! a collision costs a rebuild rather than a compromise. That is a real
//! distinction and not a hedge: `xxhash-rust` with the `xxh3` feature is several
//! times faster than BLAKE3 on large inputs, and the gap widens with file size.
//!
//! It is never the default, and the API should make the caller say `xxhash`
//! out loud rather than accept a `"fast"` alias — a hasher whose strength is
//! chosen by how the caller was feeling is a hasher nobody can audit.

/// The identifier this hasher is selected by.
pub const NAME: &str = "xxhash";

/// XXH3-64 over a stream of chunks.
pub struct Hasher(xxhash_rust::xxh3::Xxh3);

// By hand, because `Xxh3` does not implement `Debug` and this crate warns on
// `missing_debug_implementations`. The state is a 500-odd byte accumulator that
// would be noise in any log, so what is printed is the one useful fact: which
// algorithm this is.
impl std::fmt::Debug for Hasher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Hasher")
            .field("algorithm", &NAME)
            .finish_non_exhaustive()
    }
}

impl Hasher {
    /// Starts a new digest.
    #[must_use]
    pub fn new() -> Self {
        Self(xxhash_rust::xxh3::Xxh3::new())
    }

    /// Absorbs one chunk.
    pub fn update(&mut self, chunk: &[u8]) {
        self.0.update(chunk);
    }

    /// Finalises the digest as lowercase hex.
    ///
    /// Sixteen characters, not sixty-four: XXH3-64 is a 64-bit digest, and
    /// zero-padding it to look like a cryptographic one would invite exactly
    /// the confusion this module's header warns about. Big-endian so the text
    /// sorts the same way the number does.
    #[must_use]
    pub fn finish(self) -> String {
        let digest = self.0.digest();
        digest
            .to_be_bytes()
            .iter()
            .fold(String::with_capacity(16), |mut out, byte| {
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
    fn xxhash_is_named_by_the_algorithm_string_the_api_exposes() {
        assert_eq!(super::NAME, "xxhash");
    }

    #[test]
    fn the_digest_is_sixty_four_bits_of_hex_and_not_padded_to_look_stronger() {
        let hasher = super::Hasher::new();
        assert_eq!(hasher.finish().len(), 16);
    }
}
