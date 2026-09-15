---
title: "TypeScript Path Class, Serializer<T>, WalkIterator, FileSystem"
domain: implementation
status: decided
created: 2025-07-11
updated: 2025-07-11
source: conversation
depends_on:
  - architecture/core-layers
  - features/walk-traversal
  - features/serializers
  - implementation/code-rust-walker
  - implementation/repo-structure
tags: [typescript, code, path, serializer, walk, filesystem, api]
---

# TypeScript API Implementation

## Overview

This document contains the production-grade TypeScript implementation for
the `Path` class, `Serializer<T>` interface, `WalkIterator`, and supporting
types. All path string manipulation uses `pathe`. All I/O-heavy operations
delegate to the NAPI-RS Rust engine.

---

## Serializer Interface (`packages/path/src/serializers/types.ts`)

```typescript
/**
 * Pluggable serialization strategy.
 * Implementations can be JS-native (JSON) or Rust-backed (TOML, YAML).
 */
export interface Serializer<T = unknown> {
  readonly name: string;
  parse(input: string): T;
  stringify(value: T): string;
}
```

---

## Built-in JSON Serializer (`packages/path/src/serializers/json.ts`)

```typescript
import type { Serializer } from "./types.js";

/**
 * Built-in JSON serializer.
 * Uses V8's native JSON.parse/stringify (C++ implementation, very fast).
 */
export const json: Serializer<unknown> = {
  name: "json",
  parse: (input: string) => JSON.parse(input),
  stringify: (value: unknown) => JSON.stringify(value, null, 2),
};
```

---

## Core Types (`packages/path/src/types.ts`)

```typescript
import type { Serializer } from "./serializers/types.js";
import type { Path } from "./path.js";

/** Options for directory walking */
export interface WalkOptions {
  /** Glob pattern(s) for file matching */
  glob?: string | string[];
  /** Regex pattern for file matching (applied to full path) */
  regex?: RegExp;
  /** Directory names to prune (never descend into) */
  exclude?: string[];
  /** Maximum directory depth */
  maxDepth?: number;
  /** Only yield files (default: true) */
  filesOnly?: boolean;
  /** Include stat metadata in results */
  withMetadata?: boolean;
  /** Compute content hash */
  hash?: "blake3" | "sha256" | "xxhash";
  /** JS predicate filter (post-native, may be async) */
  filter?: WalkFilter;
  /** N-API batch size (default: 512) */
  batchSize?: number;
}

/** Filter predicate for walk results */
export type WalkFilter = (entry: PathEntry) => boolean | Promise<boolean>;

/** A fully populated walk result entry */
export interface PathEntry {
  readonly path: Path;
  readonly size: number;
  readonly mtime: Date;
  readonly isFile: boolean;
  readonly isDirectory: boolean;
  readonly isSymlink: boolean;
  readonly hash?: string;
  relativeTo(base: Path | string): string;
}

/** Options for write operations */
export interface WriteOptions {
  /** Write to temp file, then atomic rename */
  atomic?: boolean;
  /** fsync before rename (only with atomic) */
  fsync?: boolean;
  /** File permissions (default: 0o644) */
  mode?: number;
  /** String encoding (default: "utf-8") */
  encoding?: BufferEncoding;
}

/** Options for read operations */
export interface ReadOptions {
  /** String encoding (default: "utf-8") */
  encoding?: BufferEncoding;
  /** Schema validator (runs after parse) */
  validate?: Validator<unknown>;
}

/** Schema validator interface */
export interface Validator<T> {
  readonly name: string;
  validate(data: unknown): T;
}
```

---

## NAPI-RS Binding Loader (`packages/path/src/binding.ts`)

```typescript
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

interface NativeBinding {
  NativeScanner: new (root: string, options: NativeWalkOptions) => NativeScannerInstance;
  version: () => string;
}

interface NativeWalkOptions {
  glob?: string;
  regex?: string;
  exclude?: string[];
  maxDepth?: number;
  filesOnly?: boolean;
  withMetadata?: boolean;
  hash?: string;
  batchSize?: number;
  concurrency?: number;
}

interface NativeScannerInstance {
  scan(): Promise<number>;
  nextBatch(): NativeFusedEntry[];
  reset(): void;
}

interface NativeFusedEntry {
  path: string;
  size: number;
  mtime: number;
  hash: string;
  isFile: boolean;
  isDirectory: boolean;
  isSymlink: boolean;
}

let binding: NativeBinding | null = null;

export function getBinding(): NativeBinding {
  if (binding) return binding;

  try {
    // NAPI-RS loads the correct platform binary automatically
    binding = require("@myorg/path") as NativeBinding;
  } catch {
    throw new Error(
      "Failed to load @myorg/path native binding. " +
      "Ensure the correct platform package is installed."
    );
  }

  return binding;
}

export type { NativeWalkOptions, NativeScannerInstance, NativeFusedEntry };
```

