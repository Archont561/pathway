/**
 * Traversal: the fused walk.
 *
 * This is the module the native engine exists for, and it is the only part of
 * the public surface that is not implemented yet. The type signatures and the
 * batching contract are here because they are what the Step 0 spike has to
 * decide against; the engine call is not, because the engine has no walk yet.
 *
 * ## Why the yield unit is a batch
 *
 * The competitors — `fdir`, `tinyglobby`, `Bun.Glob.scan()`, `node:fs.glob` —
 * return paths. An application then stats, hashes and filters each path, paying
 * a boundary crossing per file. The engine here does all four in one syscall
 * pass and hands back populated batches, so the boundary is crossed once per
 * 512 entries rather than once per entry.
 *
 * Whether that batching stays load-bearing depends on the Step 0 NAPI-RS
 * iterator spike: if `#[napi(async_iterator)]` is adopted, the batch becomes a
 * prefetch window and the JavaScript side consumes entries one at a time; if it
 * is not, the batch is the mechanism. Either way the *public* API below is the
 * same, which is why the API gets frozen before the spike rather than after.
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

  /** Honour `.gitignore` and `.ignore`. Default `true`. */
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
 * Not implemented: `crates/core` has the module tree and the error type, but no
 * scanner yet. This throws instead of returning an empty generator, because an
 * empty walk is indistinguishable from a real result — a caller that filters
 * everything out and a caller with no engine look identical from the outside.
 *
 * The signature is the one the real implementation will have, which is the
 * point: the public shape is frozen *before* the Step 0 NAPI-RS iterator spike,
 * so the spike can change how entries arrive without changing how they are
 * consumed.
 *
 * @throws immediately, with the crate and task to implement
 */
export function walk(_root: string, _options: WalkOptions = {}): AsyncGenerator<WalkBatch> {
  void loadEngine();
  throw new Error(
    "walk() is not implemented yet. The scanner lives in crates/core/src/walk/scanner.rs " +
      "(see .knowledge/implementation/phase-plan.md, Phase 1 Step 1.2); this stub exists so the " +
      "public signature is frozen before the Step 0 NAPI-RS iterator spike."
  );
}

/** Walk and yield only files. */
export function walkFiles(root: string, options: WalkOptions = {}): AsyncGenerator<WalkBatch> {
  return walk(root, options);
}

/** Walk and yield only directories. */
export function walkDirs(root: string, options: WalkOptions = {}): AsyncGenerator<WalkBatch> {
  return walk(root, { ...options, absolute: options.absolute ?? false });
}
