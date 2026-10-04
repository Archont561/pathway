import { afterEach, describe, expect, test } from "bun:test";
import {
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readlinkSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { Path } from "../src/index.js";

const roots: string[] = [];

function temporaryRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "pathway-operations-"));
  roots.push(root);
  return root;
}

function writeTree(root: string, files: Record<string, string>): void {
  for (const [relative, content] of Object.entries(files)) {
    const path = join(root, relative);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, content);
  }
}

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe("Path.copyTo", () => {
  test("copies an unfiltered tree and reports every file", async () => {
    const root = temporaryRoot();
    const source = join(root, "source");
    const destination = join(root, "destination");
    writeTree(source, { "a.txt": "a", "nested/b.txt": "b" });

    const summary = await new Path(source).copyTo(new Path(destination));

    expect(summary).toEqual({ copied: 2, skipped: 0, errors: [] });
    expect(readFileSync(join(destination, "nested/b.txt"), "utf8")).toBe("b");
  });

  test("copies a tree, filters entries, and preserves symlinks by default", async () => {
    const root = temporaryRoot();
    const source = join(root, "source");
    const destination = join(root, "destination");
    writeTree(source, {
      "src/main.ts": "main",
      "src/readme.md": "readme",
      "node_modules/ignored.ts": "ignored"
    });
    if (process.platform !== "win32") {
      const link = join(source, "src/link.ts");
      symlinkSync("main.ts", link);
    }

    const summary = await new Path(source).copyTo(new Path(destination), {
      glob: "**/*.ts",
      exclude: ["node_modules"],
      concurrency: 2
    });

    expect(readFileSync(join(destination, "src/main.ts"), "utf8")).toBe("main");
    expect(() => readFileSync(join(destination, "src/readme.md"))).toThrow();
    expect(() => readFileSync(join(destination, "node_modules/ignored.ts"))).toThrow();
    if (process.platform !== "win32") {
      expect(lstatSync(join(destination, "src/link.ts")).isSymbolicLink()).toBe(true);
      expect(readlinkSync(join(destination, "src/link.ts"))).toBe("main.ts");
    }
    expect(summary.copied).toBeGreaterThanOrEqual(1);
  });
});

describe("Path.moveTo", () => {
  test("moves a file to the exact destination", async () => {
    const root = temporaryRoot();
    const source = join(root, "source.txt");
    const destination = join(root, "moved", "destination.txt");
    writeFileSync(source, "move me");

    const summary = await new Path(source).moveTo(new Path(destination));

    expect(readFileSync(destination, "utf8")).toBe("move me");
    expect(() => readFileSync(source)).toThrow();
    expect(summary.moved).toBe(1);
  });
});

describe("Path.transform", () => {
  test("applies transformers in order and aggregates per-file failures", async () => {
    const root = temporaryRoot();
    const source = join(root, "source");
    const destination = join(root, "destination");
    writeTree(source, {
      "a.txt": "a",
      "b.txt": "b",
      "fail.txt": "fail"
    });

    const summary = await new Path(source).transform(new Path(destination), {
      glob: "**/*.txt",
      concurrency: 2,
      transform: [
        (content) => `${content}-first`,
        (content, path) => {
          if (path.name === "fail.txt") {
            throw new Error("intentional transform failure");
          }
          return `${content}-second`;
        }
      ]
    });

    expect(readFileSync(join(destination, "a.txt"), "utf8")).toBe("a-first-second");
    expect(readFileSync(join(destination, "b.txt"), "utf8")).toBe("b-first-second");
    expect(() => readFileSync(join(destination, "fail.txt"))).toThrow();
    expect(summary.transformed).toBe(2);
    expect(summary.errors).toHaveLength(1);
    expect(summary.errors[0]?.path).toContain("fail.txt");
    expect(summary.errors[0]?.message).toContain("intentional transform failure");
  });
});
