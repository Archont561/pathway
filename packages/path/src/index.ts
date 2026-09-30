/**
 * `@myorg/path` — a native, pathlib-inspired filesystem API.
 *
 * Two layers, and the split is the project's central architectural claim:
 *
 * 1. **This surface.** Path objects, pluggable serializers, async iteration.
 *    Boring on purpose — it is the layer a build tool author uses all day, so
 *    it stays in JavaScript where string manipulation is free.
 * 2. **The native engine.** Traversal, pruning, hashing and parallel I/O, in
 *    Rust, behind a deliberately coarse N-API boundary.
 *
 * The boundary is coarse on purpose. It is crossed for bulk operations and
 * never for a single string, because a boundary crossing costs more than the
 * string operation it would wrap — see D2.
 *
 * @packageDocumentation
 */

export { engineAvailable } from "./binding.js";
export { Path } from "./path.js";
export { json } from "./serializers/index.js";
export type { EntryErrorKind, HasherName, PathEntry, Serializer } from "./types.js";

export type { WalkBatch, WalkOptions } from "./walk.js";
export { walk, walkDirs, walkFiles } from "./walk.js";
