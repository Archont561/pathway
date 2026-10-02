//! BLAKE3 — the default hasher.
//!
//! Fastest of the three for bulk work, which is the only criterion that matters
//! here: the fused walk's claim is measured on a large tree, and the default is
//! what the benchmark uses.
//!
//! One implementation note that is not obvious from the API: the `blake3` crate
//! compiles its C sources with the `cc` crate, so a build without a C toolchain
//! fails rather than silently falling back to a slower path. The devcontainer
//! installs `cc`/`gcc`/`ar` as a *global* pixi install for exactly this reason —
//! a feature dependency would put a ~200 MB toolchain into the published sandbox
//! branch, where it builds nothing an airlock restores.

/// The identifier this hasher is selected by.
pub const NAME: &str = "blake3";

/// BLAKE3 over a stream of chunks.
///
/// `::blake3` rather than `blake3`: this module *is* `crate::hash::blake3`, so
/// the bare name resolves here and not to the crate. Naming the module after
/// the algorithm is worth that one leading `::`.
#[derive(Debug)]
pub struct Hasher(::blake3::Hasher);

impl Hasher {
    /// Starts a new digest.
    #[must_use]
    pub fn new() -> Self {
        Self(::blake3::Hasher::new())
    }

    /// Absorbs one chunk.
    pub fn update(&mut self, chunk: &[u8]) {
        self.0.update(chunk);
    }

    /// Finalises the digest as lowercase hex.
    #[must_use]
    pub fn finish(self) -> String {
        self.0.finalize().to_hex().to_string()
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
    fn blake3_is_named_by_the_algorithm_string_the_api_exposes() {
        assert_eq!(super::NAME, "blake3");
    }
}
