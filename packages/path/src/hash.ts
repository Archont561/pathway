import { createReadStream } from "node:fs";
import { loadEngine } from "./binding.js";
import type { Hasher, HasherName, HashOptions } from "./types.js";
import { walkFiles } from "./walk.js";

const TREE_FORMAT = "pathway-tree-v1\0";

async function* fileChunks(path: string): AsyncGenerator<Uint8Array> {
  const stream = createReadStream(path, { highWaterMark: 64 * 1024 });
  for await (const chunk of stream) yield chunk as Uint8Array;
}

export async function hashFile(path: string, options: HashOptions = {}): Promise<string> {
  const selected = options.hasher ?? "blake3";
  if (typeof selected !== "string") return selected.digest(fileChunks(path));
  return loadEngine().hashFileNative(path, selected);
}

export async function hashTree(root: string, options: HashOptions = {}): Promise<string> {
  const selected = options.hasher ?? "blake3";
  const records: Array<{ path: string; digest: string }> = [];
  const walkHasher: HasherName = typeof selected === "string" ? selected : "sha256";
  const walkOptions =
    options.signal === undefined
      ? { hash: walkHasher }
      : { hash: walkHasher, signal: options.signal };
  for await (const batch of walkFiles(root, walkOptions)) {
    for (const entry of batch) {
      if (entry.hash !== undefined) records.push({ path: entry.value, digest: entry.hash });
    }
  }
  records.sort((left, right) => left.path.localeCompare(right.path));
  const bytes = new TextEncoder().encode(
    TREE_FORMAT +
      records
        .map(({ path, digest }) => `${path.length}:${path}:${digest.length}:${digest}`)
        .join("")
  );
  if (typeof selected !== "string") {
    return selected.digest(
      (async function* () {
        yield bytes;
      })()
    );
  }
  return loadEngine().hashBytesNative(Buffer.from(bytes), selected);
}

export type { Hasher, HasherName, HashOptions };
