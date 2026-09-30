//! Path containment (the `sandbox` feature).
//!
//! A sandboxed path must not escape its root. The obvious implementation —
//! `canonicalize` the candidate, `canonicalize` the root, check
//! `starts_with` — is wrong twice over:
//!
//! 1. It misses an *intermediate* symlink. A root containing `link -> /etc`, and
//!    a candidate of `root/link/passwd`, canonicalizes to `/etc/passwd`, which
//!    is not under the root, so that case is caught — but only because the
//!    final component moved. A symlink placed in the middle of a path that
//!    resolves back inside the root passes the check while the process still
//!    touched a file outside it mid-traversal.
//! 2. It is a TOCTOU window. The check and the subsequent open are two
//!    operations, and an attacker with write access to any component can swap
//!    the symlink in between.
//!
//! The real implementation walks one component at a time with
//! `openat(O_NOFOLLOW)` from a directory descriptor opened on the root, so
//! there is no window and no name to re-resolve. That is also why the error
//! type is a distinct [`crate::error::Error::Escaped`] rather than an
//! `io::Error`: refusing to leave the root is not a filesystem failure, and a
//! caller that has to tell the two apart should not have to match on a message.
