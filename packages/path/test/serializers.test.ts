import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { FileSystem, json, Path, type Serializer } from "../src/index.js";

interface Config {
  readonly name: string;
  readonly enabled: boolean;
}

const roots: string[] = [];

function temporaryRoot(): string {
  const root = mkdtempSync(join(tmpdir(), "pathway-serializer-"));
  roots.push(root);
  return root;
}

afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe("json serializer", () => {
  test("round-trips a typed value through Path", async () => {
    const file = new Path(join(temporaryRoot(), "config.json"));
    const config: Config = { name: "pathway", enabled: true };

    await file.write(json, config);
    const result = await file.read<Config>(json);

    expect(result).toEqual(config);
  });

  test("resolves JSON automatically on a default Path", async () => {
    const file = new Path(join(temporaryRoot(), "config.json"));
    const config: Config = { name: "pathway", enabled: true };

    await file.write(json, config);
    const result = await file.read();

    expect(result).toEqual(config);
  });

  test("rejects malformed JSON", async () => {
    const file = new Path(join(temporaryRoot(), "broken.json"));
    await file.writeText("{ broken");

    expect(file.read(json)).rejects.toThrow();
  });
});

describe("per-FileSystem serializer registries", () => {
  const tagged: Serializer<string> = {
    name: "tagged",
    native: false,
    extensions: [".cfg"],
    parse(bytes) {
      return new TextDecoder().decode(bytes).replace(/^value:/, "");
    },
    stringify(value) {
      return new TextEncoder().encode(`value:${value}`);
    }
  };

  test("maps extensions and preserves the mapping through Path operations", async () => {
    const root = temporaryRoot();
    const fileSystem = FileSystem.create({ serializers: [json, tagged] });
    const file = fileSystem.path(root).join("settings.cfg");

    await file.write(tagged, "from the registry");
    const result = await file.parent.join(file.name).read();

    expect(result).toBe("from the registry");
  });

  test("does not share registrations between instances", async () => {
    const root = temporaryRoot();
    const configured = FileSystem.create({ serializers: [tagged] });
    const isolated = FileSystem.create({ serializers: [json] });
    const file = configured.path(root).join("settings.cfg");

    await file.write(tagged, "private mapping");

    await expect(isolated.path(file.value).read()).rejects.toThrow(".cfg");
    await expect(configured.path(file.value).read()).resolves.toBe("private mapping");
  });

  test("allows explicit extension registration", async () => {
    const root = temporaryRoot();
    const fileSystem = FileSystem.create();
    const file = fileSystem.path(root).join("settings.data");

    fileSystem.register(tagged, ["data"]);
    await file.write(tagged, "registered");

    await expect(file.read()).resolves.toBe("registered");
  });
});

describe("atomic writes", () => {
  test("writes through a same-directory temporary file and leaves no temporary file", async () => {
    const root = temporaryRoot();
    const file = new Path(join(root, "config.json"));

    await file.write(json, { version: 1 }, { atomic: true, fsync: true });

    expect(await file.read<{ version: number }>(json)).toEqual({ version: 1 });
    expect(readdirSync(root)).toEqual(["config.json"]);
  });
});
