/**
 * Directory snapshots: capture, persist, diff.
 *
 * Build systems, test runners and watchers all need to answer *what changed?*,
 * and the usual answer is two maps of `mtimeMs` built from a glob — which
 * misses deletions, cannot see a rewrite inside the same millisecond, and pays
 * a `stat` per file across the boundary.
 *
 * ```ts
 * const before = await project.snapshot({ glob: ["**\/*.ts"], hash: "blake3" });
 * // ... run the compiler ...
 * const diff = before.diff(await project.snapshot({ glob: ["**\/*.ts"], hash: "blake3" }));
 * for (const file of diff.modified) await rebuild(file);
 * ```
 *
 * ## This module is a view, not an implementation
 *
 * The fold, the comparison rule and the persisted format all live in
 * `crates/core/src/snapshot`. A snapshot crosses the boundary **as the JSON
 * document** `save()` writes, and a diff comes back as the four arrays the
 * core computed. Nothing here sorts, compares or re-times anything: a snapshot
 * format with two implementations is a format whose two implementations
 * eventually disagree, and a build cache keyed on it would then be wrong
 * rather than merely slow.
 *
 * That also keeps the boundary coarse (D2): one crossing per snapshot and one
 * per diff, never one per file.
 *
 * ## Nanoseconds
 *
 * `modifiedNanos` is a `bigint` here and a decimal *string* in the document.
 * A JSON number is an IEEE-754 double in every mainstream parser and a 2026
 * nanosecond timestamp needs 61 bits, so a number would quietly round — and a
 * rounded mtime is exactly the stale-incremental-build bug the nanoseconds
 * exist to prevent.
 *
 * @packageDocumentation
 */

import { mkdir } from "node:fs/promises";
import { loadEngine, type NativeWalkerOptions } from "./binding.js";
import { Path } from "./path.js";
import type { SerializerRegistry } from "./serializers/registry.js";
import type { HasherName } from "./types.js";

/** The `format` tag every persisted snapshot carries. */
export const SNAPSHOT_FORMAT = "pathway-snapshot-v1";

/** What a snapshot may be filtered by — the walk options a capture understands. */
export interface SnapshotOptions {
  /** Glob patterns, combined with AND, matched against root-relative paths. */
  glob?: string[];

  /** A regular expression, matched against the full absolute path. */
  regex?: string;

  /** Directory names to prune before descending. */
  exclude?: string[];

  /** Include entries whose name starts with a dot. Default `false`. */
  dot?: boolean;

  /** Honour `.gitignore` and `.ignore`. Default `false`. */
  gitignore?: boolean;

  /**
   * Hash every file's contents, so the diff compares content rather than
   * `size` plus mtime. The difference is visible in both directions: a file
   * rewritten with identical bytes is `unchanged` only with a hash, and a file
   * whose bytes changed without changing size or mtime is `modified` only with
   * a hash.
   */
  hash?: HasherName;
}

/** What a snapshot records about one file. */
export interface SnapshotEntry {
  /** Size in bytes at capture time. */
  readonly size: number;
  /** Modification time in nanoseconds since the Unix epoch. */
  readonly modifiedNanos: bigint;
  /** Lowercase hex content digest, when the capture asked for one. */
  readonly hash?: string;
}

/** What changed between two snapshots. Every path is in exactly one bucket. */
export interface SnapshotDiff {
  /** Files present only in the later snapshot. */
  readonly added: readonly Path[];
  /** Files present only in the earlier snapshot. */
  readonly removed: readonly Path[];
  /** Files in both whose content differs. */
  readonly modified: readonly Path[];
  /** Files in both whose content is the same. */
  readonly unchanged: readonly Path[];
  /** Whether anything was added, removed or modified. */
  readonly hasChanges: boolean;
}

/**
 * A persisted document that is not a snapshot of a format this version reads.
 *
 * Its own error type, like `WalkError` and `ContainmentError`: a cache that
 * finds a stale or foreign file wants to catch exactly this and re-capture,
 * not to pattern-match a message.
 */
export class SnapshotFormatError extends Error {
  override readonly name = "SnapshotFormatError";

  constructor(message: string) {
    super(message);
  }
}

/** The wire shape of one entry inside the persisted document. */
interface EntryDocument {
  readonly path: string;
  readonly size: number;
  readonly modifiedNanos: string;
  readonly hash?: string;
}

/** The persisted document, as the core writes it. */
interface SnapshotDocument {
  readonly format: string;
  readonly root: string;
  readonly takenAtNanos: string;
  readonly entries: readonly EntryDocument[];
}

interface DiffDocument {
  readonly added: readonly string[];
  readonly removed: readonly string[];
  readonly modified: readonly string[];
  readonly unchanged: readonly string[];
}

