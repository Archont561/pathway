//! Traversal: the fused walk.
//!
//! This is the module the project exists for, and the one every other decision
//! in the knowledge base serves. The competitors — `fdir`, `tinyglobby`,
//! `Bun.Glob.scan()`, and `node:fs.glob` in Node core — return *paths*. A real
//! application then stats, hashes and filters each of those paths, paying a
//! boundary crossing per file. Here, traversal + `stat` + hash + filter happen
//! in one syscall pass across OS worker threads and yield fully populated
//! batches (D3).
//!
//! ## Planned layout
//!
//! - `scanner.rs` — [`NativeScanner`](scanner::NativeScanner), the
//!   `ignore`-backed walker. This is where `dot` (default `false`, so
//!   `hidden(true)`) and `gitignore` live, because both are properties of the
//!   walker rather than of the matcher.
//! - `matcher.rs` — the glob/regex filter. A **vector** of globs with AND
//!   logic, matched against **root-relative** paths, and a regex matched
//!   against the full absolute path. The two differ on purpose; see
//!   `.knowledge/implementation/code-rust-walker.md` for why the reference
//!   implementation got this wrong.
//! - `entry.rs` — `FusedEntry`, one yielded item: path, stat, and optionally a
//!   content hash plus a per-entry `error`.
//!
//! ## Results are batches, not streams of one
//!
//! The natural yield unit is a batch (default 512, to be tuned empirically —
//! see Q3 in the context file), not a single entry. That is what makes the fused
//! pass worth the FFI boundary: the boundary is crossed once per batch instead
//! of once per file. Whether batching stays load-bearing depends on the Step 0
//! NAPI-RS iterator spike: if `#[napi(async_iterator)]` is adopted it becomes a
//! prefetch window, and if it is not, it is the mechanism.

pub mod entry;
pub mod matcher;
pub mod scanner;

pub use entry::FusedEntry;
// `Matcher`, `ScanOptions` and `Scanner` are re-exported here when their
// implementations land. Declaring the re-exports ahead of the types would make
// the module tree depend on code that does not exist, which is the same
// mistake as a stub `pub use` for a module that was never written.
