---
type: Feature Spec
title: "Killer Features: Temp Dirs, Snapshots, Sandbox, Transactions, Locking, Parallel Ops"
description: "Differentiating filesystem features: temp dirs, snapshots and diff, sandboxing, transactions, locking, and parallel operations."
tags: [temp, snapshot, diff, sandbox, transaction, lock, parallel, security]
status: draft
generated:
  by: pathway_kb/1.0
  at: 2026-10-07T00:00:00Z
verified:
  - by: human:archont561
    at: 2025-07-11T00:00:00Z
  - by: process:gap-analysis-2026-09
    at: 2026-09-16T00:00:00Z
  - by: process:benchmark-task-4
    at: 2026-10-03T00:00:00Z
  - by: process:task-12-session
    at: 2026-10-07T00:00:00Z
  - by: process:task-14-session
    at: 2026-10-07T00:00:00Z
domain: features
decision: proposed  # legacy KB status (decided|proposed|deprecated)
created: 2025-07-11
source: conversation
depends_on:
  - architecture/core-layers
  - architecture/napi-boundary
  - features/walk-traversal
  - features/serializers
---

# Killer Features (v0.2–v1.0)

## Overview

These are the features that would make someone uninstall `fs-extra`, `glob`,
`fast-glob`, `tmp`, `proper-lockfile`, and `chokidar` in one move. Each one
solves a real, painful problem that currently requires stitching together
multiple libraries with fragile glue code.

All features in this document are **proposed** for v0.2+ and are not part
of the Phase 1 implementation. They are included here to ensure the v0.1
architecture does not accidentally foreclose on them.

> **Scope note (Sept 2026):** `watch()` is delivered in **Phase 4 (v0.4)**
> via the `notify` crate per [phase-plan.md](../../backlog/docs/phase-plan.md)
> — the "uninstall chokidar" claim above holds at v0.4, not v0.2.
> Incumbent context: chokidar 5.0.0 (Nov 2025) is ESM-only / Node ≥20.19;
> `@parcel/watcher` is the native watcher used by Parcel, VS Code, and
> Effect Platform.

---

## 1. Scoped Temp Directories (RAII Cleanup)

### The Pain

Every JS project has this somewhere:

```ts
const tmp = await fs.mkdtemp("/tmp/myapp-");
try {
  // ... do work ...
} finally {
  await fs.rm(tmp, { recursive: true, force: true });
}
```

Problems:
- Verbose and easy to forget.
- `finally` block is skipped on `process.exit()` or `SIGINT`.
- No OS-level cleanup guarantee.
- Temp files leak on crashes, filling up `/tmp` over time.

### The API

```ts
await Path.temp(async (dir) => {
  const scratch = dir.join("build-cache");
  await scratch.mkdir();
  await scratch.join("output.js").writeText(bundle);
  // ...
});
// dir is gone: on return, on throw, and on process.exit().
// On SIGINT it is gone only if the host installed a handler that exits
// cleanly (Pathway does not install signal handlers for you); a SIGKILL
// runs no code at all. See the tier table below.
```

With options:

```ts
await Path.temp({ prefix: "build-", dir: "/fast-ssd" }, async (dir) => {
  // Temp dir created on /fast-ssd with prefix "build-"
});
```

### Why Rust Matters (corrected Sept 2026, corrected again 2026-10-07 from measurement)

The 2025 draft overstated the guarantee twice, and the second correction came
from probes rather than from reading `tempfile`'s README:

1. **`tempfile` has no atexit hook.** Its guarantee *is* the destructor. A
   `process.exit()` runs no destructors, so tier 1 only reaches that exit path
   when the host adds a flush of its own — Pathway registers every live temp
   directory in the core and flushes the registry from a
   `process.on("exit")` listener installed on first use.
2. **A temp directory can never be tier 2.** `O_TMPFILE` provides anonymity by
   creating no directory entry, and the kernel ignores the `O_DIRECTORY` bit:
   `open(dir, O_TMPFILE | O_DIRECTORY | O_RDWR)` returns an unnamed *regular*
   file, and `openat` inside it fails `ENOTDIR`. A path the caller can hand to a
   child process is a directory entry, so the hard guarantee is a property of
   unnamed temp **files** only. That primitive is not implemented yet; it is
   filed as its own backlog task.

