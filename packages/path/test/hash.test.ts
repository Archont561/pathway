import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { hashFile, hashTree, Path, type Hasher } from "../src/index.js";

const roots: string[] = [];
function tree(files: Record<string, string>): string {
  const root = mkdtempSync(join("/tmp", "pathway-hash-"));
  roots.push(root);
  for (const [name, contents] of Object.entries(files)) {
    const path = join(root, name);
    mkdirSync(join(path, ".."), { recursive: true });
    writeFileSync(path, contents);
  }
  return root;
}
afterEach(() => roots.splice(0).forEach((root) => rmSync(root, { recursive: true, force: true })));

describe("hashing", () => {
  test("hashes a file with every built-in algorithm", async () => {
    const root = tree({ "a.txt": "hello" });
    for (const hasher of ["blake3", "xxhash", "sha256"] as const) {
      expect(await hashFile(join(root, "a.txt"), { hasher })).toMatch(/^[0-9a-f]+$/);
    }
  });

  test("hashes trees deterministically and tracks content changes", async () => {
    const root = tree({ "z.txt": "z", "a.txt": "a", "nested/m.txt": "m" });
    const first = await new Path(root).hashTree({ hasher: "sha256" });
    expect(await hashTree(root, { hasher: "sha256" })).toBe(first);
    writeFileSync(join(root, "a.txt"), "changed");
    expect(await new Path(root).hashTree({ hasher: "sha256" })).not.toBe(first);
  });

  test("passes bounded chunks to a custom file hasher", async () => {
    const root = tree({ "large.bin": "x".repeat(128 * 1024 + 1) });
    const sizes: number[] = [];
    const hasher: Hasher = {
      name: "counting",
      digestLength: 8,
      async digest(chunks) {
        let bytes = 0;
        for await (const chunk of chunks) {
          sizes.push(chunk.byteLength);
          bytes += chunk.byteLength;
        }
        return String(bytes);
      }
    };
    expect(await hashFile(join(root, "large.bin"), { hasher })).toBe("131073");
    expect(Math.max(...sizes)).toBeLessThanOrEqual(64 * 1024);
    expect(sizes.length).toBeGreaterThan(1);
  });
});
