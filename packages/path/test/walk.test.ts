import { afterEach, describe, expect, test } from "bun:test";
import {
  chmodSync,
  closeSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import type { PathEntry } from "../src/index.js";
import { WalkError, walk, walkDirs, walkFiles } from "../src/index.js";

/** Builds a throwaway tree; every test's setup is one call. */
const roots: string[] = [];
function tree(files: string[]): string {
  const root = mkdtempSync(join(tmpdir(), "pathway-walk-"));
  roots.push(root);
  for (const file of files) {
    const path = join(root, file);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, `contents of ${file}`);
  }
  return root;
}

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

/** Drains a walk into a sorted list of entry paths. */
async function drain(iterator: AsyncGenerator<readonly PathEntry[]>): Promise<PathEntry[]> {
  const all: PathEntry[] = [];
  for await (const batch of iterator) {
    all.push(...batch);
  }
  all.sort((a, b) => a.value.localeCompare(b.value));
  return all;
}

describe("walk", () => {
  test("yields every file and directory, root-relative", async () => {
    const root = tree(["a.ts", "src/b.ts", "src/deep/c.ts"]);
    const entries = await drain(walk(root));

    expect(entries.map((e) => e.value)).toEqual([
      "a.ts",
      "src",
      "src/b.ts",
      "src/deep",
      "src/deep/c.ts"
    ]);
    expect(entries.filter((e) => e.isDir).map((e) => e.value)).toEqual(["src", "src/deep"]);
  });

  test("respects batchSize: one boundary crossing per batch", async () => {
    const root = tree(Array.from({ length: 10 }, (_, i) => `file${i}.ts`));
    const sizes: number[] = [];
    for await (const batch of walk(root, { batchSize: 4 })) {
      sizes.push(batch.length);
    }

    expect(sizes.reduce((a, b) => a + b, 0)).toBe(10);
    expect(Math.max(...sizes)).toBeLessThanOrEqual(4);
  });

  test("applies glob patterns root-relatively", async () => {
    const root = tree(["a.ts", "src/b.ts", "src/c.rs", "src/deep/d.ts"]);
    const entries = await drain(walk(root, { glob: ["**/*.ts"] }));

    expect(entries.map((e) => e.value)).toEqual(["a.ts", "src/b.ts", "src/deep/d.ts"]);
  });

  test("populates size and modifiedNanos by default", async () => {
    const root = tree(["a.ts"]);
    const [entry] = await drain(walkFiles(root));

    expect(entry?.size).toBe("contents of a.ts".length);
    expect(typeof entry?.modifiedNanos).toBe("bigint");
    expect(entry?.modifiedNanos).toBeGreaterThan(0n);
  });

  test("computes the requested hash during the walk", async () => {
    const root = tree(["a.ts"]);
    const [entry] = await drain(walkFiles(root, { hash: "blake3" }));

    expect(entry?.hash).toMatch(/^[0-9a-f]{64}$/);
  });

  test("yields absolute paths when asked", async () => {
    const root = tree(["a.ts"]);
    const entries = await drain(walkFiles(root, { absolute: true }));

    expect(entries[0]?.value).not.toBe("a.ts");
    expect(entries[0]?.value.endsWith("a.ts")).toBe(true);
    expect(entries[0]?.value.includes("\\")).toBe(false);
  });

  test("rejects a bad glob before any traversal, naming the pattern", async () => {
    const root = tree(["a.ts"]);
    expect(drain(walk(root, { glob: ["src/**/["] }))).rejects.toThrow("src/**/[");
  });

  test("an already-aborted signal rejects before yielding", async () => {
    const root = tree(["a.ts"]);
    const controller = new AbortController();
    controller.abort();

    expect(drain(walk(root, { signal: controller.signal }))).rejects.toThrow();
  });

  test("a walk that collected failures throws WalkError instead of completing", async () => {
    const root = tree(["readable.ts", "locked.ts"]);
    const locked = join(root, "locked.ts");
    chmodSync(locked, 0o000);
    let premise: boolean;
    try {
      closeSync(openSync(locked, "r"));
      premise = false;
    } catch {
      premise = true;
    }
    const failure = await drain(walkFiles(root, { hash: "blake3" })).then(
      () => null,
      (error: unknown) => error
    );
    chmodSync(locked, 0o644);
    if (!premise) {
      // Still readable — running as root, or a platform where the mode
      // bit does not apply. The premise does not hold; nothing to prove.
      return;
    }

    expect(failure).toBeInstanceOf(WalkError);
    expect((failure as WalkError).errors.join("\n")).toContain("locked.ts");
  });

  test("entries are still yielded before WalkError ends the walk", async () => {
    const root = tree(["readable.ts", "locked.ts"]);
    const locked = join(root, "locked.ts");
    chmodSync(locked, 0o000);
    let premise: boolean;
    try {
      closeSync(openSync(locked, "r"));
      premise = false;
    } catch {
      premise = true;
    }
    const seen: PathEntry[] = [];
    let thrown: unknown = null;
    try {
      for await (const batch of walkFiles(root, { hash: "blake3" })) {
        seen.push(...batch);
      }
    } catch (error) {
      thrown = error;
    }
    chmodSync(locked, 0o644);
    if (!premise) {
      // Still readable — running as root, or a platform where the mode
      // bit does not apply. The premise does not hold; nothing to prove.
      return;
    }

    expect(thrown).toBeInstanceOf(WalkError);
    expect(seen.map((e) => e.value).sort()).toEqual(["locked.ts", "readable.ts"]);
    const failed = seen.find((e) => e.value === "locked.ts");
    expect(failed?.error?.kind).toBe("read");
    expect(failed?.hash).toBeUndefined();
    const ok = seen.find((e) => e.value === "readable.ts");
    expect(ok?.hash).toMatch(/^[0-9a-f]{64}$/);
  });

  test("aborting between batches stops the walk", async () => {
    const root = tree(Array.from({ length: 20 }, (_, i) => `file${i}.ts`));
    const controller = new AbortController();
    const iterator = walk(root, { batchSize: 5, signal: controller.signal });

    const first = await iterator.next();
    expect(first.done).toBe(false);
    controller.abort();

    expect(
      (async () => {
        for (;;) {
          const r = await iterator.next();
          if (r.done) break;
        }
      })()
    ).rejects.toThrow();
  });
});

describe("walkFiles / walkDirs", () => {
  test("walkFiles yields only files", async () => {
    const root = tree(["a.ts", "src/b.ts"]);
    const entries = await drain(walkFiles(root));

    expect(entries.map((e) => e.value)).toEqual(["a.ts", "src/b.ts"]);
    expect(entries.every((e) => !e.isDir)).toBe(true);
  });

  test("walkDirs yields only directories", async () => {
    const root = tree(["src/b.ts", "src/deep/c.ts", "lib/d.ts"]);
    const entries = await drain(walkDirs(root));

    expect(entries.map((e) => e.value)).toEqual(["lib", "src", "src/deep"]);
    expect(entries.every((e) => e.isDir)).toBe(true);
  });
});