---

## Path Class (`packages/path/src/path.ts`)

```typescript
import * as pathe from "pathe";
import * as fs from "node:fs/promises";
import * as nodePath from "node:path";
import type { Serializer } from "./serializers/types.js";
import type { WalkOptions, PathEntry, WriteOptions, ReadOptions } from "./types.js";
import { getBinding } from "./binding.js";

export class Path {
  readonly value: string;

  constructor(rawPath: string) {
    this.value = pathe.normalize(rawPath);
  }

  // ── Static Constructors ──────────────────────────────────────

  static cwd(): Path {
    return new Path(process.cwd());
  }

  static home(): Path {
    const home = process.env.HOME || process.env.USERPROFILE;
    if (!home) throw new Error("Cannot determine home directory");
    return new Path(home);
  }

  static tempDir(): Path {
    return new Path(pathe.resolve(
      process.env.TMPDIR || process.env.TMP || process.env.TEMP || "/tmp"
    ));
  }

  // ── Path Manipulation (JS-only, via pathe) ───────────────────

  join(...segments: string[]): Path {
    return new Path(pathe.join(this.value, ...segments));
  }

  resolve(...segments: string[]): Path {
    return new Path(pathe.resolve(this.value, ...segments));
  }

  relativeTo(base: Path | string): string {
    const basePath = base instanceof Path ? base.value : base;
    return pathe.relative(basePath, this.value);
  }

  get dirname(): Path {
    return new Path(pathe.dirname(this.value));
  }

  get basename(): string {
    return pathe.basename(this.value);
  }

  get extname(): string {
    return pathe.extname(this.value);
  }

  get isAbsolute(): boolean {
    return pathe.isAbsolute(this.value);
  }

  toString(): string {
    return this.value;
  }

  // ── File I/O ─────────────────────────────────────────────────

  async readText(encoding: BufferEncoding = "utf-8"): Promise<string> {
    return fs.readFile(this.value, { encoding });
  }

  async readBytes(length?: number): Promise<Buffer> {
    if (length) {
      const handle = await fs.open(this.value, "r");
      try {
        const buffer = Buffer.alloc(length);
        await handle.read(buffer, 0, length, 0);
        return buffer;
      } finally {
        await handle.close();
      }
    }
    return fs.readFile(this.value);
  }

  async read<T = unknown>(
    serializer: Serializer<T>,
    options?: ReadOptions,
  ): Promise<T> {
    const text = await this.readText(options?.encoding);
    const parsed = serializer.parse(text);

    if (options?.validate) {
      return options.validate.validate(parsed) as T;
    }

    return parsed;
  }

  async writeText(
    content: string,
    options?: WriteOptions,
  ): Promise<void> {
    if (options?.atomic) {
      await this.writeTextAtomic(content, options);
    } else {
      await fs.writeFile(this.value, content, {
        encoding: options?.encoding || "utf-8",
        mode: options?.mode,
      });
    }
  }

  async writeTextAtomic(
    content: string,
    options?: WriteOptions,
  ): Promise<void> {
    const dir = pathe.dirname(this.value);
    const tempPath = pathe.join(
      dir,
      `.${pathe.basename(this.value)}.${Date.now()}.${Math.random().toString(36).slice(2)}.tmp`,
    );

    try {
      await fs.writeFile(tempPath, content, {
        encoding: options?.encoding || "utf-8",
        mode: options?.mode,
      });

      if (options?.fsync) {
        const handle = await fs.open(tempPath, "r+");
        try {
          await handle.sync();
        } finally {
          await handle.close();
        }
      }

      await fs.rename(tempPath, this.value);
    } catch (err) {
      // Clean up temp file on failure
      await fs.unlink(tempPath).catch(() => {});
      throw err;
    }
  }

  async write<T>(
    serializer: Serializer<T>,
    data: T,
    options?: WriteOptions,
  ): Promise<void> {
    const serialized = serializer.stringify(data);
    await this.writeText(serialized, options);
  }

  // ── Directory Operations ─────────────────────────────────────

  async mkdir(options?: { recursive?: boolean; mode?: number }): Promise<void> {
    await fs.mkdir(this.value, {
      recursive: options?.recursive ?? true,
      mode: options?.mode,
    });
  }

  async remove(options?: { recursive?: boolean; force?: boolean }): Promise<void> {
    await fs.rm(this.value, {
      recursive: options?.recursive ?? true,
      force: options?.force ?? true,
    });
  }

  async exists(): Promise<boolean> {
    try {
      await fs.access(this.value);
      return true;
    } catch {
      return false;
    }
  }

  async stat(): Promise<import("node:fs").Stats> {
    return fs.stat(this.value);
  }

  // ── Traversal ────────────────────────────────────────────────

  async *walkFiles(options: WalkOptions = {}): AsyncIterableIterator<PathEntry> {
    const binding = getBinding();
    const scanner = new binding.NativeScanner(this.value, {
      glob: Array.isArray(options.glob)
        ? options.glob.join(",")
        : options.glob,
      regex: options.regex?.source,
      exclude: options.exclude,
      maxDepth: options.maxDepth,
      filesOnly: options.filesOnly ?? true,
      withMetadata: options.withMetadata ?? false,
      hash: options.hash,
      batchSize: options.batchSize ?? 512,
    });

    // Trigger the scan (runs on Rust blocking thread)
    await scanner.scan();

    // Consume batches
    let batch = scanner.nextBatch();
    while (batch.length > 0) {
      for (const raw of batch) {
        const entry: PathEntry = {
          path: new Path(raw.path),
          size: raw.size,
          mtime: new Date(raw.mtime),
          isFile: raw.isFile,
          isDirectory: raw.isDirectory,
          isSymlink: raw.isSymlink,
          hash: raw.hash || undefined,
          relativeTo: (base: Path | string) => {
            const basePath = base instanceof Path ? base.value : base;
            return pathe.relative(basePath, raw.path);
          },
        };

        // Apply JS predicate if present
        if (options.filter) {
          const passes = await options.filter(entry);
          if (!passes) continue;
        }

        yield entry;
      }
      batch = scanner.nextBatch();
    }
  }

  async *walkDirs(options: WalkOptions = {}): AsyncIterableIterator<PathEntry> {
    yield* this.walkFiles({ ...options, filesOnly: false });
    // Note: In production, this would use a dedicated dirs-only walk.
    // Simplified here for the v0.1 implementation.
  }

  async *walk(options: WalkOptions = {}): AsyncIterableIterator<PathEntry> {
    yield* this.walkFiles({ ...options, filesOnly: false });
  }
}
```

