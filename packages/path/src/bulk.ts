import {
  copyFile,
  cp,
  lstat,
  mkdir,
  readdir,
  readFile,
  readlink,
  realpath,
  rename,
  rm,
  stat,
  symlink
} from "node:fs/promises";
import * as pathe from "pathe";
import { Path } from "./path.js";
import type {
  BulkOperationError,
  CopyOptions,
  CopyResult,
  FileTransform,
  MoveOptions,
  MoveResult,
  TransformOptions,
  TransformResult
} from "./types.js";

const DEFAULT_CONCURRENCY = 8;

type EntryKind = "file" | "directory" | "symlink";

interface SourceEntry {
  readonly absolute: string;
  readonly relative: string;
  readonly kind: EntryKind;
}

interface CollectedTree {
  readonly isDirectory: boolean;
  readonly entries: readonly SourceEntry[];
}

function normaliseRelative(value: string): string {
  return value.replaceAll("\\", "/");
}

function expandBraces(pattern: string): string[] {
  const opening = pattern.indexOf("{");
  if (opening < 0) return [pattern];
  const closing = pattern.indexOf("}", opening + 1);
  if (closing < 0) return [pattern];
  const alternatives = pattern.slice(opening + 1, closing).split(",");
  return alternatives.flatMap((alternative) =>
    expandBraces(pattern.slice(0, opening) + alternative + pattern.slice(closing + 1))
  );
}