The guarantee is therefore stated per tier, with what each row was measured on:

| Tier | Mechanism | Survives |
|------|-----------|----------|
| 1 | `tempfile` Drop, plus the core's registry flushed from a `process.on("exit")` hook | a return, a throw, `process.exit()`, normal GC, and a `SIGINT`/`SIGTERM` whose handler exits cleanly |
| 2 | **Linux:** `O_TMPFILE`; **Windows:** `FILE_FLAG_DELETE_ON_CLOSE` | `SIGKILL` (local FS) — **for unnamed temp files only** |
| 3 | documented only | a `SIGKILL`, a `SIGINT`/`SIGTERM` under the default disposition, or a power loss: the tree stays on disk until something reaps it |

The cleanup happens at the **file descriptor / OS level** where tier 2 is
available, not via JS `finally` blocks that can be skipped. On Node 22 and
Bun 1.3 an `exit` hook runs for `process.exit(0)` and for a signal whose
handler exits, but not for a signal under the default disposition — which is
why Pathway does not install `SIGINT`/`SIGTERM` handlers: that would change
the host process's semantics behind its back.

Measured 2026-10-07 (linux-64), pinned by tests in
`crates/core/src/fs/temp.rs` and `packages/path/test/temp.test.ts`.

### Implementation Notes

```rust
// Rust side
use tempfile::TempDir;

#[napi]
pub struct NativeTempDir {
    inner: TempDir,  // Dropped (cleaned up) when this struct is GC'd
}

#[napi]
impl NativeTempDir {
    #[napi(constructor)]
    pub fn new(prefix: Option<String>, dir: Option<String>) -> Result<Self> {
        let mut builder = tempfile::Builder::new();
        if let Some(p) = prefix { builder.prefix(&p); }
        let inner = if let Some(d) = dir {
            builder.tempdir_in(d)
        } else {
            builder.tempdir()
        }.map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Self { inner })
    }

    #[napi]
    pub fn path(&self) -> String {
        self.inner.path().to_string_lossy().into()
    }
}
```

The `TempDir` destructor runs when the N-API wrapper is garbage-collected,
providing best-effort cleanup even if the JS callback throws, and the core's
registry covers the `process.exit()` path the destructor cannot. The tier-2
flags (`O_TMPFILE`, `FILE_FLAG_DELETE_ON_CLOSE`) do not apply to a directory
at all — see correction 2 above — so a landed `Path.temp` is tier 1 with
tier-3 behavior documented and tested for the paths nothing can cover. Where
the flags are unavailable for *files*, the tier-3 documented behavior applies.

### Phase Target: v0.2

---

## 2. Directory Snapshots & Diffing

### The Pain

Build systems, test runners, and watchers all need to answer *"what changed?"*
Everyone reimplements it badly:

```ts
// Typical fragile implementation
const before = new Map<string, number>();
for (const file of await glob("**/*.ts")) {
  const stat = await fs.stat(file);
  before.set(file, stat.mtimeMs);
}

// ... run compiler ...

const after = new Map<string, number>();
for (const file of await glob("**/*.ts")) {
  const stat = await fs.stat(file);
  after.set(file, stat.mtimeMs);
}

const modified = [...after.entries()]
  .filter(([f, t]) => before.get(f) !== t);
// ❌ Misses deletions. mtime is unreliable. No content hashing.
//    200k fs.stat() calls. Slow.
```

### The API

```ts
const before = await project.snapshot({
  glob: "**/*.{ts,tsx}",
  exclude: ["node_modules"],
  hash: "blake3",       // Content hash, not mtime
});

// ... run compiler, modify files ...

const after = await project.snapshot({
  glob: "**/*.{ts,tsx}",
  exclude: ["node_modules"],
  hash: "blake3",
});

const diff = before.diff(after);

console.log(diff.added);     // Path[]  — new files
console.log(diff.removed);   // Path[]  — deleted files
console.log(diff.modified);  // Path[]  — content changed
console.log(diff.unchanged); // Path[]  — identical content
console.log(diff.hasChanges); // boolean — added/removed/modified is non-empty
```

