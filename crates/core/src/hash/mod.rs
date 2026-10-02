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
//!
//! That shared reader is [`hash_reader`], and it is the only place in the crate
//! that opens a file to hash it. Each algorithm module contributes a `Hasher`
//! that knows how to absorb a chunk and produce a digest, and nothing else —
//! none of them ever sees a `Path` or a `File`, so none of them can accidentally
//! grow a `read_to_end`.

pub mod blake3;
pub mod sha256;
pub mod xxhash;

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use crate::error::{Error, Result};

/// The read size of one chunk, in bytes.
///
/// 64 KiB, which is the figure the reference implementation fixed and the
/// benchmark baseline assumes. It is deliberately a constant rather than an
/// option: a caller who tunes it is tuning against their own disk, and the
/// number that matters for the claim in `.knowledge/architecture/fused-walk.md`
/// is the one every measurement was taken at.
pub const CHUNK_SIZE: usize = 64 * 1024;

/// Which hash algorithm to compute.
///
/// A closed enum, and the string spelling is resolved once by
/// [`Algorithm::from_name`] rather than matched per entry. A walk over 100 000
/// files must not re-interpret the word `"blake3"` 100 000 times, and an
/// unrecognised name is a caller's mistake that should be reported before the
/// walk starts rather than attached to every entry in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Algorithm {
    /// BLAKE3 — the default.
    Blake3,
    /// XXH3 — fast, non-cryptographic.
    Xxhash,
    /// SHA-256 — the interoperability hash.
    Sha256,
}

impl Algorithm {
    /// Resolves the public spelling of an algorithm.
    ///
    /// The names are the ones the TypeScript `HasherName` union exposes, so the
    /// two surfaces cannot disagree about what `"xxhash"` means. No aliases: a
    /// hasher whose strength is chosen by how the caller was feeling is a hasher
    /// nobody can audit, which is why `"fast"` is not accepted for XXH3.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownHasher`] if the name is not one of the three.
    pub fn from_name(name: &str) -> Result<Self> {
        match name {
            blake3::NAME => Ok(Self::Blake3),
            xxhash::NAME => Ok(Self::Xxhash),
            sha256::NAME => Ok(Self::Sha256),
            other => Err(Error::UnknownHasher(other.to_owned())),
        }
    }

    /// The public spelling of this algorithm.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Blake3 => blake3::NAME,
            Self::Xxhash => xxhash::NAME,
            Self::Sha256 => sha256::NAME,
        }
    }
}

impl std::str::FromStr for Algorithm {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        Self::from_name(name)
    }
}

/// One algorithm's in-progress state.
///
/// An enum rather than `Box<dyn Trait>`: there are exactly three algorithms and
/// the set is closed, so dynamic dispatch would buy nothing and cost an
/// indirection on the hottest loop in the crate.
///
/// The variants differ in size — BLAKE3's state is ~1.9 KiB against SHA-256's
/// ~0.1 KiB — and `clippy::large_enum_variant` would have the big one boxed.
/// Not here: exactly one of these exists per *file*, it is built on the stack,
/// it is never moved or stored in a collection, and it is updated once per
/// 64 KiB chunk. Boxing would trade a few stack bytes that nobody copies for a
/// heap allocation per file, which on a 100 000-file walk is 100 000
/// allocations bought with nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum State {
    Blake3(blake3::Hasher),
    Xxhash(xxhash::Hasher),
    Sha256(sha256::Hasher),
}

impl State {
    fn new(algorithm: Algorithm) -> Self {
        match algorithm {
            Algorithm::Blake3 => Self::Blake3(blake3::Hasher::new()),
            Algorithm::Xxhash => Self::Xxhash(xxhash::Hasher::new()),
            Algorithm::Sha256 => Self::Sha256(sha256::Hasher::new()),
        }
    }

    fn update(&mut self, chunk: &[u8]) {
        match self {
            Self::Blake3(h) => h.update(chunk),
            Self::Xxhash(h) => h.update(chunk),
            Self::Sha256(h) => h.update(chunk),
        }
    }

    fn finish(self) -> String {
        match self {
            Self::Blake3(h) => h.finish(),
            Self::Xxhash(h) => h.finish(),
            Self::Sha256(h) => h.finish(),
        }
    }
}

/// Hashes everything a reader yields, in [`CHUNK_SIZE`] blocks.
///
/// The buffer is allocated once and reused for every chunk, so hashing a 10 GiB
/// file allocates 64 KiB. This is the function the "never whole" rule lives in:
/// there is no `read_to_end` anywhere in this crate's hashing path, and a change
/// that introduced one would have to be made here.
///
/// # Errors
///
/// Any read error from the underlying reader, attributed to `path`.
pub fn hash_reader(mut reader: impl Read, algorithm: Algorithm, path: &Path) -> Result<String> {
    let mut state = State::new(algorithm);
    let mut buffer = vec![0u8; CHUNK_SIZE];

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|source| Error::io("read", path, source))?;
        if read == 0 {
            break;
        }
        state.update(&buffer[..read]);
    }

    Ok(state.finish())
}

