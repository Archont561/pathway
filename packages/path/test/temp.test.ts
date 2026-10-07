import { afterEach, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";
import { FileSystem, Path, tempDir } from "../src/index.js";

/** Probe roots, removed even when a test fails. */
const roots: string[] = [];
function probeRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "pathway-temp-"));
  roots.push(root);
  return root;
}

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe("tempDir", () => {
  test("creates a directory in the requested parent with the requested name", async () => {
    const root = probeRoot();

    const dir = await tempDir({ prefix: "pathway-", suffix: "-scratch", dir: root });

    expect(dir.path.startsWith(root)).toBe(true);
    const name = basename(dir.path);
    expect(name.startsWith("pathway-")).toBe(true);
    expect(name.endsWith("-scratch")).toBe(true);
    expect(existsSync(dir.path)).toBe(true);
  });

  test("remove deletes a non-empty tree, and the handle reports the path no longer", async () => {
    const root = probeRoot();
    const dir = await tempDir({ dir: root });
    writeFileSync(join(dir.path, "scratch.txt"), "contents");

    await dir.remove();

    expect(existsSync(dir.path)).toBe(false);
    await expect(dir.remove()).rejects.toThrow();
  });

  test("keep hands the path over and disarms cleanup", async () => {
    const root = probeRoot();
    const dir = await tempDir({ dir: root });

    const kept = await dir.keep();

    expect(kept).toBe(dir.path);
    expect(existsSync(kept)).toBe(true);
  });
});

describe("Path.temp", () => {
  test("scopes a directory to the callback and removes it afterwards", async () => {
    const root = probeRoot();
    let scratch = "";

    await Path.temp({ dir: root }, async (dir) => {
      scratch = dir.value;
      expect(dir).toBeInstanceOf(Path);
      await dir.join("result.txt").writeText("contents");
      expect(existsSync(dir.join("result.txt").value)).toBe(true);
    });

    expect(existsSync(scratch)).toBe(false);
  });

  test("removes the directory when the callback throws, and rethrows", async () => {
    const root = probeRoot();
    let scratch = "";

    const failure = await Path.temp({ dir: root }, async (dir) => {
      scratch = dir.value;
      throw new Error("build failed");
    }).then(
      () => null,
      (error: unknown) => error
    );

    expect(failure).toBeInstanceOf(Error);
    expect((failure as Error).message).toBe("build failed");
    expect(existsSync(scratch)).toBe(false);
  });

  test("FileSystem.temp scopes the callback to that view's serializers", async () => {
    const view = FileSystem.create();
    const root = probeRoot();

    await view.temp({ dir: root }, async (dir) => {
      writeFileSync(join(dir.value, "config.json"), '{"port":3000}');
      expect(await dir.join("config.json").read()).toEqual({ port: 3000 });
    });
  });
});

describe("cleanup guarantee", () => {
  // The exit paths, as observed rather than as promised: the hook the surface
  // installs covers `process.exit(0)`; a `SIGKILL`, and a `SIGINT` under the
  // default disposition, run no JavaScript at all and are documented as tier 3.
  // The child scripts are TypeScript, so this runs under the Bun suite's own
  // runtime; the core pins the same mechanism without a JavaScript runtime.
  const bunOnly = typeof (globalThis as { Bun?: unknown }).Bun === "undefined";

  function runProbe(mode: "exit" | "sigkill" | "sigint"): {
    root: string;
    status: number | null;
    signal: NodeJS.Signals | null;
    entries: string[];
  } {
    const root = probeRoot();
    const entry = fileURLToPath(new URL("../src/index.js", import.meta.url));
    const script = join(root, "probe.ts");
    const ending =
      mode === "exit"
        ? "process.exit(0);"
        : `process.kill(process.pid, "${mode === "sigkill" ? "SIGKILL" : "SIGINT"}");`;
    writeFileSync(
      script,
      [
        'import { writeFileSync } from "node:fs";',
        'import { join } from "node:path";',
        `import { tempDir } from ${JSON.stringify(entry)};`,
        "const dir = await tempDir({ dir: process.env.PATHWAY_TEMP_PROBE_ROOT });",
        'writeFileSync(join(dir.path, "marker.txt"), "contents");',
        ending
      ].join("\n")
    );
    const result = spawnSync(process.execPath, [script], {
      env: {
        ...process.env,
        PATHWAY_TEMP_PROBE_ROOT: root
      }
    });
    return {
      root,
      status: result.status,
      signal: result.signal,
      entries: readdirSync(root).filter((name) => name !== "probe.ts")
    };
  }

  test.skipIf(bunOnly)("a clean process.exit() leaves nothing behind", () => {
    const probe = runProbe("exit");

    expect(probe.status).toBe(0);
    expect(probe.entries).toEqual([]);
  });

  test.skipIf(bunOnly || process.platform === "win32")(
    "SIGKILL leaves the directory and its contents behind (tier 3)",
    () => {
      const probe = runProbe("sigkill");

      expect(probe.signal).toBe("SIGKILL");
      expect(probe.entries.length).toBe(1);
      const survivor = join(probe.root, probe.entries[0] ?? "");
      expect(existsSync(join(survivor, "marker.txt"))).toBe(true);
    }
  );

  test.skipIf(bunOnly || process.platform === "win32")(
    "SIGINT under the default disposition leaves the directory behind (tier 3)",
    () => {
      const probe = runProbe("sigint");

      expect(probe.signal).toBe("SIGINT");
      expect(probe.entries.length).toBe(1);
    }
  );
});
