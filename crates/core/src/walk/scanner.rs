//! The `ignore`-backed walker.
//!
//! Wraps the `ignore` crate (ripgrep's walker) and owns the options that are
//! properties of the *walk* rather than of the filter: `dot` (default `false`,
//! which means `hidden(true)` — hidden entries are skipped unless asked for)
//! and `gitignore` (`.gitignore` and `.ignore` honoured, plus parent-directory
//! lookups, all of which `ignore` gives for free).
//!
//! The filter is [`super::matcher`]'s job. Keeping the split there means
//! adding a filter never has to re-derive `hidden`/`gitignore`, which is where
//! a traversal silently misses files.
//!
//! Spec: `.knowledge/implementation/code-rust-walker.md`.
