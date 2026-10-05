import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { engineAvailable, Path, walkFiles } from "../dist/index.js";

assert.equal(engineAvailable(), true, "the built package must expose its native addon");
assert.equal(new Path("/a/./b/../c").value, "/a/c");

const root = mkdtempSync(join(tmpdir(), "pathway-node-smoke-"));
try {
  writeFileSync(join(root, "hello.txt"), "hello from Node");
  const entries = [];
  for await (const batch of walkFiles(root)) {
    entries.push(...batch);
  }

  assert.deepEqual(
    entries.map((entry) => entry.value),
    ["hello.txt"]
  );
} finally {
  rmSync(root, { recursive: true, force: true });
}
