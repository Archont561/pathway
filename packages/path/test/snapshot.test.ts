import { afterEach, describe, expect, test } from "bun:test";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { FileSystem, Path, Snapshot, SnapshotFormatError } from "../src/index.js";

/**
 * Builds a throwaway tree; every test's setup is one call.
 *
 * The returned root is `realpath`-resolved, because a snapshot's root is the
 * canonical one — on macOS the system temp directory is `/var/folders/...`,
 * a symlink to `/private/var/folders/...`, so an un-resolved fixture path
 * would make every path assertion here a Linux-only assertion.
 */
const roots: string[] = [];
function tree(files: Record<string, string>): string {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "pathway-snapshot-")));
  roots.push(root);
  for (const [file, contents] of Object.entries(files)) {
    const path = join(root, file);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, contents);
  }
  return root;
}

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe("snapshot", () => {
  test("captures a tree as sorted, populated entries", async () => {
    const root = tree({ "a.ts": "a", "src/b.ts": "bbb" });

    const snapshot = await new Path(root).snapshot();

    expect([...snapshot.entries.keys()]).toEqual(["a.ts", "src/b.ts"]);
    expect(snapshot.entries.get("a.ts")?.size).toBe(1);
    expect(snapshot.entries.get("src/b.ts")?.size).toBe(3);
    expect(snapshot.root).toBe(root);
    expect(snapshot.takenAtNanos).toBeGreaterThan(0n);
  });

  test("walk options select what the snapshot covers", async () => {
    const root = tree({ "a.ts": "x", "a.md": "x", "node_modules/dep.ts": "x" });

    const snapshot = await new Path(root).snapshot({
      glob: ["**/*.ts"],
      exclude: ["node_modules"]
    });

    expect([...snapshot.entries.keys()]).toEqual(["a.ts"]);
  });

  test("diff sorts every path into exactly one bucket, as Paths", async () => {
    const root = tree({ "kept.ts": "same", "gone.ts": "bye", "changed.ts": "before" });
    const before = await new Path(root).snapshot({ hash: "blake3" });

    rmSync(join(root, "gone.ts"));
    writeFileSync(join(root, "changed.ts"), "after!");
    writeFileSync(join(root, "fresh.ts"), "new");
    const after = await new Path(root).snapshot({ hash: "blake3" });

    const diff = before.diff(after);

    expect(diff.added.map((path) => path.value)).toEqual([join(root, "fresh.ts")]);
    expect(diff.removed.map((path) => path.value)).toEqual([join(root, "gone.ts")]);
    expect(diff.modified.map((path) => path.value)).toEqual([join(root, "changed.ts")]);
    expect(diff.unchanged.map((path) => path.value)).toEqual([join(root, "kept.ts")]);
    expect(diff.added[0]).toBeInstanceOf(Path);
    expect(diff.hasChanges).toBe(true);
  });

  test("a hashed diff calls a rewrite with identical bytes unchanged", async () => {
    const root = tree({ "a.ts": "stable" });
    const before = await new Path(root).snapshot({ hash: "blake3" });

    writeFileSync(join(root, "a.ts"), "stable");
    const after = await new Path(root).snapshot({ hash: "blake3" });

    const diff = before.diff(after);
    expect(diff.unchanged.map((path) => path.value)).toEqual([join(root, "a.ts")]);
    expect(diff.hasChanges).toBe(false);
  });

  test("save writes the persisted document and load restores it", async () => {
    const root = tree({ "a.ts": "a", "b.ts": "b" });
    const snapshot = await new Path(root).snapshot({ hash: "blake3" });
    const file = new Path(join(root, ".cache", "snapshot.json"));

    await snapshot.save(file);
    const restored = await Snapshot.load(file);

    expect([...restored.entries.keys()]).toEqual([...snapshot.entries.keys()]);
    expect(restored.diff(snapshot).hasChanges).toBe(false);

    const document = JSON.parse(readFileSync(file.value, "utf8")) as {
      format: string;
      entries: Array<{ path: string; modifiedNanos: string }>;
    };
    expect(document.format).toBe("pathway-snapshot-v1");
    expect(document.entries.map((entry) => entry.path)).toEqual(["a.ts", "b.ts"]);
    expect(typeof document.entries[0]?.modifiedNanos).toBe("string");
  });

  test("mtime keeps nanosecond precision across the boundary and the file", async () => {
    // A JS number would round 19-digit nanoseconds; the surface exposes BigInt
    // and the document stores a decimal string, so no digit is invented or lost.
    const root = tree({ "a.ts": "a" });
    const snapshot = await new Path(root).snapshot();
    const nanos = snapshot.entries.get("a.ts")?.modifiedNanos;

    expect(typeof nanos).toBe("bigint");
    const file = new Path(join(root, "snapshot.json"));
    await snapshot.save(file);

    expect((await Snapshot.load(file)).entries.get("a.ts")?.modifiedNanos).toBe(nanos as bigint);
  });

  test("mtimes differing only below the millisecond are modified, not unchanged", () => {
    const document = (nanos: string): string =>
      JSON.stringify({
        format: "pathway-snapshot-v1",
        root: "/tmp/pathway-ns",
        takenAtNanos: nanos,
        entries: [{ path: "a.ts", size: 3, modifiedNanos: nanos }]
      });

    const before = Snapshot.parse(document("1700000000123000001"));
    const after = Snapshot.parse(document("1700000000123999999"));

    expect(before.diff(after).modified.map((path) => path.value)).toEqual([
      join("/tmp/pathway-ns", "a.ts")
    ]);
  });

  test("a document of another format is refused by name", () => {
    const foreign = JSON.stringify({
      format: "pathway-snapshot-v0",
      root: "/tmp/x",
      takenAtNanos: "1",
      entries: []
    });

    expect(() => Snapshot.parse(foreign)).toThrow(SnapshotFormatError);
    expect(() => Snapshot.parse(foreign)).toThrow(/pathway-snapshot-v0/);
  });

  test("the root is canonical, so a symlinked root resolves to its target", async () => {
    const root = tree({ "inner/a.ts": "a" });
    const link = join(root, "link");
    symlinkSync(join(root, "inner"), link);

    const snapshot = await new Path(link).snapshot();

    expect(snapshot.root).toBe(join(root, "inner"));
    expect([...snapshot.entries.keys()]).toEqual(["a.ts"]);
  });

  test("FileSystem#snapshot hands back Paths bound to that view", async () => {
    const root = tree({ "a.ts": "a" });
    const view = FileSystem.create();

    const snapshot = await view.snapshot(root);
    rmSync(join(root, "a.ts"));
    const diff = snapshot.diff(await view.snapshot(root));

    expect(diff.removed.map((path) => path.value)).toEqual([join(root, "a.ts")]);
  });
});
