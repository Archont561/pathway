//! Filesystem primitives: atomic writes, sandboxing, locks, temp directories.
//!
//! These are the v0.2 and v0.3 deliverables, and they are the reason the crate
//! is more than a glob library. Each one is a place where the obvious
//! implementation is wrong in a way that only shows up under concurrency, so
//! each carries its hazard in the module docs rather than in a comment nobody
//! re-reads.
//!
//! ## Planned layout
//!
//! - `atomic.rs` — write via `O_EXCL` temp file, `fsync`, `rename`, then a
//!   best-effort `fsync` of the *parent directory*. The last step is the one
//!   that is usually omitted and it is the only thing that makes the rename
//!   survive a power loss; it is best-effort because Windows has no equivalent
//!   for a directory handle.
//! - `sandbox.rs` — path containment. Per-component `realpath` plus
//!   `openat(O_NOFOLLOW)`, not a `canonicalize` of the final path: a final-path
//!   check misses an intermediate symlink escape and leaves a TOCTOU window
//!   between the check and the use.
//! - `lock.rs` — `flock` on Unix, `LockFileEx` on Windows, with a sidecar-file
//!   option for filesystems where advisory locks are unreliable (NFS), and the
//!   caveats documented rather than discovered.
//! - `temp.rs` — temp directories via `tempfile`, with `O_TMPFILE` /
//!   `DELETE_ON_CLOSE` for the hard cleanup guarantee on platforms that have
//!   them, and tiered cleanup documented for the ones that do not
//!   (`.knowledge/features/killer-features.md`).

pub mod atomic;
pub mod lock;
pub mod sandbox;
pub mod temp;
