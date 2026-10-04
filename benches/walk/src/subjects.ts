/**
 * The subjects: what is being compared, and on what terms.
 *
 * Every subject enumerates the same set of files under the same root, so the
 * harness can time them against each other. Three rules keep that comparison
 * honest, and all three are easy to get wrong:
 *
 * 1. **The file set is identical.** pathway's walk defaults to honouring
 *    `.gitignore` and to skipping dotfiles; `fdir`, `tinyglobby` and
 *    `node:fs.glob` do neither by default. Left alone, pathway would be
 *    measured on a *smaller* tree than its competitors and would win for free.
 *    So every pathway subject sets `gitignore: false, dot: true`.
 * 2. **Narrowing to files counts.** pathway narrows to files inside Rust,
 *    before the batch crosses the boundary. `node:fs.glob` on Bun 1.3 yields
 *    directories as bare strings and has no `withFileTypes`, so its narrowing
 *    is a JavaScript predicate per entry. That asymmetry is not a thumb on the
 *    scale — it is exactly the cost the fused walk claims to remove — but it is
 *    written down here so nobody has to infer it from a number.
 * 3. **Benchmark C composites, it does not cheat.** The baselines do not get to
 *    return bare paths and be compared against a walk that also hashed
 *    everything. Every implementation in scenario C pays for `stat` and a
 *    content hash; pathway simply pays for them in one pass instead of three.
 */

