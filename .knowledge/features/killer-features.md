---
title: "Killer Features: Temp Dirs, Snapshots, Sandbox, Transactions, Locking, Parallel Ops"
domain: features
status: proposed
created: 2025-07-11
updated: 2025-07-11
source: conversation
depends_on:
  - architecture/core-layers
  - architecture/napi-boundary
  - features/walk-traversal
  - features/serializers
tags: [temp, snapshot, diff, sandbox, transaction, lock, parallel, security]
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

### Why Rust Matters

Rust's `tempfile` crate uses OS-level guarantees:
- **Linux:** `O_TMPFILE` flag — file exists only as an inode, never appears
  in the directory tree. Or `mkstemp()` + immediate `unlink()`.
- **Windows:** `FILE_FLAG_DELETE_ON_CLOSE` — the OS deletes the file when
  the last handle is closed, even if the process crashes.
- **macOS:** `mkstemp()` + `unlink()` with `atexit()` handler.

The cleanup happens at the **file descriptor level**, not via JS `finally`
blocks that can be skipped. Even if the Node/Bun process is killed with
`SIGKILL`, the OS reclaims the temp files.

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
providing a best-effort cleanup even if the JS callback throws. The OS-level
flags provide the hard guarantee.

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

**Expected speedup: 10–20x on 100k+ file trees.**

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
    stale: 30000,        // Auto-release after 30s (safety net)
  },
  async () => {
    // Critical section
  }
);
```

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
| Temp dirs via OS flags | `O_TMPFILE` / `DELETE_ON_CLOSE` survive crashes; JS `finally` doesn't |
| Snapshots use content hash | `mtime` is unreliable across git checkout, Docker, CI |
| Sandbox is a Path subclass | Type-level enforcement; zero-cost at runtime |
| Parallel ops cross boundary only for transform | User code is unavoidable; everything else stays native |
| Transactions are best-effort | Honest about limitations; not a database |
| Locking uses `flock()` | Kernel-managed; no polling, no PID files, no races |
| All features opt-in | Core package stays lean; features don't bloat v0.1 |