> **Landed 2026-10-07 (TASK-14).** `Path#snapshot(options?)`,
> `FileSystem#snapshot(root, options?)`, `Snapshot#diff/save/toJSON`,
> `Snapshot.load/parse`, and `SnapshotFormatError`. `exclude` is the spelling
> for pruned directory *names* (a walk option), not a glob list, so the
> example above reads `exclude: ["node_modules"]` exactly as a walk does. What
> is **not** proven is the performance claim set below — see the verification
> note.

### Why Rust Matters

Snapshotting 100k files with content hashes in JS means:
- 100k `fs.stat()` calls across libuv
- 100k `fs.readFile()` calls
- 100k `crypto.createHash()` calls
- ~200MB of JS Buffer allocations
- Severe GC pressure

Rust does it with:
- `ignore` crate parallel traversal (single pass)
- `rayon` parallel hashing across all CPU cores
- Memory-mapped I/O for large files
- Zero JS allocations until the final batch yield

**Verification status: NOT VERIFIED for `snapshot()` (2026-10-07).** Every
number in this subsection was measured on the *fused walk* (task-4's harness:
walk + stat + hash), not on `snapshot()`, and the phase plan's "snapshot +
diff on 100k files in <500ms" has no measurement behind it at all. The
mechanism is shared — a capture is one `NativeScanner` pass — so the walk
figures are the honest prior, not evidence:

- 2026-10-03 locally, 100k files: **1.85x** the strongest baseline (`fdir`
  plus a 32-wide stat/hash pool); 2026-10-04 CI sweep, 10k–1M, both Bun
  lines: **1.06–1.15x**. Peak heap **14.3 MiB vs 39.3 MiB**, GC **2.8 ms vs
  20.6 ms** per sample. The originally projected 10–20x was off by ~6x
  locally and ~10x in CI. See [verified-data.md](/competitive/verified-data.md).
- The snapshot-specific costs that prior does *not* cover: the sorted
  `BTreeMap` fold, JSON serialization of the document, and the diff itself.

Closing this needs a snapshot case in `benches/walk` plus a CI sweep, which is
why TASK-14 keeps its performance acceptance criterion unchecked.

### Persistence

Snapshots can be saved and restored for incremental builds:

```ts
// Save after a full build
await before.save(project.join(".cache/snapshot.json"));

// Restore on next build start
const cached = await Snapshot.load(project.join(".cache/snapshot.json"));
const current = await project.snapshot({ glob: "**/*.ts", hash: "blake3" });
const diff = cached.diff(current);

// Only rebuild what changed
for (const file of diff.modified) {
  await rebuild(file);
}
```

### Snapshot Data Structure

```ts
interface Snapshot {
  readonly timestamp: number;
  readonly root: string;
  readonly entries: Map<string, SnapshotEntry>;

  diff(other: Snapshot): SnapshotDiff;
  save(path: Path): Promise<void>;
  static load(path: Path): Promise<Snapshot>;
}

interface SnapshotEntry {
  readonly size: number;
  readonly mtime: number;
  readonly hash?: string;
}

interface SnapshotDiff {
  readonly added: Path[];
  readonly removed: Path[];
  readonly modified: Path[];
  readonly unchanged: Path[];
}
```