import { createHash } from "node:crypto";
import { glob as nodeGlob, readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import { walkFiles } from "@archont561/pathway";
import { fdir } from "fdir";
import { glob as tinyglobby } from "tinyglobby";

/**
 * Benchmark B's exclusion set.
 *
 * Shaped like a real "give me the source files" query rather than a single
 * `node_modules` skip, because the single-skip case is the one every walker
 * handles and the multi-shape case is the one that separates them: eight
 * directory names to prune plus three file globs to reject.
 */
export const EXCLUDED_DIRS: readonly string[] = [
  "node_modules",
  "dist",
  "build",
  "target",
  "vendor",
  ".git",
  ".cache",
  "coverage"
] as const;

export const EXCLUDED_FILE_GLOBS: readonly string[] = ["**/*.test.ts", "**/*.spec.js", "**/*.md"];

/** Shared by every pathway subject, for the reason in the file header. */
// `withMetadata: false` is load-bearing, not a default. The N-API binding
// defaults it to `true`, which makes the walker stat every entry. Leaving it on
// gave scenario A a per-file `stat` syscall that fdir, tinyglobby and
// `node:fs.glob` never perform, and had it stayed, the raw-traversal numbers
// would have measured pathway's `stat` rather than its traversal.
// `gitignore: false` matches the baselines: none of them reads `.gitignore`.
const PATHWAY_BASE = { dot: true, gitignore: false, withMetadata: false } as const;

export type Scenario = "a-raw" | "b-exclusions" | "c-fused";

export type Implementation =
  | "pathway"
  | "node-glob"
  | "fdir"
  | "tinyglobby"
  | "bun-glob"
  | "fdir-pool";

export interface Subject {
  readonly implementation: Implementation;
  readonly scenario: Scenario;
  /**
   * One iteration. Must consume the whole stream — a lazy subject must not look
   * fast by never doing the work.
   *
   * `onFirst` is called at the moment the caller could first act on a result.
   * For a streaming walk that is the first batch to arrive; for a subject that
   * collects the whole tree before returning, it is after it returns, because
   * there is no earlier moment at which the caller has anything. That
   * asymmetry is the time-to-first-entry metric, so it has to be the subject's
   * own judgement rather than something the harness guesses.
   */
  run(onFirst: () => void): Promise<number>;
}

/** The root every subject closes over. Set by the harness before each scenario. */
let root = "";

export function setRoot(next: string): void {
  root = next;
}

/**
 * `stat` plus a SHA-256 of the contents, as two separate awaits.
 *
 * Deliberately sequential and un-batched. A composite that fanned out with
 * `Promise.all` would be faster than the fused walk on a 2-core box for reasons
 * that have nothing to do with fusion, and the honest version of this baseline
 * is the one an application actually writes.
 */
async function statAndHash(path: string): Promise<void> {
  const info = await stat(path);
  const bytes = await readFile(path);
  createHash("sha256").update(bytes).digest("hex");
  // Touch `size` so the stat cannot be optimised into a liveness check.
  if (info.size < 0) {
    throw new Error("unreachable");
  }
}

/**
 * Whether a relative path is a file, judged from the string alone.
 *
 * The generated corpus gives every file an extension and every directory none,
 * so this is exact here and costs one `lastIndexOf`. Using `stat` instead would
 * fold a syscall per entry into the node baseline, and the comparison would end
 * up about `stat` twice rather than about the walker.
 */
function looksLikeFile(value: string): boolean {
  return value.lastIndexOf(".") > value.lastIndexOf("/");
}

// `node:fs.glob`, narrowed to files.
//
// Note the line comments, not a block one: this file talks about glob patterns,
// and the sequence `*/` that ends every block comment also occurs inside a
// `**`-prefixed glob. A block comment holding one closes early and turns the
// rest of the paragraph into code.
//
// `exclude` takes an array of globs, and on Bun 1.3 a directory pattern such as
// `**` + `/dist` prunes the directory and everything under it — verified rather
// than assumed, because getting this wrong is exactly what the harness's
// entry-count cross-check exists to catch.
async function* nodeGlobFiles(exclude?: readonly string[]): AsyncGenerator<string> {
  for await (const entry of nodeGlob("**/*", {
    cwd: root,
    ...(exclude !== undefined && { exclude: [...exclude] })
  })) {
    const value = entry as string;
    if (looksLikeFile(value)) {
      yield value;
    }
  }
}

/**
 * Drain a stream, counting as it goes.
 *
 * `onFirst` fires on the first item rather than after the loop, so a streaming
 * subject reports the moment its first result was usable instead of the moment
 * its last one was.
 */
async function countOf(stream: AsyncIterable<unknown>, onFirst: () => void): Promise<number> {
  let count = 0;
  for await (const item of stream) {
    if (count === 0) {
      onFirst();
    }
    if (item !== undefined) {
      count++;
    }
  }
  return count;
}

function pathway(scenario: Scenario): Subject {
  return {
    implementation: "pathway",
    scenario,
    async run(onFirst) {
      const iter =
        scenario === "a-raw"
          ? walkFiles(root, { ...PATHWAY_BASE })
          : scenario === "b-exclusions"
            ? walkFiles(root, {
                ...PATHWAY_BASE,
                exclude: [...EXCLUDED_DIRS],
                // `!` negation is globby-style and the engine implements it: a
                // negated pattern is a `GlobSet` of its own, checked before the
                // positives. See crates/core/src/walk/matcher.rs.
                glob: EXCLUDED_FILE_GLOBS.map((pattern) => `!${pattern}`)
              })
            : walkFiles(root, { ...PATHWAY_BASE, hash: "blake3", withMetadata: true });
      let count = 0;
      for await (const batch of iter) {
        if (count === 0) {
          onFirst();
        }
        count += batch.length;
      }
      return count;
    }
  };
}

function nodeGlobSubject(scenario: Scenario): Subject {
  return {
    implementation: "node-glob",
    scenario,
    async run(onFirst) {
      if (scenario === "c-fused") {
        let count = 0;
        for await (const relative of nodeGlobFiles()) {
          if (count === 0) {
            onFirst();
          }
          await statAndHash(join(root, relative));
          count++;
        }
        return count;
      }
      const exclusions =
        scenario === "b-exclusions"
          ? // Both halves of the exclusion set: the directory patterns prune,
            // the file globs reject. Passing only the first is what made the
            // first run of this harness report 4,731 files against everyone
            // else's 3,949 — which the cross-check caught.
            [...EXCLUDED_DIRS.map((dir) => `**/${dir}`), ...EXCLUDED_FILE_GLOBS]
          : undefined;
      return await countOf(nodeGlobFiles(exclusions), onFirst);
    }
  };
}

function fdirSubject(scenario: Scenario): Subject {
  return {
    implementation: "fdir",
    scenario,
    async run(onFirst) {
      const builder = new fdir().withFullPaths();
      if (scenario === "b-exclusions") {
        builder
          .exclude((path) => EXCLUDED_DIRS.includes(path.split("/").pop() ?? ""))
          .filter((path) => !/\.(test\.ts|spec\.js|md)$/.test(path));
      }
      // `withPromise` collects the whole tree into an array before resolving, so
      // the first result the caller could act on only exists after it returns.
      // Calling `onFirst` here rather than pretending it streams is the point.
      const paths = await builder.crawl(root).withPromise();
      onFirst();
      if (scenario !== "c-fused") {
        return paths.length;
      }
      for (const path of paths) {
        await statAndHash(path);
      }
      return paths.length;
    }
  };
}

function tinyglobbySubject(scenario: Scenario): Subject {
  return {
    implementation: "tinyglobby",
    scenario,
    async run(onFirst) {
      const ignore =
        scenario === "b-exclusions"
          ? [...EXCLUDED_DIRS.map((dir) => `**/${dir}/**`), ...EXCLUDED_FILE_GLOBS]
          : undefined;
      // Also collect-then-return, like fdir: the promise resolves with the whole
      // array, so there is no first result before it settles.
      const paths = await tinyglobby(["**/*"], {
        cwd: root,
        dot: true,
        ...(ignore !== undefined && { ignore })
      });
      onFirst();
      if (scenario !== "c-fused") {
        return paths.length;
      }
      for (const path of paths) {
        await statAndHash(join(root, path));
      }
      return paths.length;
    }
  };
}

// `Bun.Glob.scan`, reached through a cast because the typings and the runtime
// disagree.
//
// The scan API is an *instance* method — `new Bun.Glob(pattern).scan(opts)` —
// and there is no static `Bun.Glob.scan` on any released Bun (probed on
// 1.3.11-canary.1 and 1.4.2), which is why the first revision of this subject
// never activated and the task believed the API did not exist on 1.3.
//
// The options `GlobScanOptions` accepts are `cwd`, `dot`, `absolute`,
// `followSymlinks`, `throwErrorOnBrokenSymlink` and `onlyFiles` — there is no
// `ignore`/`exclude`. Verified empirically on both runtimes rather than assumed:
// every ignore pattern shape tried (`[asterisk][asterisk]/dist/…`, `dist/…`,
// `[asterisk][asterisk]/*.md`, absolute variants, `exclude`) left the entry
// count untouched, and the bun.com reference for Glob.scan lists no such
// option. Scenario B therefore filters in JavaScript over the full stream —
// the same shape as fdir's exclude/filter callbacks — and that asymmetry is
// recorded with the results: the bun-glob B row measures "scan everything,
// then filter", not exclusion pruning.
function bunGlobScanner(): { scan(options: unknown): AsyncGenerator<string> } | null {
  const Glob = (Bun as unknown as { Glob?: new (pattern: string) => unknown }).Glob;
  if (typeof Glob !== "function") {
    return null;
  }
  try {
    const instance = new (Glob as new (pattern: string) => unknown)("**/*") as {
      scan?: unknown;
    };
    return typeof instance.scan === "function"
      ? (instance as unknown as { scan(options: unknown): AsyncGenerator<string> })
      : null;
  } catch {
    return null;
  }
}

// The directory half of benchmark B's exclusion set, as path segments.
const EXCLUDED_DIR_SEGMENTS: ReadonlySet<string> = new Set(EXCLUDED_DIRS);

/**
 * Benchmark B's exclusion set as a JavaScript predicate over relative paths.
 *
 * Only used by the subject whose native API cannot express exclusions — see
 * `bunGlobScanner` for why that is a documented fact about `Bun.Glob.scan`,
 * not a wiring shortcut.
 */
function notExcluded(relative: string): boolean {
  const segments = relative.split("/");
  for (let i = 0; i < segments.length - 1; i += 1) {
    if (EXCLUDED_DIR_SEGMENTS.has(segments[i] as string)) {
      return false;
    }
  }
  return !/\.(test\.ts|spec\.js|md)$/.test(relative);
}

function bunGlob(scenario: Scenario): Subject | null {
  const globber = bunGlobScanner();
  if (globber === null) {
    return null;
  }
  return {
    implementation: "bun-glob",
    scenario,
    async run(onFirst) {
      // `onlyFiles: true` is the documented default; stated explicitly so a
      // future runtime default change cannot silently change the file set.
      // scan() yields relative paths, files only — verified on both runtimes.
      const stream = globber.scan({ cwd: root, dot: true, onlyFiles: true });
      if (scenario === "c-fused") {
        let count = 0;
        for await (const relative of stream) {
          if (count === 0) {
            onFirst();
          }
          await statAndHash(join(root, relative));
          count++;
        }
        return count;
      }
      if (scenario === "b-exclusions") {
        let count = 0;
        for await (const relative of stream) {
          if (notExcluded(relative)) {
            if (count === 0) {
              onFirst();
            }
            count++;
          }
        }
        return count;
      }
      return await countOf(stream, onFirst);
    }
  };
}

/**
 * Bounded-concurrency stat + hash over paths already collected.
 *
 * This is the baseline that decides the verdict. A plain
 * `for (const p of paths) await statAndHash(p)` loop serialises every round
 * trip, so it measures pathway's internal parallelism against a single-threaded
 * consumer — which is not the claim being made. Pathway does the work in one
 * pass over its own traversal; the honest comparison is against a consumer that
 * keeps the disk busy too.
 *
 * The limit is a fixed pool rather than unbounded `Promise.all`, because
 * fanning out 1M concurrent reads would measure the allocator.
 */
async function statAndHashPooled(paths: readonly string[], limit: number): Promise<number> {
  let next = 0;
  let done = 0;
  // `next++` is synchronous, so a single thread hands out each index exactly
  // once and the workers never need a lock.
  const workers = Array.from({ length: Math.min(limit, paths.length) }, async () => {
    for (;;) {
      const path = paths[next++];
      if (path === undefined) {
        return;
      }
      await statAndHash(path);
      done++;
    }
  });
  await Promise.all(workers);
  return done;
}

/** Pool width for the concurrent baselines. */
const POOL_WIDTH = 32;

/**
 * The strongest alternative: fdir's crawl (the fastest of the three crawlers
 * measured above) feeding a 32-wide stat + hash pool.
 */
function fdirPoolSubject(scenario: Scenario): Subject | null {
  if (scenario !== "c-fused") {
    return null;
  }
  return {
    implementation: "fdir-pool",
    scenario,
    async run(onFirst) {
      const paths = await new fdir().withFullPaths().crawl(root).withPromise();
      onFirst();
      return await statAndHashPooled(paths, POOL_WIDTH);
    }
  };
}

/** Every subject for one scenario, minus what this runtime cannot provide. */
export function subjectsFor(scenario: Scenario): Subject[] {
  const subjects: Subject[] = [
    pathway(scenario),
    nodeGlobSubject(scenario),
    fdirSubject(scenario),
    tinyglobbySubject(scenario)
  ];
  const pooled = fdirPoolSubject(scenario);
  if (pooled !== null) {
    subjects.push(pooled);
  }
  const bun = bunGlob(scenario);
  if (bun !== null) {
    subjects.push(bun);
  }
  return subjects;
}

/** What the acceptance criteria require as baselines, and what this runtime has. */
export function baselineAvailability(): Record<string, boolean> {
  return {
    "node:fs.glob": true,
    fdir: true,
    tinyglobby: true,
    "Bun.Glob.scan": bunGlobScanner() !== null,
    [`fdir + ${POOL_WIDTH}-wide stat/hash pool`]: true
  };
}
