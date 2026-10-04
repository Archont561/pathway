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
 * A serializer is passed in, not selected by a string on `Path`. That is
 * decision D4: a native codec and a JavaScript implementation are interchangeable,
 * while the type parameter keeps the value returned by `read()` precise.
 */
export interface Serializer<T> {
  /** The name this serializer is registered under, for example `"json"`. */
  readonly name: string;

  /** Whether this serializer is backed by the native engine. */
  readonly native: boolean;

  /** Extensions used when a `FileSystem` resolves a serializer automatically. */
  readonly extensions?: readonly string[];

  /** Parse raw bytes into a value. */
  parse(bytes: Uint8Array): T;

  /** Serialize a value into raw bytes. */
  stringify(value: T): Uint8Array;
}

/** A value that can validate data after a serializer has parsed it. */
export interface Validator<T> {
  readonly name: string;
  validate(data: unknown): T;
}

/** Options for reading text or serialized values. */
export interface ReadOptions {
  /** Text encoding used by `readText()`. Serialized reads receive bytes directly. */
  readonly encoding?: BufferEncoding;
  /** Optional application-level validation after deserialization. */
  readonly validate?: Validator<unknown>;
}

/** Options for text, byte, and serialized writes. */
export interface WriteOptions {
  /** Write to a same-directory temporary file and rename it into place. */
  readonly atomic?: boolean;
  /** Flush the temporary file and best-effort flush its directory before returning. */
  readonly fsync?: boolean;
  /** File mode used when a new file is created. */
  readonly mode?: number;
  /** Text encoding used by `writeText()`. */
  readonly encoding?: BufferEncoding;
}

/** Options used when constructing an isolated filesystem view. */
export interface FileSystemOptions {
  /** Serializers registered for this filesystem instance only. */
  readonly serializers?: readonly Serializer<unknown>[];
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
 * and filter — rather than being a path the caller then has to populate.
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