function globRegex(pattern: string): RegExp {
  const value = normaliseRelative(pattern).replace(/^\.\//, "");
  let source = "^";
  for (let index = 0; index < value.length; index += 1) {
    const character = value.charAt(index);
    if (character === "*" && value[index + 1] === "*") {
      if (value[index + 2] === "/") {
        source += "(?:.*/)?";
        index += 2;
      } else {
        source += ".*";
        index += 1;
      }
    } else if (character === "*") {
      source += "[^/]*";
    } else if (character === "?") {
      source += "[^/]";
    } else {
      source += /[\\^$.*+?()[\]{}|]/.test(character) ? `\\${character}` : character;
    }
  }
  return new RegExp(`${source}$`);
}

function matchesAny(value: string, patterns: readonly string[]): boolean {
  return patterns.some((pattern) =>
    expandBraces(pattern).some((expanded) => globRegex(expanded).test(value))
  );
}

function patternsOf(value: string | readonly string[] | undefined): readonly string[] {
  if (value === undefined) return [];
  return typeof value === "string" ? [value] : value;
}

function matchesSource(relative: string, source: string, patterns: readonly string[]): boolean {
  if (patterns.length === 0) return true;
  return matchesAny(relative === "" ? pathe.basename(source) : relative, patterns);
}

function isExcluded(relative: string, patterns: readonly string[]): boolean {
  const components = relative.split("/");
  return patterns.some((pattern) => {
    if (!pattern.includes("/") && components.includes(pattern)) return true;
    return matchesAny(relative, [pattern]);
  });
}

function concurrencyOf(value: number | undefined): number {
  if (value === undefined) return DEFAULT_CONCURRENCY;
  if (!Number.isInteger(value) || value < 1) {
    throw new RangeError("concurrency must be a positive integer");
  }
  return value;
}

function throwIfAborted(signal: AbortSignal | undefined): void {
  signal?.throwIfAborted();
}

async function classify(path: string, followSymlinks: boolean): Promise<EntryKind> {
  const linkStats = await lstat(path);
  if (!linkStats.isSymbolicLink() || !followSymlinks) {
    if (linkStats.isDirectory()) return "directory";
    if (linkStats.isFile()) return "file";
    return "symlink";
  }

  const targetStats = await stat(path);
  return targetStats.isDirectory() ? "directory" : "file";
}

async function collectTree(source: string, options: CopyOptions): Promise<CollectedTree> {
  const followSymlinks = options.followSymlinks === true;
  const rootKind = await classify(source, followSymlinks);
  if (rootKind !== "directory") {
    return { isDirectory: false, entries: [{ absolute: source, relative: "", kind: rootKind }] };
  }

  const entries: SourceEntry[] = [];
  const visitedDirectories = new Set<string>();

  async function visit(directory: string, relativeDirectory: string): Promise<void> {
    throwIfAborted(options.signal);
    const identity = followSymlinks ? await realpath(directory) : directory;
    if (visitedDirectories.has(identity)) {
      throw new Error(`symlink cycle or repeated directory encountered at ${directory}`);
    }
    visitedDirectories.add(identity);

    const children = await readdir(directory, { withFileTypes: true });
    for (const child of children) {
      throwIfAborted(options.signal);
      const absolute = pathe.join(directory, child.name);
      const relative = normaliseRelative(
        relativeDirectory === "" ? child.name : pathe.join(relativeDirectory, child.name)
      );
      if (isExcluded(relative, options.exclude ?? [])) {
        continue;
      }

      const kind = await classify(absolute, followSymlinks);
      entries.push({ absolute, relative, kind });
      if (kind === "directory") {
        await visit(absolute, relative);
      }
    }

    visitedDirectories.delete(identity);
  }

  await visit(source, "");
  return { isDirectory: true, entries };
}

async function runWorkers(
  entries: readonly SourceEntry[],
  concurrency: number,
  signal: AbortSignal | undefined,
  operation: (entry: SourceEntry) => Promise<void>
): Promise<{ completed: number; errors: BulkOperationError[] }> {
  let cursor = 0;
  let completed = 0;
  const errors: BulkOperationError[] = [];

  async function worker(): Promise<void> {
    for (;;) {
      throwIfAborted(signal);
      const index = cursor;
      cursor += 1;
      const entry = entries[index];
      if (entry === undefined) return;
      try {
        await operation(entry);
        completed += 1;
      } catch (error) {
        if (signal?.aborted) throw signal.reason;
        errors.push({ path: entry.absolute, message: String(error) });
      }
    }
  }

  await Promise.all(Array.from({ length: Math.min(concurrency, entries.length) }, worker));
  return { completed, errors };
}

async function prepareDestination(
  destination: string,
  isDirectory: boolean,
  entries: readonly SourceEntry[] = []
): Promise<void> {
  if (isDirectory) {
    await mkdir(destination, { recursive: true });
  } else {
    await mkdir(pathe.dirname(destination), { recursive: true });
  }

  // A directory's children are created before file workers start, so parallel
  // workers never race to create their parent directories.
  for (const entry of entries) {
    if (entry.kind === "directory") {
      await mkdir(pathe.join(destination, entry.relative), { recursive: true });
    }
  }
}

async function copyEntry(
  entry: SourceEntry,
  destination: string,
  followSymlinks: boolean
): Promise<void> {
  const target = entry.relative === "" ? destination : pathe.join(destination, entry.relative);
  if (entry.kind === "symlink" && !followSymlinks) {
    await rm(target, { recursive: true, force: true });
    await symlink(await readlink(entry.absolute), target);
    return;
  }
  await copyFile(entry.absolute, target);
}

function rootEntry(tree: CollectedTree): SourceEntry {
  const entry = tree.entries[0];
  if (entry === undefined) {
    throw new Error("a non-directory source must have one entry");
  }
  return { ...entry, relative: "" };
}

function canUseNativeCopy(options: CopyOptions): boolean {
  return (
    options.glob === undefined &&
    (options.exclude === undefined || options.exclude.length === 0) &&
    options.followSymlinks !== true &&
    options.concurrency === undefined &&
    options.signal === undefined
  );
}

async function countCopiedEntries(path: string): Promise<number> {
  const information = await lstat(path);
  if (!information.isDirectory()) return 1;
  let count = 0;
  for (const child of await readdir(path, { withFileTypes: true })) {
    count += child.isDirectory() ? await countCopiedEntries(pathe.join(path, child.name)) : 1;
  }
  return count;
}

/** Copies one path tree using a bounded worker pool. */
export async function copyTree(
  source: string,
  destination: string,
  options: CopyOptions = {}
): Promise<CopyResult> {
  throwIfAborted(options.signal);
  const concurrency = concurrencyOf(options.concurrency);
  if (canUseNativeCopy(options)) {
    await mkdir(pathe.dirname(destination), { recursive: true });
    await cp(source, destination, { recursive: true, dereference: false, force: true });
    return { copied: await countCopiedEntries(destination), skipped: 0, errors: [] };
  }

  const tree = await collectTree(source, options);
  const globs = patternsOf(options.glob);
  const files = tree.entries.filter(
    (entry) => entry.kind !== "directory" && matchesSource(entry.relative, source, globs)
  );
  const totalFiles = tree.entries.filter((entry) => entry.kind !== "directory").length;

  if (!tree.isDirectory) {
    const selected = matchesSource("", source, globs);
    if (!selected) {
      return { copied: 0, skipped: 1, errors: [] };
    }
    await prepareDestination(destination, false);
    const result = await runWorkers([rootEntry(tree)], concurrency, options.signal, (entry) =>
      copyEntry(entry, destination, options.followSymlinks === true)
    );
    return { copied: result.completed, skipped: 0, errors: result.errors };
  }

  await prepareDestination(destination, true, tree.entries);
  const result = await runWorkers(files, concurrency, options.signal, (entry) =>
    copyEntry(entry, destination, options.followSymlinks === true)
  );
  return {
    copied: result.completed,
    skipped: totalFiles - files.length,
    errors: result.errors
  };
}

/** Moves one path, falling back to copy-and-remove across filesystems. */
export async function movePath(
  source: string,
  destination: string,
  options: MoveOptions = {}
): Promise<MoveResult> {
  throwIfAborted(options.signal);
  await mkdir(pathe.dirname(destination), { recursive: true });
  try {
    await rename(source, destination);
    return { moved: 1, errors: [] };
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "EXDEV") {
      throw error;
    }
  }

  const copied = await copyTree(
    source,
    destination,
    options.signal === undefined ? {} : { signal: options.signal }
  );
  if (copied.errors.length > 0) {
    return { moved: 0, errors: copied.errors };
  }

  try {
    await rm(source, { recursive: true, force: false });
    return { moved: 1, errors: [] };
  } catch (error) {
    return {
      moved: 0,
      errors: [{ path: source, message: String(error) }]
    };
  }
}

