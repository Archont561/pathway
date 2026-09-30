//! Temporary files and directories.
//!
//! Cleanup is tiered by what the platform can actually guarantee, and the
//! guarantee is documented per tier instead of being implied by a name that
//! promises more than the OS delivers:
//!
//! - **Tier 1 — guaranteed.** An exception, a `return`, or a normal exit runs
//!   the cleanup, so `Drop` is enough.
//! - **Tier 2 — best effort.** `SIGKILL`, a power loss, or the OOM killer skip
//!   `Drop` entirely. `O_TMPFILE` (Linux) and `DELETE_ON_CLOSE` (Windows)
//!   extend the guarantee to these cases, at the cost of an unnamed file that
//!   cannot be inspected or handed to another process. That cost is why they are
//!   opt-in rather than the default.
//! - **Tier 3 — documented only.** A temp directory on a filesystem that
//!   supports none of the above, cleaned by age on the next run.
//!
//! `tempfile` implements tiers 1 and 2, and it is already a dependency.
//! Reimplementing it would be a way to ship a weaker version of a solved
//! problem, so this module is the tier documentation and the option plumbing,
//! not another temp dir implementation.
