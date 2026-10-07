/**
 * Temporary directories with a cleanup guarantee, tiered and documented.
 *
 * Every JavaScript project has this somewhere:
 *
 * ```ts
 * const tmp = await fs.mkdtemp("/tmp/myapp-");
 * try {
 *   // ... do work ...
 * } finally {
 *   await fs.rm(tmp, { recursive: true, force: true });
 * }
 * ```
 *
 * `finally` is skipped by `process.exit()`, and nothing runs at all after a
 * `SIGKILL`. What this module adds is not a shorter spelling of that block; it
 * is a life cycle the engine drives instead of the callback:
 *
 * | Tier | Mechanism | Survives |
 * |------|-----------|----------|
 * | 1 | the native guard, plus a `process.on("exit")` flush installed on first use | a return, a throw, `process.exit()`, and a `SIGINT`/`SIGTERM` for which the host installed a handler that exits |
 * | 2 | `O_TMPFILE` (Linux) / `FILE_FLAG_DELETE_ON_CLOSE` (Windows) | `SIGKILL` — **for temp files only** |
 * | 3 | documented only | a `SIGKILL`, a `SIGINT`/`SIGTERM` under the default disposition, or a power loss: the tree stays on disk until something reaps it |
 *
 * Two things the 2025 draft promised that are not true, both pinned by tests
 * in `crates/core/src/fs/temp.rs` rather than argued here:
 *
 * - **A temp directory is never tier 2.** `O_TMPFILE` provides anonymity by
 *   creating no directory entry; a directory that a caller can hand to a child
 *   process is a path, and a path is an entry. The kernel ignores the
 *   `O_DIRECTORY` bit on `O_TMPFILE` and returns a regular file.
 * - **Cleanup on `SIGINT` needs the host's cooperation.** Node and Bun run an
 *   `exit` hook for `process.exit()` and for a signal whose *handler* exits
 *   cleanly; under the default disposition a `SIGINT` terminates the process
 *   without running a line of JavaScript. Installing signal handlers from here
 *   would change the host's semantics behind its back, so this module does not.
 *
 * @packageDocumentation
 */

import { loadEngine, type NativeTempDir, type NativeTempOptions } from "./binding.js";

/** How a temporary directory is named and where it is created. */
export interface TempOptions {
  /** Prefix for the directory's name. */
  readonly prefix?: string;
  /** Suffix for the directory's name. */
  readonly suffix?: string;
  /** The directory to create it inside. Defaults to the system temp directory. */
  readonly dir?: string;
}

/** Whether the exit flush has been registered, so it is installed once. */
let exitFlushInstalled = false;

/**
 * Registers the process exit flush.
 *
 * Called on first use rather than at import: a process that never asks for a
 * temp directory never gets a listener added to its `exit` event. The listener
 * swallows its own failures — it runs while a process is already on its way
 * out, and a cleanup that cannot complete must not change the exit code.
 */
function installExitFlush(): void {
  if (exitFlushInstalled) {
    return;
  }
  exitFlushInstalled = true;
  process.on("exit", () => {
    try {
      loadEngine().flushTempDirs();
    } catch {
      // The engine is gone or the tree is unremovable; the OS temp reaper and
      // the documented tier-3 behaviour are what remain.
    }
  });
}

/**
 * A temporary directory, held by path.
 *
 * The handle is deliberately string-valued, like `fs.mkdtemp`: the ergonomic
 * `Path` is what {@link Path.temp} hands the callback. Cleanup follows the
 * tiers in this module's header — `remove()` and garbage collection cover tier
 * 1, the exit flush covers `process.exit()`, and nothing covers a `SIGKILL`.
 */
export class TempDir {
  /** The directory's absolute path. */
  readonly path: string;

  /** The native guard, which is what actually deletes the tree. */
  private readonly native: NativeTempDir;

  /** @internal Constructed by {@link tempDir}, never by a caller. */
  constructor(native: NativeTempDir) {
    this.native = native;
    this.path = native.path();
  }

  /**
   * Removes the tree now.
   *
   * The explicit form of what garbage collection does, for a caller that wants
   * the failure instead of a best-effort attempt. Called twice — or after
   * {@link keep} — it rejects rather than pretending, because a second removal
   * is a lifecycle bug and not a cleanup.
   *
   * A removal that fails leaves the directory queued for the exit flush, so
   * `catch` here still means "the OS will try again at exit".
   */
  async remove(): Promise<void> {
    this.native.remove();
  }

  /**
   * Disarms cleanup and hands the path over.
   *
   * The directory is now the caller's to remove: neither garbage collection
   * nor the exit flush will touch it. Returns the same path as
   * {@link TempDir.path}.
   */
  async keep(): Promise<string> {
    return this.native.keep();
  }

  /** This directory's path, as a plain string. */
  toString(): string {
    return this.path;
  }
}

/** Maps the public options onto the engine's, omitting what the caller omitted. */
function nativeOptions(options: TempOptions): NativeTempOptions {
  return {
    ...(options.prefix !== undefined && { prefix: options.prefix }),
    ...(options.suffix !== undefined && { suffix: options.suffix }),
    ...(options.dir !== undefined && { parent: options.dir })
  };
}

/**
 * Creates a temporary directory and hands back its handle.
 *
 * The handle form is the primitive; `Path.temp(options, callback)` is the
 * scoped sugar over it. Requires the native engine, because the cleanup
 * guarantee is the engine's guard — a JavaScript implementation with a
 * `finally` block would be exactly the fragile pattern this module replaces.
 */
export async function tempDir(options: TempOptions = {}): Promise<TempDir> {
  const engine = loadEngine();
  installExitFlush();
  return new TempDir(new engine.TempDir(nativeOptions(options)));
}

/**
 * Runs `body` inside a temporary directory and removes it afterwards.
 *
 * @internal The seam behind `Path.temp` and `FileSystem#temp`: it deals in
 * strings so both of them can wrap the path in the `Path` flavour — and the
 * serializer registry — that the caller is entitled to see.
 */
export async function withTempDirectory<T>(
  options: TempOptions | undefined,
  body: (path: string) => Promise<T>
): Promise<T> {
  const dir = await tempDir(options);
  try {
    return await body(dir.path);
  } finally {
    // A removal that fails is not allowed to replace the callback's own
    // failure, and the engine keeps a failed removal queued for the exit
    // flush — so swallowing here does not mean leaking.
    await dir.remove().catch(() => undefined);
  }
}