---

## Public Exports (`packages/path/src/index.ts`)

```typescript
export { Path } from "./path.js";
export { json } from "./serializers/json.js";
export type { Serializer } from "./serializers/types.js";
export type {
  WalkOptions,
  WalkFilter,
  PathEntry,
  WriteOptions,
  ReadOptions,
  Validator,
} from "./types.js";
```

---

## Usage Examples

### Basic Path Operations

```typescript
import { Path, json } from "@myorg/path";

const project = Path.cwd();
const pkg = project.join("package.json");
const data = await pkg.read(json);
console.log(data.name);
```

### Fused Walk with Hashing

```typescript
for await (const entry of project.walkFiles({
  glob: "**/*.{ts,tsx}",
  exclude: ["node_modules", ".git", "dist"],
  withMetadata: true,
  hash: "blake3",
})) {
  console.log(`${entry.path.relativeTo(project)} ${entry.size}B ${entry.hash}`);
}
```

### Atomic Config Write

```typescript
import { toml } from "@myorg/path-toml";

const config = project.join("config.toml");
await config.write(toml, { server: { port: 3000 } }, { atomic: true });
```

---

## Key Implementation Notes

1. **All path manipulation is JS-only.** `join()`, `resolve()`, `dirname`,
   etc. use `pathe` and never cross the N-API boundary.

2. **`readText()` uses `node:fs`.** For v0.1, simple file reads use Node's
   built-in `fs.promises`. A future optimization could use the Rust engine
   for large files or Bun's `Bun.file()` when running on Bun.

3. **Atomic writes use temp+rename.** The temp file is created in the same
   directory as the target to ensure the rename is atomic (same filesystem).

4. **Walk batches are consumed lazily.** The `scan()` call triggers the
   full Rust traversal, but results are consumed in batches via `nextBatch()`.
   This keeps memory bounded even for million-file trees.

5. **JS `filter` is post-native.** The predicate runs after the native
   filters have already pruned the search space, minimizing boundary
   crossings.

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| `pathe` for all string ops | Zero-cost; no FFI overhead |
| `node:fs` for simple I/O (v0.1) | Proven, stable; optimize later |
| Atomic write via temp+rename | OS-level atomicity on same filesystem |
| Lazy batch consumption | Bounded memory; clean async iterator |
| `Serializer<T>` as separate interface | Composable; independent of Path |
| JSON stays in JS | V8 `JSON.parse()` is faster than Serde→N-API |
| `filter` is post-native | Documented trade-off; flexibility over speed |
