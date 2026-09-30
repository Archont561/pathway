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
