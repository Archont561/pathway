/**
 * `Path` — the ergonomic surface.
 *
 * String manipulation stays in TypeScript. Single-file operations use the
 * platform's promises API; bounded bulk copy, move, and transform operations
 * live in `bulk.ts`. Large-scale traversal, hashing, and filtering continue
 * to use the native engine through `walk.ts`.
 */

import { randomUUID } from "node:crypto";
import * as fs from "node:fs/promises";
import * as pathe from "pathe";
import { copyTree, movePath, transformTree } from "./bulk.js";
import { hashFile, hashTree } from "./hash.js";
import { defaultSerializerRegistry, type SerializerRegistry } from "./serializers/registry.js";
import type {
  CopyOptions,
  CopyResult,
  MoveOptions,
  MoveResult,
  ReadOptions,
  Serializer,
  TransformOptions,
  TransformResult,
  WriteOptions
} from "./types.js";

function isSerializer(value: unknown): value is Serializer<unknown> {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const candidate = value as { parse?: unknown; stringify?: unknown };
  return typeof candidate.parse === "function" && typeof candidate.stringify === "function";
}

function writeOptions(options: WriteOptions | undefined): { mode?: number } {
  return options?.mode === undefined ? {} : { mode: options.mode };
}

/**
 * A filesystem path with object methods.
 *
 * Path values are immutable: operations return new Paths and preserve the
 * filesystem view that supplied their serializer registry.
 */
export class Path {
  /** The normalized, absolute path this instance refers to. */
  readonly value: string;

  /** @internal The registry is supplied by FileSystem and is not global state. */
  constructor(
    value: string,
    protected readonly serializers: SerializerRegistry = defaultSerializerRegistry
  ) {
    this.value = pathe.normalize(
      pathe.isAbsolute(value) ? value : pathe.join(process.cwd(), value)
    );
  }

  /** Hook for confined path views to revalidate before filesystem access. */
  protected assertSafe(): void {}

  /** The current working directory as a Path. */
  static cwd(): Path {
    return new Path(process.cwd());
  }

  /** The directory containing this path. */
  get parent(): Path {
    return new Path(pathe.dirname(this.value), this.serializers);
  }

  /** The final component of this path. */
  get name(): string {
    return pathe.basename(this.value);
  }

  /** The final component without its extension. */
  get stem(): string {
    return pathe.basename(this.value, pathe.extname(this.value));
  }

  /** The extension, lowercase and including the dot, or `""` when absent. */
  get ext(): string {
    return pathe.extname(this.value).toLowerCase();
  }

  /** Join path segments onto this path. */
  join(...segments: string[]): Path {
    return new Path(pathe.join(this.value, ...segments), this.serializers);
  }

  /** Copy this file or tree to the exact destination using bounded concurrency. */
  async copyTo(destination: Path, options?: CopyOptions): Promise<CopyResult> {
    this.assertSafe();
    destination.assertSafe();
    return copyTree(this.value, destination.value, options);
  }

  /** Move this file or tree to the exact destination. */
  async moveTo(destination: Path, options?: MoveOptions): Promise<MoveResult> {
    this.assertSafe();
    destination.assertSafe();
    return movePath(this.value, destination.value, options);
  }

  /** Transform selected text files into a destination tree in bounded parallelism. */
  async transform(destination: Path, options: TransformOptions): Promise<TransformResult> {
    this.assertSafe();
    destination.assertSafe();
    return transformTree(this.value, destination.value, options);
  }

  /** Hash this file with a built-in or custom streaming hasher. */
  async hash(options?: import("./types.js").HashOptions): Promise<string> {
    this.assertSafe();
    return hashFile(this.value, options);
  }

  /** Hash this directory deterministically from its fused file walk. */
  async hashTree(options?: import("./types.js").HashOptions): Promise<string> {
    this.assertSafe();
    return hashTree(this.value, options);
  }

