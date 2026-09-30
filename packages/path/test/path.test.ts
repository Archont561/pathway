import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { engineAvailable, Path } from "../src/index.js";

const repoRoot = new URL("../../../", import.meta.url);
// The published artefact is this package's own manifest, not the private
// workspace root's, so the checks below read it rather than the root.
const packageManifest = new URL("../package.json", import.meta.url);

function workspaceVersion(): string {
  const manifest = readFileSync(fileURLToPath(new URL("Cargo.toml", repoRoot)), "utf8");
  // Scoped to the table, not the first `version = "…"` in the file: the
  // workspace dependencies and every crate manifest carry their own.
  const table = manifest.match(/^\[workspace\.package\]$([\s\S]*?)^\[/m)?.[1];
  const version = table?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version) {
    throw new Error("no [workspace.package] version in the workspace manifest");
  }
  return version;
}

describe("Path", () => {
  test("normalises so one path is one value", () => {
    expect(new Path("/a/b").value).toBe(new Path("/a/./c/../b").value);
  });

  test("resolves a relative path against the working directory", () => {
    expect(new Path(".").value).toBe(Path.cwd().value);
  });

  test("joins and normalises", () => {
    expect(new Path("/a").join("b", "../c").value).toBe("/a/c");
  });

  test("exposes name, stem and ext", () => {
    const p = new Path("/a/b/file.tar.gz");
    expect([p.parent.value, p.name, p.stem, p.ext]).toEqual([
      "/a/b",
      "file.tar.gz",
      "file.tar",
      ".gz"
    ]);
  });
});

describe("engine", () => {
  test("reports the addon as absent rather than throwing at import", () => {
    // The addon is not built in a fresh clone, so the honest answer is false.
    // The point of the test is that *asking* is safe: importing the package must
    // not require a build step, or `bun test` cannot run before `napi build`.
    expect(typeof engineAvailable()).toBe("boolean");
  });
});

describe("version", () => {
  const pkg = JSON.parse(readFileSync(fileURLToPath(packageManifest), "utf8")) as {
    version: string;
    license?: string;
  };

  test("the npm package and the Rust workspace share one version", () => {
    // The published artefacts are the two halves of one product, so a version
    // that says otherwise is a support ticket waiting to happen. Cargo is the
    // source of truth (`[workspace.package] version`); this test is what stops
    // package.json from being bumped on its own.
    expect(pkg.version).toBe(workspaceVersion());
  });

  test("the package is licensed", () => {
    // `publishConfig.access` is public, so a missing license field would ship a
    // package that npm renders as "UNLICENSED" — and a consumer's legal review
    // of a filesystem library starts with the licence.
    expect(pkg.license).toBe("MIT");
  });
});