function walkerOptions(options: SnapshotOptions): NativeWalkerOptions {
  const native: {
    glob?: readonly string[];
    regex?: string;
    exclude?: readonly string[];
    dot?: boolean;
    gitignore?: boolean;
    hash?: string;
  } = {};
  if (options.glob !== undefined) native.glob = options.glob;
  if (options.regex !== undefined) native.regex = options.regex;
  if (options.exclude !== undefined) native.exclude = options.exclude;
  if (options.dot !== undefined) native.dot = options.dot;
  if (options.gitignore !== undefined) native.gitignore = options.gitignore;
  if (options.hash !== undefined) native.hash = options.hash;
  return native;
}

/** Turns a native rejection into the typed error when it is a format refusal. */
function asSnapshotError(error: unknown): unknown {
  const message = error instanceof Error ? error.message : String(error);
  return message.includes("snapshot codec failed") ? new SnapshotFormatError(message) : error;
}

/**
 * A directory tree as one fused walk saw it.
 *
 * Immutable, and cheap to hold: the document it was built from is kept as the
 * canonical form, so `save()` writes bytes rather than re-serializing a
 * JavaScript object the core would have to re-validate.
 */
export class Snapshot {
  /** The absolute root the snapshot was taken of. */
  readonly root: string;

  /** When the capture finished, in nanoseconds since the Unix epoch. */
  readonly takenAtNanos: bigint;

  /** Every recorded file, keyed by its root-relative path, in sorted order. */
  readonly entries: ReadonlyMap<string, SnapshotEntry>;

  /** @internal The canonical document; the transport and the persisted form. */
  private readonly document: string;

  /** @internal The registry the Paths in a diff are bound to. */
  private readonly serializers: SerializerRegistry | undefined;

  private constructor(document: string, serializers?: SerializerRegistry) {
    const parsed = JSON.parse(document) as SnapshotDocument;
    this.document = document;
    this.root = parsed.root;
    this.takenAtNanos = BigInt(parsed.takenAtNanos);
    this.entries = new Map(
      parsed.entries.map((entry) => [
        entry.path,
        entry.hash === undefined
          ? { size: entry.size, modifiedNanos: BigInt(entry.modifiedNanos) }
          : { size: entry.size, modifiedNanos: BigInt(entry.modifiedNanos), hash: entry.hash }
      ])
    );
    this.serializers = serializers;
  }

  /**
   * Captures a snapshot of `root`.
   *
   * The walk runs on the libuv pool, so the JavaScript thread is free while
   * the tree is traversed, statted and (optionally) hashed in one pass.
   */
  static async capture(
    root: string,
    options: SnapshotOptions = {},
    serializers?: SerializerRegistry
  ): Promise<Snapshot> {
    const document = await loadEngine().snapshotCaptureNative(root, walkerOptions(options));
    return new Snapshot(document, serializers);
  }

  /**
   * Parses a persisted document, which the core validates.
   *
   * @throws {SnapshotFormatError} if the payload is not a snapshot document
   * this version reads.
   */
  static parse(document: string, serializers?: SerializerRegistry): Snapshot {
    try {
      return new Snapshot(loadEngine().snapshotValidateNative(document), serializers);
    } catch (error) {
      throw asSnapshotError(error);
    }
  }

  /**
   * Reads a snapshot written by {@link Snapshot.save}.
   *
   * @throws {SnapshotFormatError} if the file is not a snapshot document this
   * version reads.
   */
  static async load(path: Path | string, serializers?: SerializerRegistry): Promise<Snapshot> {
    const file = typeof path === "string" ? new Path(path) : path;
    return Snapshot.parse(await file.readText(), serializers);
  }

  /** The persisted JSON document: deterministic for a given tree. */
  toJSON(): string {
    return this.document;
  }

  /**
   * Writes the document to `path`, atomically and creating parents.
   *
   * Atomic because a snapshot is a cache key: a half-written document that
   * still parses would diff as a tree that never existed.
   */
  async save(path: Path | string): Promise<void> {
    const file = typeof path === "string" ? new Path(path) : path;
    await mkdir(file.parent.value, { recursive: true });
    await file.writeText(this.document, { atomic: true });
  }

  /** Compares this snapshot (the *before*) with `other` (the *after*). */
  diff(other: Snapshot): SnapshotDiff {
    const computed = JSON.parse(
      loadEngine().snapshotDiffNative(this.document, other.document)
    ) as DiffDocument;

    const resolve = (relative: readonly string[], root: string): Path[] =>
      relative.map((value) => new Path(`${root}/${value}`, this.serializers));

    const added = resolve(computed.added, other.root);
    const removed = resolve(computed.removed, this.root);
    const modified = resolve(computed.modified, other.root);
    return {
      added,
      removed,
      modified,
      unchanged: resolve(computed.unchanged, other.root),
      hasChanges: added.length > 0 || removed.length > 0 || modified.length > 0
    };
  }
}
