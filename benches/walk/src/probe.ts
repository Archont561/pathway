/**
 * The measurement layer, for everything `mitata` cannot measure.
 *
 * `mitata` is the wall-time authority: it owns the timing loop, the warm-up
 * threshold, GC between samples and its own statistics (`p50`, `p99`, and the
 * raw `samples` array that p95 is derived from), and with `inner_gc` it reports
 * `stats.gc`. Re-deriving those here would mean two definitions of p50 in one
 * report.
 *
 * What is left for this module is the three things `mitata` cannot know:
 *
 * - **time-to-first-entry** — from starting the iteration to the consumer
 *   holding its first usable result. This is the metric that separates a
 *   streaming walk from a collect-then-return one, and for the two
 *   array-returning baselines it is by construction equal to the total. Each
 *   subject reports it by calling `onFirst` at the moment its first result is
 *   in the consumer's hands.
 * - **peak heap** — see {@link ProbeMeasurement.peakHeapBytes} for why this is
 *   not read out of `mitata`.
 * - **cancellation cost** — pathway-only, because only pathway has a
 *   cancellation contract. Measured from `abort()` to the iterator settling,
 *   with the abort landing *during* the walk rather than before it starts.
 */

/** Heap samples per second. Fast enough to catch a spike, slow enough not to perturb. */
const HEAP_SAMPLE_HZ = 500;

export interface ProbeMeasurement {
  /** Median time from starting the iteration to the first usable result, ms. */
  readonly firstEntryP50Ms: number;
  /**
   * Highest `process.memoryUsage().heapUsed` seen while any iteration ran, bytes.
   *
   * Measured here rather than taken from `mitata`'s `stats.heap`, which is a
   * per-sample *delta* read after its own inner GC has already collected the
   * sample's garbage — so for a subject that allocates a fresh array per
   * iteration it reports zero, which is the opposite of the truth.
   */
  readonly peakHeapBytes: number;
  /** Entries the subject reported. */
  readonly entries: number;
  /** Whether every iteration agreed on the entry count. */
  readonly entriesStable: boolean;
}

/** The body under test: returns the entry count, calls `onFirst` at the first result. */
export type Probeable = (onFirst: () => void) => Promise<number>;

function percentile(sorted: readonly number[], p: number): number {
  if (sorted.length === 0) {
    return Number.NaN;
  }
  // Nearest-rank. A linear-interpolated percentile of three samples invents a
  // value no run produced, which is worse than reporting the nearest real one.
  const rank = Math.ceil((p / 100) * sorted.length);
  const index = Math.min(sorted.length - 1, Math.max(0, rank - 1));
  return sorted[index] as number;
}

/**
 * Run `body` and measure when its first result arrives, and how much heap it
 * held while doing so.
 *
 * Deliberately does *not* time the whole iteration — `mitata` does that, and
 * two clocks in one report is two chances to disagree.
 */
export async function probeStream(body: Probeable, samples: number): Promise<ProbeMeasurement> {
  const firstEntryTimes: number[] = [];
  const entryCounts: number[] = [];
  let peakHeapBytes = 0;

  const sampleHeap = (): void => {
    const used = process.memoryUsage().heapUsed;
    if (used > peakHeapBytes) {
      peakHeapBytes = used;
    }
  };

  // One untimed warm-up: the first iteration pays for JIT and for the page
  // cache being cold, and its time-to-first is not what a consumer experiences.
  await body(() => {});

  for (let i = 0; i < samples; i++) {
    let firstAt = Number.NaN;
    const sampler = setInterval(sampleHeap, Math.floor(1000 / HEAP_SAMPLE_HZ));
    const started = performance.now();
    try {
      const entries = await body(() => {
        sampleHeap();
        if (Number.isNaN(firstAt)) {
          firstAt = performance.now();
        }
      });
      sampleHeap();
      if (!Number.isNaN(firstAt)) {
        firstEntryTimes.push(firstAt - started);
      }
      entryCounts.push(entries);
    } finally {
      clearInterval(sampler);
    }
  }

  const first = entryCounts[0];
  return {
    firstEntryP50Ms: percentile(
      [...firstEntryTimes].sort((a, b) => a - b),
      50
    ),
    peakHeapBytes,
    entries: first ?? 0,
    entriesStable: entryCounts.every((count) => count === first)
  };
}

/** p95 from `mitata`'s raw samples. Nearest-rank, as in {@link percentile}. */
export function p95Of(samples: readonly number[]): number {
  return percentile(
    [...samples].sort((a, b) => a - b),
    95
  );
}

export interface CancellationMeasurement {
  /** Milliseconds from starting the iteration to the first batch. */
  readonly firstBatchMs: number;
  /** Milliseconds from `abort()` to the iterator settling. */
  readonly abortToSettleMs: number;
  /** Batches consumed before the abort landed. */
  readonly batchesBeforeAbort: number;
  /** Whether the iterator surfaced the abort reason rather than finishing. */
  readonly abortedCleanly: boolean;
}

/**
 * What cancellation costs.
 *
 * The abort is requested from inside the first batch's consumer, so it lands
 * while the engine is mid-walk. Cancelling a walk that has not begun is free,
 * and measuring that would measure nothing.
 */
export async function probeCancellation(
  start: (signal: AbortSignal, onFirstBatch: () => void) => AsyncIterable<unknown>
): Promise<CancellationMeasurement> {
  const controller = new AbortController();
  const reason = new Error("cancelled by the benchmark");
  const started = performance.now();
  let firstBatchMs = Number.NaN;
  let batches = 0;
  let abortedCleanly = false;
  let abortToSettleMs = Number.NaN;

  try {
    for await (const batch of start(controller.signal, () => {
      if (Number.isNaN(firstBatchMs)) {
        firstBatchMs = performance.now() - started;
        controller.abort(reason);
      }
      if (batch !== undefined) {
        batches++;
      }
    })) {
      // Draining is the point: stopping early would measure how fast it is to
      // stop not walking at all.
    }
  } catch (error) {
    // The generator surfaces `signal.reason` rather than yielding a partial
    // batch, so the throw is the contract working — but only if it is *our*
    // reason, and only if the abort had actually been requested.
    abortedCleanly = controller.signal.aborted && error === reason;
    abortToSettleMs = performance.now() - started - firstBatchMs;
    if (!controller.signal.aborted) {
      throw error;
    }
  }

  return {
    abortToSettleMs,
    firstBatchMs,
    batchesBeforeAbort: batches,
    abortedCleanly
  };
}
