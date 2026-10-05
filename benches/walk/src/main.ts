/**
 * The harness driver.
 *
 * Generates each tree size, runs every subject against it, times each with
 * `mitata`, probes each for time-to-first-entry and the entry count, checks
 * that every implementation saw the same number of files, measures
 * cancellation once per size, and writes two artefacts: a machine-readable JSON
 * file and the per-configuration Markdown tables the acceptance criteria ask
 * for.
 *
 * The entry-count cross-check is the important part. A walk that is fast
 * because it silently skipped two thirds of the tree is the single most likely
 * way this harness could report a win that does not exist, so a disagreement
 * between implementations is a hard error rather than a footnote.
 *
 * ## Which tool measures what
 *
 * `mitata` owns the clock. It does the warm-up, the GC between samples, the
 * statistics and the heap accounting, and it reports its own p50/p99 plus the
 * raw samples p95 is derived from. This module owns what only the caller can
 * know — when the first result became usable, and what cancelling costs — and
 * renders the report. See `probe.ts` for why the split is there.
 */

import { mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { walkFiles } from "@archont561/pathway";
import { measure } from "mitata";
import { p95Of, probeCancellation, probeStream } from "./probe.js";
import {
  baselineAvailability,
  type Scenario,
  type Subject,
  setRoot,
  subjectsFor
} from "./subjects.js";
import { buildTree, TREE_SIZES } from "./tree.js";

/** Where the trees live. Under `results/`, which is the turbo `bench` task's declared output. */
const WORK_ROOT = join(import.meta.dir, "..", "results");

/**
 * Read every file under `root` once and return the byte count.
 *
 * Deliberately built on `node:fs` rather than on any subject under test: it
 * runs before the measurements, so it can only affect the page cache, never a
 * number. Using `pathway` here would mean the thing being measured warms the
 * cache for the thing being compared against.
 */
async function readWholeTree(root: string): Promise<number> {
  const stack: string[] = [root];
  let bytes = 0;
  while (stack.length > 0) {
    const dir = stack.pop() as string;
    for (const entry of await readdir(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        stack.push(full);
      } else if (entry.isFile()) {
        bytes += (await readFile(full)).byteLength;
      }
    }
  }
  return bytes;
}

/**
 * The size of the addon the harness actually loaded, recorded in every report.
 *
 * The harness cannot ask the engine whether it is a debug or a release build,
 * but the two profiles differ by roughly an order of magnitude on linux-x64
 * (release ≈ 2.4 MiB, debug ≈ 15 MiB), and the methodology requires release:
 * the debug profile was measured to change the *ranking*, not just the margin
 * (245 ms vs 1,380 ms for the fused walk at 100k files). Both profiles write
 * the same `.node` path under `packages/path/dist/`, so whichever build
 * finished last is what this process loaded — a fact the report must carry,
 * because a number without it cannot be compared to the recorded baseline.
 *
 * The threshold is a loud warning, not a hard error: file sizes drift across
 * platforms and Rust versions, and a report that merely looks wrong is better
 * than a run that silently produces incomparable numbers.
 */
async function addonOnDisk(): Promise<{ name: string; sizeMiB: number } | null> {
  try {
    const distDir = dirname(fileURLToPath(import.meta.resolve("@archont561/pathway")));
    const entries = await readdir(distDir);
    const name = entries.find((entry) => entry.endsWith(".node"));
    if (name === undefined) {
      return null;
    }
    const info = await stat(join(distDir, name));
    return { name, sizeMiB: info.size / (1024 * 1024) };
  } catch {
    return null;
  }
}

const RESULTS_DIR = join(WORK_ROOT, "results");

interface Row {
  readonly implementation: string;
  readonly scenario: Scenario;
  readonly size: number;
  /** p50 wall time, milliseconds. From `mitata`. */
  readonly p50Ms: number;
  /** p95 wall time, milliseconds. Derived from `mitata`'s raw samples. */
  readonly p95Ms: number;
  /** Median time from start to the first usable result, milliseconds. */
  readonly firstEntryP50Ms: number;
  /** Mean forced-GC duration per sample, milliseconds. From `mitata`. */
  readonly gcMs: number;
  /** Peak heap during the run, bytes. From `mitata`. */
  readonly peakHeapBytes: number;
  /** Entries every iteration agreed on. */
  readonly entries: number;
}

/**
 * `mitata` sample budget and clock configuration.
 *
 * Bounded explicitly because the defaults are tuned for microbenchmarks: on a
 * 1M-file tree a single iteration is seconds of real I/O, and mitata's adaptive
 * sampling would run it dozens of times. Two samples is the floor at which a p95
 * is still a measurement rather than the maximum of two.
 *
 * `inner_gc` is what makes `mitata` emit `stats.gc` at all — without it, `gc`
 * only runs between samples and no GC statistics come back. `gc` is passed as a
 * function rather than `true` because `mitata`'s `true` means "call a global
 * `$gc`", which exists under `node --expose-gc` and nowhere else; `Bun.gc` is
 * the portable spelling.
 */
function mitataOptions(size: number): {
  min_samples: number;
  max_samples: number;
  warmup_threshold: number;
  samples_threshold: number;
  gc: () => void;
  inner_gc: boolean;
} {
  const big = size > 10_000;
  return {
    min_samples: big ? 2 : 5,
    max_samples: big ? 2 : 10,
    warmup_threshold: big ? 200 : 100,
    samples_threshold: big ? 400 : 200,
    gc: () => Bun.gc(true),
    inner_gc: true
  };
}

/**
 * `mitata` reports every duration in **nanoseconds** — verified against a
 * function pinned to 20 ms of `performance.now`, which it reported as
 * 20,017,282. Dividing by 1e6 is not a guess.
 */
const NS_PER_MS = 1e6;

function fmtMs(ms: number): string {
  return Number.isFinite(ms) ? ms.toFixed(1) : "n/a";
}

function fmtBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "n/a";
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

function markdownTable(rows: readonly Row[]): string {
  return [
    "| Implementation | p50 ms | p95 ms | first entry ms | GC ms | peak heap | entries |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
    ...rows.map(
      (row) =>
        `| ${row.implementation} | ${fmtMs(row.p50Ms)} | ${fmtMs(row.p95Ms)} | ` +
        `${fmtMs(row.firstEntryP50Ms)} | ${fmtMs(row.gcMs)} | ${fmtBytes(row.peakHeapBytes)} | ` +
        `${row.entries.toLocaleString("en-US")} |`
    )
  ].join("\n");
}

/**
 * Pathway's speedup against the strongest alternative in the same scenario.
 *
 * "≥5x faster than the best alternative" has to be measured against the *best*
 * competitor in the same scenario, not the mean of all of them: comparing
 * against the slowest baseline is the standard way a benchmark makes its subject
 * look good.
 */
function speedupAgainst(rows: readonly Row[]): { speedup: number; best: Row } | undefined {
  const subject = rows.find((row) => row.implementation === "pathway");
  if (subject === undefined) {
    return undefined;
  }
  const competitors = rows.filter((row) => row.implementation !== "pathway");
  const best = competitors.reduce<Row | undefined>(
    (fastest, row) => (fastest === undefined || row.p50Ms < fastest.p50Ms ? row : fastest),
    undefined
  );
  if (best === undefined || !(best.p50Ms > 0)) {
    return undefined;
  }
  return { speedup: best.p50Ms / subject.p50Ms, best };
}

/** The claim the acceptance criteria assert. */
const CLAIMED_SPEEDUP = 5;

/**
 * The verdict line for a scenario.
 *
 * Only `c-fused` gets a pass/fail verdict, because the ≥5x claim is a claim
 * about the *fused* walk. Reporting "the claim does not hold" under the raw
 * traversal would be incoherent — pathway losing the raw race is not a failure
 * of the fused claim, it is a separate fact that gets its own sentence.
 */
function verdict(scenario: Scenario, rows: readonly Row[]): string {
  const measured = speedupAgainst(rows);
  if (measured === undefined) {
    return (
      "_No usable comparison: no competitor ran, or the fastest alternative reported a " +
      "non-positive p50._"
    );
  }
  const { speedup, best } = measured;
  const ratio = `${speedup.toFixed(2)}x`;
  const comparison =
    `Against \`${best.implementation}\`, the fastest alternative here: p50 ` +
    `${fmtMs(best.p50Ms)} ms against pathway's ${fmtMs(rows[0]?.p50Ms ?? Number.NaN)} ms.`;

  if (scenario !== "c-fused") {
    return (
      `**${ratio}** against \`${best.implementation}\`. ${comparison} ` +
      (speedup < 1
        ? "Pathway is behind here; this scenario is not what the >=5x fused claim is about."
        : "Pathway is ahead here, but this scenario does no fused work to claim credit for.")
    );
  }
  if (speedup >= CLAIMED_SPEEDUP) {
    return `**${ratio}** faster than \`${best.implementation}\`. ${comparison} The >=5x claim **holds**.`;
  }
  // "x short" is read as a subtraction and is not a speedup; report the ratio
  // actually measured and the ratio the claim needs, so neither can be misread.
  return (
    `**${ratio}** faster than \`${best.implementation}\`. ${comparison} ` +
    `The >=5x claim does **not** hold: the measured speedup is ${ratio}, ` +
    `and the claim needs ${CLAIMED_SPEEDUP}x — ${(CLAIMED_SPEEDUP / speedup).toFixed(2)}x more.`
  );
}

async function measureSubject(subject: Subject, size: number, samples: number): Promise<Row> {
  process.stdout.write(`  ${subject.scenario} ${subject.implementation}… `);

  const probed = await probeStream(subject.run, samples);
  if (!probed.entriesStable) {
    throw new Error(
      `${subject.implementation}/${subject.scenario} returned a varying entry count; ` +
        "its iterations are not comparable"
    );
  }

  const stats = await measure(async () => await subject.run(() => {}), mitataOptions(size));

  process.stdout.write(
    `p50 ${fmtMs(stats.p50 / NS_PER_MS)} ms, first ${fmtMs(probed.firstEntryP50Ms)} ms, ` +
      `${probed.entries.toLocaleString("en-US")} entries\n`
  );

  return {
    implementation: subject.implementation,
    scenario: subject.scenario,
    size,
    p50Ms: stats.p50 / NS_PER_MS,
    p95Ms: p95Of(stats.samples) / NS_PER_MS,
    firstEntryP50Ms: probed.firstEntryP50Ms,
    gcMs: (stats.gc?.avg ?? Number.NaN) / NS_PER_MS,
    peakHeapBytes: probed.peakHeapBytes,
    entries: probed.entries
  };
}

/**
 * Which tree sizes to run.
 *
 * Defaults to all four, because the >=5x claim is about 100k+ files and a 10k
 * result does not support it. `PATHWAY_BENCH_SIZES=10000,100000` narrows it,
 * which is what makes the harness usable on a laptop mid-task: generating a
 * million files takes minutes and several gigabytes, and nobody runs that on
 * every iteration of a change they are making to the harness itself.
 */
function sizesToRun(): readonly number[] {
  const override = process.env.PATHWAY_BENCH_SIZES;
  if (override === undefined || override.trim() === "") {
    return TREE_SIZES;
  }
  const parsed = override
    .split(",")
    .map((part) => Number.parseInt(part.trim(), 10))
    .filter((value) => Number.isSafeInteger(value) && value > 0);
  if (parsed.length === 0) {
    throw new Error(`PATHWAY_BENCH_SIZES was set to ${JSON.stringify(override)} but held no sizes`);
  }
  return parsed;
}

async function main(): Promise<void> {
  const sizes = sizesToRun();
  const scenarios: readonly Scenario[] = ["a-raw", "b-exclusions", "c-fused"];
  const allRows: Row[] = [];
  const markdown: string[] = [];
  const addon = await addonOnDisk();

  markdown.push(
    "# Fused-walk benchmark",
    "",
    `Generated by \`pixi run bench\` on Bun \`${Bun.version}\` (Node target \`${process.versions.node}\`) ` +
      `on \`${process.platform}-${process.arch}\`.`,
    ...(addon !== null
      ? [
          "",
          `Addon on disk: \`${addon.name}\`, ${addon.sizeMiB.toFixed(1)} MiB` +
            (addon.sizeMiB > 8
              ? " — **this looks like a DEBUG build; the methodology requires the release addon, so these numbers are not comparable to the recorded baseline**"
              : " (release profile)") +
            "."
        ]
      : []),
    "",
    // A partial run cannot support a claim about 100k+ files, so the report says
    // so at the top rather than leaving a reader to notice the missing sizes.
    ...(sizes.length === TREE_SIZES.length
      ? []
      : [
          `> **Partial run.** Measured: ${sizes.map((s) => s.toLocaleString("en-US")).join(", ")}. ` +
            `Not measured: ${TREE_SIZES.filter((s) => !sizes.includes(s))
              .map((s) => s.toLocaleString("en-US"))
              .join(", ")}. The >=5x claim is about 100k+ files, so a verdict from a ` +
            "partial run does not speak to it.",
          ""
        ]),
    "Baselines required by the acceptance criteria, and whether this runtime provided them:",
    "",
    ...Object.entries(baselineAvailability()).map(
      ([name, available]) =>
        `- \`${name}\`: ${available ? "available" : "**absent in this runtime**"}`
    ),
    ""
  );

  for (const size of sizes) {
    const treeRoot = join(WORK_ROOT, `tree-${size}`);
    process.stdout.write(`\n▸ generating ${size.toLocaleString("en-US")} files…\n`);
    const written = await buildTree(treeRoot, size);
    if (written !== size) {
      throw new Error(`the generator wrote ${written} files, asked for ${size}`);
    }
    // Read the whole tree once before measuring anything.
    //
    // Without this the harness is a coin flip. Generating 100k files leaves
    // hundreds of megabytes of dirty pages, and the first subject that reads
    // *content* then pays for the generator's writeback — which is how one run
    // put the fused walk at 29.2 s and the next at 1.3 s for identical work.
    // Scenarios A and B only read directory entries, so they looked stable
    // throughout and hid the cause.
    //
    // This defines the measurement as warm-cache steady state, which is the
    // regime the claim is about: a build tool walking a tree it has just
    // written, or re-walking one it has walked before.
    process.stdout.write("▸ settling the page cache…\n");
    const settleStart = performance.now();
    const settledBytes = await readWholeTree(treeRoot);
    process.stdout.write(
      `  read ${settledBytes.toLocaleString("en-US")} bytes in ${fmtMs(performance.now() - settleStart)} ms\n`
    );

    for (const scenario of scenarios) {
      setRoot(treeRoot);
      const subjects = subjectsFor(scenario);
      const rows: Row[] = [];
      const samples = size > 10_000 ? 2 : 3;

      for (const subject of subjects) {
        rows.push(await measureSubject(subject, size, samples));
      }

      // Every implementation must have seen the same tree. A disagreement means
      // one exclusion set is not equivalent and the whole table is void.
      const counts = new Set(rows.map((row) => row.entries));
      if (counts.size > 1) {
        throw new Error(
          `${scenario} at ${size.toLocaleString("en-US")} files: implementations disagreed on ` +
            `the file count (${[...counts].join(" vs ")}); the comparison is not like-for-like`
        );
      }

      allRows.push(...rows);
      markdown.push(
        `## ${size.toLocaleString("en-US")} files — ${scenario}`,
        "",
        markdownTable(rows),
        "",
        verdict(scenario, rows),
        ""
      );
    }

    // Cancellation is pathway-only and measured once per tree size.
    setRoot(treeRoot);
    const cancellation = await probeCancellation((signal, onFirstBatch) => {
      let batches = 0;
      return (async function* stream(): AsyncGenerator<unknown> {
        for await (const batch of walkFiles(treeRoot, {
          dot: true,
          gitignore: false,
          signal
        })) {
          batches++;
          onFirstBatch();
          yield batch;
          if (batches > 4) {
            return;
          }
        }
      })();
    });
    markdown.push(
      `## ${size.toLocaleString("en-US")} files — cancellation (pathway only)`,
      "",
      `- First batch: ${fmtMs(cancellation.firstBatchMs)} ms`,
      `- Abort to settle: ${fmtMs(cancellation.abortToSettleMs)} ms ` +
        `(${cancellation.abortedCleanly ? "surfaced the abort reason" : "**did not surface the reason**"})`,
      `- Batches consumed before the abort: ${cancellation.batchesBeforeAbort}`,
      ""
    );
  }

  await mkdir(RESULTS_DIR, { recursive: true });
  await writeFile(join(RESULTS_DIR, "results.json"), `${JSON.stringify(allRows, null, 2)}\n`);
  await writeFile(join(RESULTS_DIR, "report.md"), `${markdown.join("\n")}\n`);

  process.stdout.write(`\n▸ wrote ${RESULTS_DIR}/results.json and ${RESULTS_DIR}/report.md\n`);
}

await main();