> **Shipped shape (2026-10-07).** `entries` is a `ReadonlyMap<string,
> SnapshotEntry>` keyed by the root-relative path; `SnapshotEntry.modifiedNanos`
> is a `bigint`, not a `Date`, because the whole point is the digits a `Date`
> cannot hold. `save()`/`load()` take a `Path` (or a string) and the document
> is JSON:
>
> ```json
> {"format":"pathway-snapshot-v1","root":"/abs/root","takenAtNanos":"1700000000123456789",
>  "entries":[{"path":"a.ts","size":1,"modifiedNanos":"1700000000123456789","hash":"…"}]}
> ```
>
> Nanoseconds are decimal **strings**: a JSON number is an IEEE-754 double in
> every mainstream parser and a 2026 nanosecond timestamp needs 61 bits. A
> document whose `format` is anything else is refused with
> `SnapshotFormatError` rather than decoded into a diff that would claim the
> whole tree changed. The fold, the comparison rule and the format live in
> `crates/core/src/snapshot`; the TypeScript class is a view over the document,
> so the two surfaces cannot disagree about what "the same tree" means.
>
> **Comparison rule.** Hashes decide when *both* sides carry one, so a file
> rewritten with identical bytes is `unchanged`; otherwise the comparison is
> `size` plus `modifiedNanos`.
>
> **Precision & determinism (Sept 2026):**
> - `SnapshotEntry.mtime` is stored at **full nanosecond precision** in the
>   persisted format (Rust `Duration` carries sub-ms precision; the JS API
>   exposes ms `Date`s). Build systems write files within the same
>   millisecond — ms-only snapshots would falsely report "unchanged".
> - Snapshots fold entries in **sorted path order**, so `save()`/`load()`
>   round-trips and diffs are deterministic across runs and machines
>   (required for cross-build caching).

### Phase Target: v0.2

---

## 3. Safe Path Containment (Anti-Traversal)

### The Pain

Every web server that serves static files has a path traversal vulnerability
waiting to happen:

```ts
// DANGEROUS — classic path traversal
const file = path.join(publicDir, req.params.filename);
// req.params.filename = "../../etc/passwd"
// file = "/etc/passwd" ← escaped the public directory
```

The typical fix is fragile:

```ts
// Fragile — race conditions, symlink escapes, encoding tricks
const resolved = path.resolve(publicDir, userInput);
if (!resolved.startsWith(publicDir)) {
  throw new Error("Access denied");
}
```

Problems with the manual approach:
- Doesn't handle symlinks that point outside the root.
- Doesn't handle URL-encoded `..` (`%2e%2e%2f`).
- Doesn't handle null bytes or Unicode normalization attacks.
- Easy to forget in one route handler.
- No type-level enforcement.

### The API

```ts
const sandbox = project.sandbox("public");

// Safe resolution — throws ContainmentError if path escapes
const safe = sandbox.resolve(userInput);
// ✅ Returns Path if inside "public/"
// ❌ Throws ContainmentError if userInput contains "../"

// Safe join — same guarantee
const file = sandbox.join(userInput);
```

### Why This Is a Killer Feature

This is a **security primitive** that doesn't exist in the JS ecosystem as
a first-class API. Making it a native, zero-cost guarantee on the `Path`
object itself is enormous for server frameworks (Express, Fastify, Hono,
Elysia, etc.).

### Implementation

The current TypeScript seam is `FileSystem.sandbox(root)`. It canonicalizes the
existing root, returns `SandboxPath` values, and applies the same guard to
`resolve()`, `join()`, `parent`, and filesystem I/O:

```ts
const sandbox = FileSystem.create().sandbox("public");
const file = sandbox.resolve(userInput); // SandboxPath
await file.readText();                  // revalidates before opening
```

The guard first performs a separator-aware, host-normalized containment check.
It then finds the deepest existing component and resolves that component with
`realpath`; this catches intermediate symlink escapes, loops, and broken links
while still allowing a non-existing final component for a write. `ContainmentError`
is a distinct exported error with `path` and `root` fields. `SandboxPath` keeps
its sandbox identity across `join()` and `parent` instead of returning an
unconfined `Path`.

### Native descriptor primitive

The core also now exposes an internal Rust `fs::sandbox::Sandbox::open_read`
primitive. On Unix it holds the root directory descriptor and walks components
with `openat(O_NOFOLLOW)`, rejecting symlinks rather than following them. This
makes the descriptor-based read path race-resistant against replacement of
components after construction. It is deliberately not duplicated in the
TypeScript path layer: the public path API remains compatible with existing
Node I/O and performs a documented check immediately before each operation.

