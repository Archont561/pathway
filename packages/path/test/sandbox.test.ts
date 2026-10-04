import { afterEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { ContainmentError, FileSystem } from "../src/index.js";

const roots: string[] = [];

function temporaryRoot(name = "root"): string {
  const parent = mkdtempSync(join(process.cwd(), "pathway-sandbox-"));
  const root = join(parent, name);
  mkdirSync(root);
  roots.push(parent);
  return root;
}

function sandboxAt(root = temporaryRoot()): ReturnType<FileSystem["sandbox"]> {
  return FileSystem.create().sandbox(root);
}

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe("FileSystem sandbox", () => {
  test("requires an existing directory root", () => {
    const parent = mkdtempSync(join(process.cwd(), "pathway-sandbox-"));
    roots.push(parent);
    const file = join(parent, "not-a-directory");
    writeFileSync(file, "file");

    expect(() => FileSystem.create().sandbox(file)).toThrow();
  });

  test("allows the root and paths below it", () => {
    const sandbox = sandboxAt();
    const child = sandbox.resolve("nested/config.json");

    expect(sandbox.resolve(".").value).toBe(sandbox.root.value);
    expect(child.value).toBe(join(sandbox.root.value, "nested/config.json"));
    expect(child.parent.value).toBe(join(sandbox.root.value, "nested"));
  });

  test("blocks traversal and prefix-collision escapes", () => {
    const root = temporaryRoot("public");
    const sandbox = sandboxAt(root);

    expect(() => sandbox.resolve("../outside")).toThrow(ContainmentError);
    expect(() => sandbox.resolve(join(dirname(root), "public-evil", "file.txt"))).toThrow(
      ContainmentError
    );
    expect(() => sandbox.resolve("nested/../../outside")).toThrow(ContainmentError);
  });

  test("blocks an intermediate symlink that points outside the root", () => {
    if (process.platform === "win32") return;
    const root = temporaryRoot();
    const outside = mkdtempSync(join(process.cwd(), "pathway-sandbox-outside-"));
    roots.push(outside);
    writeFileSync(join(outside, "secret.txt"), "secret");
    symlinkSync(outside, join(root, "link"));
    const sandbox = sandboxAt(root);

    expect(() => sandbox.resolve("link/secret.txt")).toThrow(ContainmentError);
  });

  test("blocks symlink loops and broken symlinks", () => {
    if (process.platform === "win32") return;
    const root = temporaryRoot();
    symlinkSync("loop", join(root, "loop"));
    symlinkSync("missing.txt", join(root, "broken"));
    const sandbox = sandboxAt(root);

    expect(() => sandbox.resolve("loop/file.txt")).toThrow(ContainmentError);
    expect(() => sandbox.resolve("broken")).toThrow(ContainmentError);
  });

  test("revalidates a sandbox path before reading after a symlink replacement", async () => {
    if (process.platform === "win32") return;
    const root = temporaryRoot();
    const outside = mkdtempSync(join(process.cwd(), "pathway-sandbox-outside-"));
    roots.push(outside);
    const target = join(root, "target.txt");
    const outsideTarget = join(outside, "target.txt");
    writeFileSync(target, "inside");
    writeFileSync(outsideTarget, "outside");
    const sandbox = sandboxAt(root);
    const safePath = sandbox.resolve("target.txt");

    rmSync(target);
    symlinkSync(outsideTarget, target);

    await expect(safePath.readText()).rejects.toBeInstanceOf(ContainmentError);
  });

  test("handles host case sensitivity instead of trusting a string prefix", () => {
    const root = temporaryRoot("Public");
    const alternate = join(dirname(root), basename(root).toLowerCase(), "file.txt");
    const sandbox = sandboxAt(root);
    const resolveAlternate = () => sandbox.resolve(alternate);

    if (process.platform === "win32" || process.platform === "darwin") {
      expect(resolveAlternate).not.toThrow();
    } else {
      expect(resolveAlternate).toThrow(ContainmentError);
    }
  });

  test("handles Unicode normalization according to the host filesystem", () => {
    const root = temporaryRoot("café");
    const decomposed = join(dirname(root), "cafe\u0301", "file.txt");
    const sandbox = sandboxAt(root);
    const resolveDecomposed = () => sandbox.resolve(decomposed);

    if (process.platform === "darwin") {
      expect(resolveDecomposed).not.toThrow();
    } else {
      expect(resolveDecomposed).toThrow(ContainmentError);
    }
  });
});
