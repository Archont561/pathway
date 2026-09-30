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

#[cfg(test)]
mod tests {
    #[test]
    fn blake3_is_named_by_the_algorithm_string_the_api_exposes() {
        assert_eq!(super::NAME, "blake3");
    }
}