/// Hashes a file's contents, reading it in [`CHUNK_SIZE`] blocks.
///
/// Returns a lowercase hex digest. The file is never loaded whole — see
/// [`hash_reader`].
///
/// # Errors
///
/// [`Error::Io`] if the file cannot be opened or read.
///
/// # Examples
///
/// ```
/// use pathway_fs_core::hash::{hash_file, Algorithm};
///
/// let dir = tempfile::tempdir()?;
/// let file = dir.path().join("greeting.txt");
/// std::fs::write(&file, b"hello")?;
///
/// let digest = hash_file(&file, Algorithm::Sha256)?;
/// assert_eq!(
///     digest,
///     "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn hash_file(path: &Path, algorithm: Algorithm) -> Result<String> {
    let file = File::open(path).map_err(|source| Error::io("open", path, source))?;
    hash_reader(BufReader::new(file), algorithm, path)
}

#[cfg(test)]
mod tests {
    use super::{hash_file, hash_reader, Algorithm, CHUNK_SIZE};
    use crate::error::Error;

    /// The three well-known digests of the empty input. If a refactor ever
    /// changes what "no bytes at all" hashes to, it changed the algorithm.
    #[test]
    fn empty_input_produces_the_published_digests() {
        let empty: &[u8] = b"";
        let path = std::path::Path::new("<memory>");

        assert_eq!(
            hash_reader(empty, Algorithm::Sha256, path).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hash_reader(empty, Algorithm::Blake3, path).unwrap(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        // XXH3-64 of the empty input.
        assert_eq!(
            hash_reader(empty, Algorithm::Xxhash, path).unwrap(),
            "2d06800538d394c2"
        );
    }

    #[test]
    fn sha256_matches_the_reference_digest_of_abc() {
        let path = std::path::Path::new("<memory>");
        assert_eq!(
            hash_reader(&b"abc"[..], Algorithm::Sha256, path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// The property the chunking has to preserve: a digest must not depend on
    /// how the bytes were divided up on the way in. This is the test that would
    /// catch a hasher that reset its state per chunk, or one that hashed the
    /// whole reusable buffer instead of `&buffer[..read]`.
    #[test]
    fn a_digest_does_not_depend_on_the_chunk_boundaries() {
        let dir = tempfile::tempdir().unwrap();
        // Deliberately larger than one chunk and not a multiple of it, so the
        // final read is short and the buffer still holds stale bytes.
        let payload: Vec<u8> = (0..(CHUNK_SIZE * 2 + 1337))
            .map(|i| u8::try_from(i % 251).unwrap())
            .collect();

        let file = dir.path().join("big.bin");
        std::fs::write(&file, &payload).unwrap();

        for algorithm in [Algorithm::Blake3, Algorithm::Xxhash, Algorithm::Sha256] {
            let from_file = hash_file(&file, algorithm).unwrap();
            let in_one_go = hash_reader(payload.as_slice(), algorithm, &file).unwrap();
            assert_eq!(from_file, in_one_go, "{} disagreed", algorithm.name());
        }
    }

    #[test]
    fn every_algorithm_round_trips_through_its_name() {
        for algorithm in [Algorithm::Blake3, Algorithm::Xxhash, Algorithm::Sha256] {
            assert_eq!(Algorithm::from_name(algorithm.name()).unwrap(), algorithm);
        }
    }

    #[test]
    fn an_unknown_hasher_names_itself_and_the_alternatives() {
        let error = Algorithm::from_name("md5").unwrap_err();
        assert!(matches!(error, Error::UnknownHasher(ref name) if name == "md5"));
        let message = error.to_string();
        assert!(message.contains("md5"), "{message}");
        assert!(message.contains("blake3"), "{message}");
    }

    /// `"fast"` is not an alias for XXH3, deliberately — see `xxhash.rs`.
    #[test]
    fn strength_cannot_be_chosen_by_a_vague_alias() {
        assert!(Algorithm::from_name("fast").is_err());
        assert!(Algorithm::from_name("xxh3").is_err());
    }

    #[test]
    fn a_missing_file_reports_the_path_it_could_not_open() {
        let error =
            hash_file(std::path::Path::new("/nope/missing.bin"), Algorithm::Blake3).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("missing.bin"), "{message}");
        assert!(message.contains("open"), "{message}");
    }
}
