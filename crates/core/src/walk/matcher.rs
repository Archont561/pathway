//! The glob and regex filter.
//!
//! Two rules, and they are not symmetric — this is the single most bug-prone
//! part of the traversal:
//!
//! - A **vector** of glob patterns, combined with **AND** semantics, matched
//!   against **root-relative** paths. Root-relative, not absolute: a `**/*.ts`
//!   must match `src/a.ts` when the root is `src`, and matching the absolute
//!   path would make every pattern depend on where the checkout lives.
//! - A **regex**, matched against the **full absolute path**, because that is
//!   what a regex written by a user almost always means.
//!
//! The reference implementation conflated these and globbed a single `String`.
//! Spec, including the glob-semantics matrix (nested vs. root-level `**/*.ts`,
//! `*.ts`, Windows separators): `.knowledge/implementation/code-rust-walker.md`.
