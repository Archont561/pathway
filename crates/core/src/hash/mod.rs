//! Content hashing.
//!
//! Hashing is half of the fused walk's claim (D3): the competitor path is
//! `glob` → `stat` → `readFile` → `crypto.createHash`, which is a read of the
//! whole file into the JavaScript heap followed by a hash. Here the file is read
//! in **64 KiB chunks and never loaded whole**, so peak memory is a constant
//! rather than a function of file size, and the bytes never cross the language
//! boundary.
//!
//! ## Planned layout
//!
//! - `blake3.rs` — the default. Fastest of the three for bulk work and the
//!   reason the Rust `blake3` crate compiles its C sources with `cc`; the
//!   devcontainer provisions that toolchain as a *global* pixi install
//!   precisely so it does not end up in the published sandbox branch.
//! - `xxhash.rs` — non-cryptographic, for cache keys and change detection where
//!   the input is not adversarial.
//! - `sha256.rs` — the interoperability hash, because everything else that has
//!   ever hashed a file agrees on this one.
//!
//! All three read in chunks through one shared reader, so the "never whole" rule
//! has exactly one implementation to get right.

pub mod blake3;
pub mod sha256;
pub mod xxhash;
