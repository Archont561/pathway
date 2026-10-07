/**
 * `@archont561/pathway` — a native, pathlib-inspired filesystem API.
 *
 * Two layers, and the split is the project's central architectural claim:
 *
 * 1. **This surface.** Path objects, pluggable serializers, async iteration.
 *    Boring on purpose — it is the layer a build tool author uses all day, so
 *    it stays in JavaScript where string manipulation is cheap.
 * 2. **The native engine.** Traversal, pruning, hashing and parallel I/O, in
 *    Rust, behind a deliberately coarse N-API boundary.
 *
 * The boundary is coarse on purpose. It is crossed for bulk operations and
 * never for a single string, because a boundary crossing costs more than the
 * string operation it would wrap.
 *
 * @packageDocumentation
 */

export { engineAvailable } from "./binding.js";
export { FileSystem } from "./filesystem.js";
export { hashFile, hashTree } from "./hash.js";
export { Path } from "./path.js";
export { ContainmentError, Sandbox, SandboxPath } from "./sandbox.js";
export { json, SerializerRegistry } from "./serializers/index.js";
export type {
  BulkOperationError,
  CopyOptions,
  CopyResult,
  EntryErrorKind,
  FileSystemOptions,
  FileTransform,
  Hasher,
  HasherName,
  HashOptions,
  MoveOptions,
  MoveResult,
  PathEntry,
  ReadOptions,
  Serializer,
  TransformOptions,
  TransformResult,
  Validator,
  WriteOptions
} from "./types.js";

export type { WalkBatch, WalkOptions } from "./walk.js";
export { WalkError, walk, walkDirs, walkFiles } from "./walk.js";