The native core's non-Unix fallback uses canonicalization and a containment
check, so it retains a TOCTOU limitation until an equivalent platform
primitive is implemented. The TypeScript API has the same limitation on every
platform because Node's ordinary path-based `readFile`/`writeFile` calls do not
accept a directory descriptor as their root.

### Type-Level Safety (Stretch Goal)

```ts
type SandboxedPath = Path & { __brand: "sandboxed" };

// Server framework integration
function serveStatic(file: SandboxedPath): Response {
  // Compiler guarantees this path is contained
}

// This won't compile:
serveStatic(Path.cwd().join("etc/passwd"));  // ❌ Not sandboxed
serveStatic(sandbox.join("index.html"));      // ✅ Sandboxed
```

### Hardening Requirements (added Sept 2026)

The final-path canonicalize check above is **not sufficient**:

1. **Intermediate symlinks:** `public/link → /etc` must be rejected even
   when a textual prefix looks safe. The TypeScript view checks the deepest
   existing component with `realpath`; the Rust read primitive uses the stronger
   per-component `openat(2, O_NOFOLLOW)` path, so resolution and open are one
   descriptor-anchored step.
2. **Prefix collision:** `startsWith(root + sep)` must compare against the
   separator-terminated root (the sketch does this; keep the unit test:
   `root = /app/public`, `candidate = /app/public-evil/x` must fail).
3. **Case-insensitive FS:** macOS/Windows default volumes — `Public/` vs
   `public`. Canonicalize both sides (already done) and test.
4. **Unicode normalization:** NFC vs NFD on macOS — test.

**Test matrix:** nested symlink escape, symlink loops, case-insensitive
FS, Unicode normalization, root-exact path, broken symlink, `public-evil`
prefix collision.

### Phase Target: v0.3

---

## 4. Parallel Bulk Operations

### The Pain

Copying, transforming, or processing 50k files in JS means either:
- Sequential `for` loops (correct but slow)
- `Promise.all(files.map(...))` (fast but OOM on large trees)
- `p-limit` or `p-map` (correct but adds a dependency and complexity)

```ts
// Current state of the art in JS
import pMap from "p-map";

await pMap(files, async (file) => {
  const content = await fs.readFile(file, "utf-8");
  const transformed = content.replace(/foo/g, "bar");
  await fs.writeFile(dest(file), transformed);
}, { concurrency: 8 });
// 50k files × 2 I/O ops each = 100k libuv round-trips
// Plus JS string allocation for every file
```

### The API

```ts
// Parallel copy with filtering
await project.copyTo(destination, {
  glob: "**/*.{ts,tsx,css}",
  exclude: ["node_modules", ".git"],
  concurrency: 8,
});

// Parallel transform
await project.transform(destination, {
  glob: "**/*.ts",
  concurrency: 12,
  transform: async (content, path) => {
    return content.replaceAll("process.env.NODE_ENV", '"production"');
  },
});

// Parallel delete
await project.remove({
  glob: "**/*.map",
  concurrency: 16,
});
```

### Why Rust Matters

Rust's `rayon` + `tokio` makes parallel I/O trivial:
- Worker threads handle file reads/writes in parallel.
- No JS heap allocation for file contents (Rust `Vec<u8>`).
- No libuv thread pool contention.
- The JS side only crosses the boundary for the `transform` callback
  (which is explicitly user code and unavoidable).

### Architecture

```
Path.copyTo / Path.moveTo / Path.transform
   │
   ├── collect entries with lstat and root-relative filters
   ├── preserve symlinks unless followSymlinks is requested
   ├── bounded worker pool for file operations
   └── return summary: { copied/transformed, skipped, errors }
```

The current public implementation keeps callback transforms in TypeScript, where
user code can run safely and predictably. It uses a bounded pool rather than
`Promise.all(files.map(...))`, so a large tree does not allocate one pending
promise per file. `moveTo()` uses a same-filesystem rename and falls back to
copy-then-remove for `EXDEV`; it does not reimplement atomic writes. The native
engine remains the planned optimization boundary for large copy trees, while
the public semantics and tests are established here.

