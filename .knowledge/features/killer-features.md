---
type: Feature Spec
title: "Killer Features: Temp Dirs, Snapshots, Sandbox, Transactions, Locking, Parallel Ops"
description: "Differentiating filesystem features: temp dirs, snapshots and diff, sandboxing, transactions, locking, and parallel operations."
tags: [temp, snapshot, diff, sandbox, transaction, lock, parallel, security]
status: draft
generated:
  by: pathway_kb/1.0
  at: 2026-10-03T00:00:00Z
verified:
  - by: human:archont561
    at: 2025-07-11T00:00:00Z
  - by: process:gap-analysis-2026-09
    at: 2026-09-16T00:00:00Z
  - by: process:benchmark-task-4
    at: 2026-10-03T00:00:00Z
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
// dir is gone. Even on throw. Even on SIGINT.
```

With options:

```ts
await Path.temp({ prefix: "build-", dir: "/fast-ssd" }, async (dir) => {
  // Temp dir created on /fast-ssd with prefix "build-"
});
```

### Why Rust Matters (corrected Sept 2026)

The 2025 draft overstated the guarantee. The Rust `tempfile` crate uses
`mkstemp`/`mkdtemp` + **destructor-based** cleanup. That covers throws,
`process.exit()`, and GC — but **not** `SIGKILL`/hard crash: the
destructor never runs, and the OS reclaims nothing until a tmp reaper does.

The implementation must therefore use OS-level flags where available, and
the docs must state the **tiered** guarantee honestly:

| Tier | Mechanism | Survives |
|------|-----------|----------|
| 1 | `tempfile` Drop + atexit | throw, `process.exit()`, normal GC |
| 2a | **Linux:** `O_TMPFILE` (anonymous inode — never appears in the directory tree; reclaimed on close even after SIGKILL). Not supported on all filesystems (e.g. NFS) — fall back to mkstemp + immediate unlink | SIGKILL (local FS) |
| 2b | **Windows:** `FILE_FLAG_DELETE_ON_CLOSE` — OS deletes on last handle close | SIGKILL |
| 3 | **macOS/other:** mkstemp + unlink, best-effort | leaks until OS tmp reaper on SIGKILL — documented |

The cleanup happens at the **file descriptor / OS level** where tier 2 is
available, not via JS `finally` blocks that can be skipped.

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
providing best-effort cleanup even if the JS callback throws. **In
addition**, the implementation opens temp files with `O_TMPFILE` on Linux
local filesystems and `FILE_FLAG_DELETE_ON_CLOSE` on Windows (via the
`open`/`tempfile` crate options) to obtain the hard tier-2 guarantee; where
those flags are unavailable the tier-3 documented behavior applies.

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
```

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

**Measured speedup (2026-10-03, 100k files): 1.85x** on the full pipeline
against the strongest baseline (`fdir` + a 32-wide stat/hash pool), with peak
heap **14.3 MiB vs 39.3 MiB** and GC **2.8 ms vs 20.6 ms** per sample. The
originally projected 10–20x was off by ~6x. See
[verified-data.md](/competitive/verified-data.md).

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

```ts
class SandboxPath extends Path {
  readonly root: string;

  join(...segments: string[]): SandboxPath {
    const resolved = pathe.resolve(this.root, this.value, ...segments);
    if (!resolved.startsWith(this.root + "/") && resolved !== this.root) {
      throw new ContainmentError(
        `Path "${resolved}" escapes sandbox root "${this.root}"`
      );
    }
    return new SandboxPath(resolved, this.root);
  }

  resolve(specifier: string): SandboxPath {
    return this.join(specifier);  // Same containment check
  }
}

class ContainmentError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ContainmentError";
  }
}
```

### Symlink Handling (Native)

For full security, the containment check must resolve symlinks natively:

```rust
#[napi]
pub fn resolve_realpath(path: String, root: String) -> Result<String> {
    let real = std::fs::canonicalize(&path)
        .map_err(|e| Error::from_reason(e.to_string()))?;
    let real_root = std::fs::canonicalize(&root)
        .map_err(|e| Error::from_reason(e.to_string()))?;

    if !real.starts_with(&real_root) {
        return Err(Error::new(
            Status::GenericFailure,
            format!("Path escapes sandbox: {} not in {}", real.display(), real_root.display())
        ));
    }

    Ok(real.to_string_lossy().into())
}
```

This handles symlink escapes that the JS `path.resolve()` check misses.

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

1. **Intermediate symlinks:** `public/link → /etc` escapes even when the
   *final* canonical path is inside the root. Fix: per-component realpath,
   or (preferred) fd-based `openat(2, O_NOFOLLOW)` opens in the Rust engine
   so resolution and open are a single step — which also closes the TOCTOU
   window (the file can't be swapped for a symlink between check and open).
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
Rust Engine
   │
   ├── Walk + filter (ignore crate, parallel)
   │
   ├── For each file (rayon parallel):
   │     ├── Read content (mmap or buffered)
   │     ├── If transform: yield to JS callback (N-API ThreadSafeFunction)
   │     ├── Write to destination (parallel I/O)
   │     └── Track progress
   │
   └── Return summary: { copied, skipped, errors }
```

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
| Sandbox | 🟠 Medium | 🟢 Low | v0.3 | `pathe` + `canonicalize` |
| Parallel ops | 🟠 Medium | 🟡 Medium | v0.3 | Fused walk + rayon |
| File locking | 🟠 Medium | 🟢 Low | v0.3 | `fs2` crate |
| Transactions | 🟡 Low (niche) | 🔴 High | v0.4 | Locking + atomic writes |

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Temp dirs: tiered guarantee, documented | `O_TMPFILE` / `DELETE_ON_CLOSE` where available; mkstemp+Drop baseline; SIGKILL behavior documented per tier (2026 correction) |
| Snapshots use content hash | `mtime` is unreliable across git checkout, Docker, CI; ns-precision mtime kept as fast-path tie-breaker |
| Sandbox via `openat(O_NOFOLLOW)` + canonicalize | Final-path check alone misses intermediate symlink escapes and TOCTOU (2026 audit) |
| Parallel ops cross boundary only for transform | User code is unavoidable; everything else stays native |
| Transactions are best-effort | Honest about limitations; not a database |
| Locking uses `flock()` | Kernel-managed; no polling, no PID files, no races; `stale` option removed (2026) |
| All features opt-in | Core package stays lean; features don't bloat v0.1 |