  /** Read the file as text. */
  async readText(encoding: BufferEncoding = "utf8"): Promise<string> {
    this.assertSafe();
    return fs.readFile(this.value, { encoding });
  }

  /** Read all bytes, or a bounded range starting at `offset`. */
  async readBytes(offset = 0, length?: number): Promise<Buffer> {
    this.assertSafe();
    if (length === undefined) {
      return fs.readFile(this.value);
    }

    const handle = await fs.open(this.value, "r");
    try {
      const buffer = Buffer.alloc(length);
      const { bytesRead } = await handle.read(buffer, 0, length, offset);
      return buffer.subarray(0, bytesRead);
    } finally {
      await handle.close();
    }
  }

  /** Read and deserialize a value using an explicit serializer. */
  async read<T>(serializer: Serializer<T> | Serializer<unknown>, options?: ReadOptions): Promise<T>;

  /** Read and resolve a serializer from this Path's FileSystem registry. */
  async read(options?: ReadOptions): Promise<unknown>;

  async read<T>(
    serializerOrOptions?: Serializer<T> | Serializer<unknown> | ReadOptions,
    options?: ReadOptions
  ): Promise<unknown> {
    this.assertSafe();
    const explicit = isSerializer(serializerOrOptions);
    const serializer = explicit
      ? (serializerOrOptions as Serializer<T>)
      : this.serializers.resolve(this.value);
    const readOptions = explicit ? options : serializerOrOptions;

    if (serializer === undefined) {
      const extension = this.ext || "<no extension>";
      throw new Error(`No serializer registered for "${extension}" on ${this.value}`);
    }

    const parsed = serializer.parse(await fs.readFile(this.value));
    if (readOptions?.validate !== undefined) {
      return readOptions.validate.validate(parsed);
    }
    return parsed;
  }

  /** Write text, optionally using the atomic write path. */
  async writeText(content: string, options?: WriteOptions): Promise<void> {
    const encoding = options?.encoding ?? "utf8";
    await this.writeBytes(Buffer.from(content, encoding), options);
  }

  /** Write bytes, optionally using a same-directory temporary file and rename. */
  async writeBytes(content: Uint8Array, options?: WriteOptions): Promise<void> {
    this.assertSafe();
    if (options?.atomic === true) {
      await this.writeBytesAtomic(content, options);
      return;
    }

    await fs.writeFile(this.value, content, writeOptions(options));
  }

  /** Write and serialize a typed value. */
  async write<T>(serializer: Serializer<T>, data: T, options?: WriteOptions): Promise<void> {
    await this.writeBytes(serializer.stringify(data), options);
  }

  /**
   * Write bytes through a collision-safe temporary file and atomic rename.
   *
   * The temporary file is kept beside the target so the rename remains on one
   * filesystem. Directory fsync is best effort because some supported systems
   * do not allow opening directories for synchronization.
   */
  private async writeBytesAtomic(content: Uint8Array, options: WriteOptions): Promise<void> {
    const directory = pathe.dirname(this.value);
    const temporary = pathe.join(directory, `.${pathe.basename(this.value)}.${randomUUID()}.tmp`);

    try {
      await fs.writeFile(temporary, content, { flag: "wx", ...writeOptions(options) });

      if (options.fsync === true) {
        const fileHandle = await fs.open(temporary, "r+");
        try {
          await fileHandle.sync();
        } finally {
          await fileHandle.close();
        }
      }

      await fs.rename(temporary, this.value);

      if (options.fsync === true) {
        const directoryHandle = await fs.open(directory, "r").catch(() => undefined);
        if (directoryHandle !== undefined) {
          try {
            await directoryHandle.sync().catch(() => undefined);
          } finally {
            await directoryHandle.close().catch(() => undefined);
          }
        }
      }
    } catch (error) {
      await fs.unlink(temporary).catch(() => undefined);
      throw error;
    }
  }

  /** This path as a plain string. */
  toString(): string {
    return this.value;
  }
}
