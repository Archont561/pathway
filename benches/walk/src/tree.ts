/**
 * The file-tree generator.
 *
 * A benchmark of a walker is only as trustworthy as the tree it walks, so this
 * is deliberately not a flat directory with a million entries in it — real
 * trees are deep and unevenly sized, and a flat tree measures `readdir`
 * without measuring anything a walker actually has to do.
 *
 * Two properties matter for comparability:
 *
 * - **Deterministic.** The layout comes from an index, not from `Math.random`,
 *   so the 10k tree is a prefix of the 100k tree and two runs of the harness
 *   walk byte-identical directories. A benchmark whose subject changes between
 *   runs cannot be regressed against.
 * - **Non-empty files.** Benchmark C hashes every file, so empty files would
 *   make the hashing half of the fused pass measure `open`/`close` on nothing.
 */

import { mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

/** Sizes the harness generates by default. 10k is the smoke size; 1M is the claim size. */
export const TREE_SIZES = [10_000, 100_000, 500_000, 1_000_000] as const;

/** One of the four default sizes. A run may override them with any positive count. */
export type CanonicalTreeSize = (typeof TREE_SIZES)[number];

/**
 * The names the generator draws directory and file names from.
 *
 * Deliberately mundane and repeated. Real trees are `node_modules` and `src`
 * over and over, and the cost profile of a walk depends on how much of the
 * work is directory-name interning — so the generator has to repeat names the
 * way a checkout does rather than inventing unique ones.
 */
const WORDS = [
  "src",
  "lib",
  "test",
  "dist",
  "build",
  "node_modules",
  "target",
  "vendor",
  "pkg",
  "internal",
  "api",
  "core",
  "util",
  "types",
  "config"
] as const;

const EXTENSIONS = [".ts", ".js", ".json", ".md", ".txt", ".rs"] as const;

/** A 32-bit mix of an integer. Deterministic, and spreads indices over the name table. */
function mix(n: number): number {
  let x = n | 0;
  x = x ^ 61 ^ (x >>> 16);
  x = x + (x << 3);
  x = x ^ (x >>> 4);
  x = Math.imul(x, 0x27d4eb2d);
  x = x ^ (x >>> 15);
  return x >>> 0;
}

/** The bytes a generated file holds. Fixed-size so hashing cost is content-independent. */
function payload(seed: number, size: number): string {
  const unit = `line ${seed % 9973} of the generated corpus\n`;
  let out = "";
  while (out.length < size) {
    out += unit;
  }
  return out.slice(0, size);
}

export interface TreeOptions {
  /** Bytes per generated file. Small enough that 1M files stay inside a laptop's disk. */
  fileSize?: number;
  /** Approximate directory count. Defaults to files/50, which is a plausible src tree. */
  directories?: number;
}

/**
 * Build the tree at `root` with exactly `files` files in it.
 *
 * Returns the number of files written, which the caller asserts against the
 * requested count: a generator that silently wrote 40% of what was asked for
 * would make every downstream number a comparison between different trees.
 */
export async function buildTree(
  root: string,
  files: number,
  options: TreeOptions = {}
): Promise<number> {
  const fileSize = options.fileSize ?? 64;
  const directories = options.directories ?? Math.max(1, Math.floor(files / 50));

  await rm(root, { recursive: true, force: true });
  await mkdir(root, { recursive: true });

  const dirs: string[] = [root];
  for (let d = 1; d < directories; d++) {
    const a = mix(d);
    const b = mix(d + 0x9e3779b9);
    const dir = join(
      root,
      WORDS[a % WORDS.length] as string,
      WORDS[b % WORDS.length] as string,
      `${(a >>> 8) % 997}`
    );
    dirs.push(dir);
  }
  await Promise.all(dirs.map((dir) => mkdir(dir, { recursive: true })));

  let written = 0;
  // Batched so a 1M-file tree does not hold a million promises at once, and so
  // the writes actually overlap — the generator is not the thing under test,
  // but it should not become the bottleneck that dominates the measurement.
  const batchSize = 2_000;
  for (let start = 0; start < files; start += batchSize) {
    const end = Math.min(start + batchSize, files);
    const work: Promise<void>[] = [];
    for (let i = start; i < end; i++) {
      const h = mix(i);
      const dir = dirs[h % dirs.length] as string;
      // The index is in the name, not just the hash of it. `mix` is not
      // injective, so hash-only names collide somewhere around 100k files — and
      // a collision silently overwrites, which makes the tree hold fewer files
      // than the harness claims it generated while every counter still agrees.
      const name = `${WORDS[(h >>> 5) % WORDS.length] as string}-${i}-${h % 9_973}${EXTENSIONS[(h >>> 11) % EXTENSIONS.length] as string}`;
      work.push(writeFile(join(dir, name), payload(h, fileSize)));
    }
    await Promise.all(work);
    written += work.length;
  }
  return written;
}
