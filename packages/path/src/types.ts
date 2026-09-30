/**
 * Core type definitions for the public surface.
 *
 * Kept in one file, and separate from the implementations, because these types
 * are the contract: they are what a consumer sees in the generated `.d.ts`, and
 * the reason the package can be reviewed without reading the engine.
 */

/**
 * A strategy object that turns bytes into a value and back.
 *
 * A serializer is passed *in*, not selected by a string on `Path`. That is
 * decision D4, and the two reasons for it are the reason this is an interface
 * and not an enum:
 *
 * 1. `read<T>()` and `write<T>()` stay generic in `T`, so the return type
 *    follows the serializer the caller passed instead of a union the caller has
 *    to narrow.
 * 2. A native Serde codec and a plain JavaScript object are interchangeable. The
 *    built-in `json` serializer is a JavaScript one (there is no reason to
 *    cross the boundary for `JSON.parse`); `toml` and `yaml` are native ones,
 *    because neither runtime has a parser for them.
 *
 * The registry these live in is per-`FileSystem` instance, never global — see
 * `FileSystem.create()`.
 */
export interface Serializer<T> {
  /** The name this serializer is registered under, e.g. `"json"`. */
  readonly name: string;

  /**
   * Whether this serializer runs in the native engine.
   *
   * Exposed rather than internal so a caller can pick a JavaScript serializer
   * deliberately when it matters — for a hot loop over many small files, a
   * native round trip costs more than `JSON.parse` does.
   */
  readonly native: boolean;

  /** Parse raw bytes into a value. */
  parse(bytes: Uint8Array): T;

  /** Serialise a value into raw bytes. */
  stringify(value: T): Uint8Array;
}

/** Which hash algorithm a walk should compute while it traverses. */
export type HasherName = "blake3" | "xxhash" | "sha256";

/** Why one entry in a walk could not be completed. */
export type EntryErrorKind =
  /** The entry could not be `stat`ed. */
  | "stat"
  /** The entry could not be read for hashing. */
  | "read"
  /** The entry's hash algorithm was not recognised. */
  | "hasher"
  /** The entry escaped the walk root. */
  | "escaped"
  /** The entry's contents did not match the codec. */
  | "codec";

/**
 * One result of a fused walk.
 *
 * Populated by the engine in a single syscall pass — traversal, `stat`, hash
 * and filter — rather than being a path the caller then has to go and populate.
 * That is the whole claim, and it is why a walk yields this and not a string.
 */
export interface PathEntry {
  /** The path, relative to the walk root unless `absolute` was requested. */
  readonly value: string;

  /** Whether this entry is a directory. Directories are yielded, not walked past silently. */
  readonly isDir: boolean;

  /** Size in bytes, when metadata was requested and the `stat` succeeded. */
  readonly size?: number;

  /**
   * Modification time in nanoseconds since the Unix epoch.
   *
   * Nanoseconds, not milliseconds: two files written in the same millisecond
   * are indistinguishable at millisecond precision, which silently breaks
   * incremental builds.
   */
  readonly modifiedNanos?: bigint;

  /** Lowercase hex digest, when a hash algorithm was requested. */
  readonly hash?: string;

  /**
   * Why this entry could not be completed, if it could not.
   *
   * The entry is still yielded: its path and stat are known, and dropping it
   * would hide a file that exists. A caller that needs a clean run checks this
   * field; a caller that can tolerate a bad file does not have to.
   */
  readonly error?: {
    readonly kind: EntryErrorKind;
    readonly message: string;
  };
}
