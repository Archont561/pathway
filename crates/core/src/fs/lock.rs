//! File locking.
//!
//! `flock` on Unix, `LockFileEx` on Windows, both advisory: they coordinate
//! processes that use this library and nothing else. A lock this library takes
//! is not a lock against a program that never asks for one, and the docs for
//! `withLock` have to say so.
//!
//! Two failure modes worth stating up front rather than discovering:
//!
//! - **NFS.** `flock` is advisory *and* was historically a no-op on some NFS
//!   versions and mount options. The sidecar-file option exists for that case:
//!   an `O_EXCL` create is portable, at the cost of leaving a file behind when
//!   a process is `SIGKILL`ed, which then needs a staleness check.
//! - **Re-entrancy.** `flock` locks are per open file description, not per
//!   process, so a second `open` of the same path in the same process gets a
//!   *different* lock and deadlocks against itself. Taking the lock once per
//!   `Path` and reusing the descriptor is the fix, and it is a design
//!   constraint rather than an optimisation.