Transformer arrays run left to right for every file. A read, transform, or write
failure is recorded for that file and does not prevent other workers from
completing; cancellation and invalid concurrency are still hard failures.

The repository benchmark creates 50,000 files and compares `copyTo()` with a
sequential Node baseline. On 2026-10-04 it measured 906 ms versus 3,957 ms,
for a 4.37x speedup against the documented 3x target. This is intentionally a
reproducible local baseline, not a claim about every `fs-extra` release; an
external `fs-extra.copy()` comparison remains a release-benchmark follow-up.
Run it with `pixi run bench --filter=@repo/bench-copy`.

### Phase Target: v0.3

---

## 5. Transactional Filesystem Operations

### The Pain

"Update these 3 config files and rename this directory — but if any step
fails, roll back everything." This literally doesn't exist in the JS
ecosystem.

```ts
// Current approach: manual, fragile, incomplete
try {
  await fs.writeFile("config.toml", newConfig);
  await fs.writeFile("lock.json", newLock);
  await fs.rename("old-dir", "new-dir");
} catch (err) {
  // 😱 How do you undo the partial writes?
  // What if the process crashes mid-transaction?
}
```

### The API

```ts
await Path.transaction(async (tx) => {
  await tx.write(configPath, toml, newConfig);
  await tx.write(lockPath, json, newLock);
  await tx.rename(oldDir, newDir);
  await tx.remove(staleFile);
});
// All succeed → committed
// Any throw → all rolled back to original state
```

### Implementation Reality

True filesystem transactions don't exist on most OSes (except ZFS/Btrfs
snapshots and NTFS TxF, which is deprecated). The practical implementation
uses a **write-ahead log** pattern:

```
1. BEGIN TRANSACTION
   ├── Create temp staging directory
   └── Copy originals to staging (config.toml.bak, lock.json.bak, etc.)

2. EXECUTE OPERATIONS
   ├── Write config.toml (directly to target)
   ├── Write lock.json (directly to target)
   ├── Rename old-dir → new-dir
   └── Remove staleFile

3a. COMMIT (all operations succeeded)
    └── Delete staging directory

3b. ROLLBACK (any operation failed or process crashed)
    ├── Restore config.toml from staging
    ├── Restore lock.json from staging
    ├── Rename new-dir → old-dir
    └── Recreate staleFile from staging
```

### Crash Recovery

For crash safety, the transaction writes a **journal file** before executing:

```
.staging/tx-12345/
  ├── journal.json     ← List of operations and original file locations
  ├── config.toml.bak  ← Original content
  └── lock.json.bak    ← Original content
```

On startup, the library checks for orphaned journal files and offers
recovery:

```ts
const pending = await Path.recoverTransactions(project);
if (pending.length > 0) {
  await pending[0].rollback();  // Or .commit() if appropriate
}
```

### Limitations (Documented Honestly)

- Not ACID in the database sense. A `SIGKILL` during the rename step may
  leave the filesystem in a partially-applied state that requires manual
  recovery.
- Large transactions (1000+ files) are slow due to the backup copy overhead.
- Does not work across filesystem mount points.
- Symlinks are followed, not preserved.

This is a **best-effort transaction** suitable for configuration updates,
not a database replacement.

### Phase Target: v0.4

---

## 6. Native File Locking

### The Pain

`proper-lockfile` (the most popular JS file locking library) uses:
- Polling (checks every 100–500ms)
- PID files (race conditions on PID reuse)
- Stale detection heuristics (fragile)
- No OS-level guarantees

```ts
// proper-lockfile — fragile
const release = await lockfile.lock("data.json", {
  stale: 30000,
  retries: { retries: 5, factor: 2 },
});
try {
  // ... do work ...
} finally {
  await release();
}
// ❌ Polling-based. Race conditions. Breaks on crashes.
//    PID reuse on Linux can unlock someone else's lock.
```

### The API

```ts
await file.withLock(async () => {
  const data = await file.read(json);
  data.counter++;
  await file.write(json, data);
});
// Lock released automatically, even on crash
```

With options:

```ts
await file.withLock(
  {
    exclusive: true,     // Exclusive (write) vs shared (read) lock
    timeout: 5000,       // Wait up to 5s to acquire lock
    sidecar: true,       // Lock a "<file>.lock" next to the target (default: false)
  },
  async () => {
    // Critical section
  }
);
```

> **Sept 2026:** `stale` was removed from the 2025 draft API. With
> kernel-managed `flock`, locks release automatically on process death — a
> stale timeout is meaningless (and the `proper-lockfile` heuristics it
> mimics are exactly what this feature exists to eliminate).

### Why Rust Matters

Direct access to OS-level locking primitives:
- **Linux/macOS:** `flock(fd, LOCK_EX | LOCK_NB)` — kernel-managed, no
  polling, no PID files, automatic release on process death.
- **Windows:** `LockFileEx(hFile, LOCKFILE_EXCLUSIVE_LOCK, ...)` — same
  guarantees.

The OS handles cleanup on process death. No stale lock files. No PID reuse
bugs. No polling overhead.

### Implementation

```rust
use fs2::FileExt;  // Cross-platform flock wrapper

#[napi]
pub async fn with_lock(
    path: String,
    exclusive: bool,
    timeout_ms: Option<u32>,
    callback: JsFunction,
) -> Result<()> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&path)
        .map_err(|e| Error::from_reason(e.to_string()))?;

    if exclusive {
        file.lock_exclusive()
    } else {
        file.lock_shared()
    }.map_err(|e| Error::from_reason(e.to_string()))?;

    // Call JS callback...
    // Lock released when `file` is dropped (even on panic)
    let _guard = scopeguard::guard(file, |f| {
        let _ = f.unlock();
    });

    // ... invoke callback ...
    Ok(())
}
```

### Refinements (added Sept 2026)

- **Sidecar option:** `sidecar: true` locks a `"<file>.lock"` file next to
  the target instead of the data file itself (the proper-lockfile
  convention). Use it when the target must remain readable/unlocked by
  other readers, or when the target may not exist yet.
- **NFS caveat (documented):** POSIX `flock` on NFS is client-side
  emulation; a crashed holder may leave the lock until the lease expires.
  Kernel guarantees hold on local filesystems.
- **Windows:** `LockFileEx` is range-based — we lock a 0..1 sentinel byte.
  Documented, and tested on the Windows CI legs.

### Phase Target: v0.3

---

## Feature Priority Matrix

| Feature | Pain Level | Implementation Complexity | Phase | Dependencies |
|---------|-----------|--------------------------|-------|-------------|
| Temp dirs | 🔴 High | 🟢 Low | v0.2 | `tempfile` crate |
| Snapshots/diff | 🔴 High | 🟡 Medium | v0.2 | Fused walk + hash |
| Sandbox | 🟠 Medium | 🟡 Medium | v0.3 | `pathe` + per-component checks; native `openat` on Unix |
| Parallel ops | 🟠 Medium | 🟡 Medium | v0.3 | Fused walk + rayon |
| File locking | 🟠 Medium | 🟢 Low | v0.3 | `fs2` crate |
| Transactions | 🟡 Low (niche) | 🔴 High | v0.4 | Locking + atomic writes |

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Temp dirs: tiered guarantee, documented | `O_TMPFILE` / `DELETE_ON_CLOSE` where available; mkstemp+Drop baseline; SIGKILL behavior documented per tier (2026 correction) |
| Snapshots use content hash | `mtime` is unreliable across git checkout, Docker, CI; ns-precision mtime kept as fast-path tie-breaker |
| Sandbox via `SandboxPath` checks plus native `openat(O_NOFOLLOW)` reads | The public Node path view preserves compatibility but documents its remaining TOCTOU window; Unix descriptor reads close it |
| Parallel ops cross boundary only for transform | User code is unavoidable; everything else stays native |
| Transactions are best-effort | Honest about limitations; not a database |
| Locking uses `flock()` | Kernel-managed; no polling, no PID files, no races; `stale` option removed (2026) |
| All features opt-in | Core package stays lean; features don't bloat v0.1 |