async function applyTransforms(
  content: string,
  path: Path,
  transforms: readonly FileTransform[]
): Promise<string> {
  let current = content;
  for (const transform of transforms) {
    current = await transform(current, path);
  }
  return current;
}

/** Transforms selected text files in parallel while aggregating per-file errors. */
export async function transformTree(
  source: string,
  destination: string,
  options: TransformOptions
): Promise<TransformResult> {
  throwIfAborted(options.signal);
  const tree = await collectTree(source, options);
  const concurrency = concurrencyOf(options.concurrency);
  const globs = patternsOf(options.glob);
  const transforms = Array.isArray(options.transform) ? options.transform : [options.transform];
  const files = tree.entries.filter(
    (entry) => entry.kind !== "directory" && matchesSource(entry.relative, source, globs)
  );
  const totalFiles = tree.entries.filter((entry) => entry.kind !== "directory").length;

  if (!tree.isDirectory) {
    const selected = matchesSource("", source, globs);
    if (!selected) {
      return { transformed: 0, skipped: 1, errors: [] };
    }
    await prepareDestination(destination, false);
    const result = await runWorkers(
      [rootEntry(tree)],
      concurrency,
      options.signal,
      async (entry) => {
        const content = await readFile(entry.absolute, "utf8");
        const output = await applyTransforms(content, new Path(entry.absolute), transforms);
        await new Path(destination).writeText(output);
      }
    );
    return { transformed: result.completed, skipped: 0, errors: result.errors };
  }

  await prepareDestination(destination, true, tree.entries);
  const result = await runWorkers(files, concurrency, options.signal, async (entry) => {
    if (entry.kind === "symlink" && options.followSymlinks !== true) {
      await copyEntry(entry, destination, false);
      return;
    }
    const content = await readFile(entry.absolute, "utf8");
    const output = await applyTransforms(content, new Path(entry.absolute), transforms);
    await new Path(pathe.join(destination, entry.relative)).writeText(output);
  });
  return {
    transformed: result.completed,
    skipped: totalFiles - files.length,
    errors: result.errors
  };
}
