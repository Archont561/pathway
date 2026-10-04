import { copyFile, mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { Path } from "@archont561/pathway";

const fileCount = Number(process.env.PATHWAY_COPY_FILES ?? 50_000);
const targetSpeedup = 3;

async function createTree(root: string): Promise<void> {
  for (let index = 0; index < fileCount; index += 1) {
    const path = join(root, `dir-${index % 100}`, `file-${index}.txt`);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, "copy benchmark payload\n");
  }
}

async function copySequential(source: string, destination: string): Promise<void> {
  await mkdir(destination, { recursive: true });
  for (const entry of await readdir(source, { withFileTypes: true })) {
    const from = join(source, entry.name);
    const to = join(destination, entry.name);
    if (entry.isDirectory()) {
      await copySequential(from, to);
    } else {
      await copyFile(from, to);
    }
  }
}

const root = await mkdtemp(join(tmpdir(), "pathway-copy-bench-"));
try {
  const source = join(root, "source");
  const pathwayDestination = join(root, "pathway");
  const sequentialDestination = join(root, "sequential");
  await mkdir(source);
  await createTree(source);

  const pathwayStart = performance.now();
  const result = await new Path(source).copyTo(new Path(pathwayDestination));
  const pathwayMs = performance.now() - pathwayStart;

  const sequentialStart = performance.now();
  await copySequential(source, sequentialDestination);
  const sequentialMs = performance.now() - sequentialStart;
  const speedup = sequentialMs / pathwayMs;

  const report = {
    files: fileCount,
    pathwayMs: Math.round(pathwayMs),
    sequentialMs: Math.round(sequentialMs),
    speedup: Number(speedup.toFixed(2)),
    targetSpeedup,
    targetMet: speedup >= targetSpeedup,
    copied: result.copied,
    errors: result.errors.length
  };
  await mkdir("results", { recursive: true });
  await writeFile("results/copy.json", JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
} finally {
  await rm(root, { recursive: true, force: true });
}
