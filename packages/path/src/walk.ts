/**
 * Traversal: the fused walk.
 *
 * This is the module the native engine exists for. The Step 0 spike ran
 * (task-1) and froze the transport — chunked paging over `AsyncTask` — and
 * task-3 wired the engine's `Walker` under the signatures that were frozen
 * here before the spike. The public shape did not move, which was the point
 * of freezing it first.
 *
 * ## Why the yield unit is a batch
 *
 * The competitors — `fdir`, `tinyglobby`, `Bun.Glob.scan()`, `node:fs.glob` —
 * return paths. An application then stats, hashes and filters each path, paying
 * a boundary crossing per file. The engine here does all four in one syscall
 * pass and hands back populated batches, so the boundary is crossed once per
 * 512 entries rather than once per entry.
 *
 * The spike kept batching load-bearing: `#[napi(async_iterator)]` measured
 * within 4.5% of paging at the default batch size while batch size itself
 * dominated, so the batch is the mechanism, not a prefetch window — see the
 * Step 0 record in `.knowledge/architecture/napi-boundary.md`.
 */

import { loadEngine } from "./binding.js";
import type { HasherName, PathEntry } from "./types.js";

/** Options for a walk. All are optional; the defaults are the ones the phase plan fixes. */
export interface WalkOptions {
  /**
   * Glob patterns, combined with AND, matched against root-relative paths.
   *
   * A vector, not a single pattern — and root-relative on purpose, so
   * `**\/*.ts` means the same thing regardless of where the checkout lives.
   */
  glob?: string[];

  /** A regular expression, matched against the full absolute path. */
  regex?: string;

  /** Directory names to prune before descending. Cheaper than filtering after. */
  exclude?: string[];

  /** Include entries whose name starts with a dot. Default `false`. */
  dot?: boolean;

  /** Honour `.gitignore` and `.ignore`. Default `false`. */
  gitignore?: boolean;

  /** Return absolute paths instead of root-relative ones. Default `false`. */
  absolute?: boolean;

  /** Compute a content hash for every entry. Hashing is the other half of the fused pass. */
  hash?: HasherName;

  /** Populate `size` and `modifiedNanos`. Default `true`. */
  withMetadata?: boolean;

  /**
   * Entries per batch yielded across the boundary.
   *
   * 512 is the phase plan's starting point, not a measured value — Q3 in the
   * context file is exactly the question of whether 256, 512 or 1024 is right.
   */
  batchSize?: number;

  /** Stop the walk when aborted. Wired to the engine's cancellation flag. */
  signal?: AbortSignal;
}

/**
 * A batch of walk results.
 *
 * Yielded as a whole rather than entry by entry, which is the entire point of
 * the fused walk: one boundary crossing per batch instead of one per file.
 */
export type WalkBatch = readonly PathEntry[];

/**
 * Walk a tree, yielding populated batches.
 *
 * The transport is the one the Step 0 spike froze (task-1, recorded in
 * `.knowledge/architecture/napi-boundary.md`): the native `Walker` is a
 * pager — `scan()` once on the libuv pool, then one boundary crossing per
 * `nextBatch()` — and this generator is the thin loop that drives it.
 *
 * Yields files *and* directories; {@link walkFiles} and {@link walkDirs}
 * narrow. An `AbortSignal` cancels natively: the engine's atomic flag stops
 * the worker threads at their next check, and the generator surfaces
 * `signal.reason` rather than yielding a partial batch as if it were a
 * result.
 *
 * The engine loads lazily on first consumption, so importing this module —
 * and calling `walk()` without iterating — stays safe without a built
 * addon; the first `next()` fails with the build hint instead.
 */
export async function* walk(root: string, options: WalkOptions = {}): AsyncGenerator<WalkBatch> {
  yield* drive(root, options, undefined);
}

/** Walk and yield only files. Delegates to {@link walk}. */
export async function* walkFiles(
  root: string,
  options: WalkOptions = {}
): AsyncGenerator<WalkBatch> {
  yield* drive(root, options, false);
}

/** Walk and yield only directories. Delegates to {@link walk}. */
export async function* walkDirs(
  root: string,
  options: WalkOptions = {}
): AsyncGenerator<WalkBatch> {
  yield* drive(root, options, true);
}

/**
 * The one loop behind all three entry points.
 *
 * `isDir` narrows a batch after it crosses the boundary — filtering here
 * costs a `filter()` over 512 entries, whereas a second native option would
 * be a third walk mode for the engine to test. `undefined` means unfiltered.
 */
async function* drive(
  root: string,
  options: WalkOptions,
  isDir: boolean | undefined
): AsyncGenerator<WalkBatch> {
  const { signal } = options;
  signal?.throwIfAborted();

  const engine = loadEngine();
  const walker = new engine.Walker(root, {
    ...(options.glob !== undefined && { glob: options.glob }),
    ...(options.regex !== undefined && { regex: options.regex }),
    ...(options.exclude !== undefined && { exclude: options.exclude }),
    ...(options.dot !== undefined && { dot: options.dot }),
    ...(options.gitignore !== undefined && { gitignore: options.gitignore }),
    ...(options.absolute !== undefined && { absolute: options.absolute }),
    ...(options.withMetadata !== undefined && { withMetadata: options.withMetadata }),
    ...(options.hash !== undefined && { hash: options.hash }),
    ...(options.batchSize !== undefined && { batchSize: options.batchSize }),
    // Directories must cross the boundary when the caller wants directories
    // (or everything); only walkFiles can let the engine skip them.
    filesOnly: isDir === false
  });

  // The abort listener calls straight into the native cancel: the flag is
  // atomic, the worker threads observe it mid-walk, and `scan()` resolves
  // early with a partial count instead of running to completion first.
  const onAbort = () => {
    walker.cancel();
  };
  signal?.addEventListener("abort", onAbort, { once: true });

  try {
    await walker.scan();
    for (;;) {
      signal?.throwIfAborted();
      const batch = await walker.nextBatch();
      if (batch.length === 0) {
        break;
      }
      const narrowed = isDir === undefined ? batch : batch.filter((e) => e.isDir === isDir);
      if (narrowed.length > 0) {
        yield narrowed;
      }
    }
  } finally {
    // Early `break` by the consumer lands here: stop the traversal rather
    // than letting worker threads walk a tree nobody is draining.
    walker.cancel();
    signal?.removeEventListener("abort", onAbort);
  }
}
