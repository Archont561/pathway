/**
 * The binding loader's front end.
 *
 * ## Why this file is a diagnostic and not a loader
 *
 * The native build is configured with `--no-js --dts` and writes the
 * platform-specific `.node` file plus its declaration file under `dist/`.
 * This front end keeps the runtime checks and lazy loading in one place,
 * without maintaining a platform list in generated JavaScript.
 *
 * A hand-written platform `require` list is forbidden in this repository. The
 * 2025 draft was `require("@archont561/pathway")` from inside the package that
 * `@archont561/pathway` *is* — a circular self-require, because the binary lives in the
 * platform packages (`@archont561/pathway-linux-x64-gnu` and friends) and never in the
 * root one. The failure is not a crash: it is a resolution that works in
 * development, where a stale `dist/` sits next to a symlink, and fails on a
 * clean install.
 *
 * So this module does the one thing a hand-written file can do honestly —
 * explain what is missing and how to build it — and defers to the generated
 * loader the moment there is something to load.
 */

import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import type { PathEntry } from "./types.js";

/**
 * `require`, reconstructed for ESM. A bare `require(file)` only *looks*
 * portable from a Bun test run — Bun injects `require` into ES modules, Node
 * does not, so the untestable-under-Bun failure mode was Node throwing at the
 * first real `loadEngine()` call. (Caught by the task-3 Node smoke run.)
 */
const requireAddon = createRequire(import.meta.url);

/**
 * How to build the addon, spelled out for the error message. `build-native` is
 * a debug build (~40s cold, seconds warm through turbo); the release variant
 * is for benchmarks and the publish pipeline.
 */
const BUILD_HINT = [
  "The native engine is not built. From the repository root:",
  "",
  "  pixi run build-native",
  "",
  "Or build the crates directly (needs the C toolchain for blake3):",
  "",
  "  pixi run -e default -- cargo build -p pathway-fs-engine --release"
].join("\n");

/**
 * The addon's platform-specific filename, e.g. `pathway.linux-x64-gnu.node`.
 *
 * `pathway` is the `napi.name` declared in this package's manifest — the same
 * field `napi build` reads to name the artifact — so the loader and the
 * builder agree on it through one config rather than two constants.
 *
 * The platform suffix is NAPI-RS's npm spelling (`linux-x64-gnu`,
 * `darwin-arm64`, `win32-x64-msvc`), *not* the rust triple: composing
 * `${arch}-unknown-linux-gnu` produces a filename the CLI never writes. The
 * alternatives — a hardcoded table of every triple, or a `require` of a name
 * that does not exist — both fail the same way and neither says why.
 */
function addonFileName(platform: string, arch: string): string {
  switch (platform) {
    case "linux": {
      // `glibcVersionRuntime` is present only on glibc; its absence is the
      // musl (Alpine) detection the napi-rs ecosystem itself uses.
      const report = process.report?.getReport() as
        | { header?: { glibcVersionRuntime?: string } }
        | undefined;
      const libc = report?.header?.glibcVersionRuntime !== undefined ? "gnu" : "musl";
      return `pathway.linux-${arch}-${libc}.node`;
    }
    case "darwin":
      return `pathway.darwin-${arch}.node`;
    case "win32":
      return `pathway.win32-${arch}-msvc.node`;
    default:
      throw new Error(`unsupported platform for the native engine: ${platform}-${arch}`);
  }
}

/** The package root, used for manifest/version validation. */
const PACKAGE_ROOT = join(import.meta.dirname, "..");
/** The native build output configured by `packages/path/package.json`. */
const NATIVE_DIR = join(PACKAGE_ROOT, "dist");

/**
 * The options object the native `Walker` constructor decodes.
 *
 * Field-for-field what `crates/engine/src/walk.rs` declares as
 * `WalkerOptions`; the engine applies the defaults, so absence here means
 * "the engine decides", never a second default table on this side.
 */
export interface NativeWalkerOptions {
  readonly glob?: readonly string[];
  readonly regex?: string;
  readonly exclude?: readonly string[];
  readonly dot?: boolean;
  readonly gitignore?: boolean;
  readonly absolute?: boolean;
  readonly maxDepth?: number;
  readonly filesOnly?: boolean;
  readonly withMetadata?: boolean;
  readonly hash?: string;
  readonly batchSize?: number;
  readonly concurrency?: number;
}

/**
 * The paged walk handle (`crates/engine/src/walk.rs`, transport frozen by
 * task-1): construct, `scan()` once, `nextBatch()` until empty, `cancel()`
 * anytime. The entries come back already shaped as {@link PathEntry} —
 * that is a deliberate contract between the two files, so the generator in
 * `walk.ts` yields them without a per-entry re-mapping pass.
 */
export interface NativeWalker {
  scan(): Promise<number>;
  nextBatch(): Promise<PathEntry[]>;
  cancel(): void;
  errors(): string[];
}

/**
 * The options object the native `TempDir` constructor decodes.
 *
 * Field-for-field what `crates/engine/src/temp.rs` declares as
 * `NativeTempOptions`; `dir` on the public surface is spelled `parent` here
 * because that is the word the Rust side and `tempfile` use.
 */
export interface NativeTempOptions {
  readonly prefix?: string;
  readonly suffix?: string;
  readonly parent?: string;
}

/**
 * A native temp-directory guard.
 *
 * `remove()` and `keep()` consume it: the Rust side holds the `TempDir` in an
 * `Option` and takes it, so a second call fails instead of deleting a
 * directory the handle no longer owns.
 */
export interface NativeTempDir {
  /** The directory's absolute path. */
  path(): string;
  /** Remove the tree now. */
  remove(): void;
  /** Disarm cleanup and return the path. */
  keep(): string;
}

/** The symbols the generated loader is expected to export. */
export interface NativeEngine {
  /** The engine crate's version, used to refuse a stale `.node` file. */
  engineVersion(): string;
  /** The Node-API version the addon was compiled against. */
  napiVersion(): number;
  /** The paged walk, one instance per `walk()` call. */
  Walker: new (
    root: string,
    options?: NativeWalkerOptions
  ) => NativeWalker;
  hashFileNative(path: string, algorithm: string): string;
  hashBytesNative(bytes: Buffer, algorithm: string): string;
  /** The temp-directory guard; one instance per temp directory. */
  TempDir: new (
    options?: NativeTempOptions
  ) => NativeTempDir;
  /** Removes every live temp directory; called from the `exit` flush. */
  flushTempDirs(): number;
}

let cached: NativeEngine | null = null;

/**
 * Load the native engine, or explain why it is not there.
 *
 * Throws rather than returning `null`: every caller needs the engine, so a
 * nullable return would push a null check into code that has nothing to do if
 * it fails. The message is the useful part — a bare
 * `Cannot find module ... .node` sends the reader looking for a dependency
 * problem instead of a build step.
 */
export function loadEngine(): NativeEngine {
  if (cached !== null) {
    return cached;
  }

  const file = join(NATIVE_DIR, addonFileName(process.platform, process.arch));
  if (!existsSync(file)) {
    throw new Error(`Cannot find the native engine at ${file}.\n\n${BUILD_HINT}`);
  }

  // Required lazily and through a non-literal specifier: a literal
  // `import "./pathway.linux-x64-gnu.node"` would have to name this host's
  // platform at build time, which is the hand-written list again.
  const loaded = requireAddon(file) as Partial<NativeEngine>;
  if (typeof loaded.engineVersion !== "function" || typeof loaded.napiVersion !== "function") {
    throw new Error(
      `The native engine at ${file} is missing engineVersion()/napiVersion(). ` +
        "It was probably built from a different revision — rebuild it."
    );
  }

  // The two version guards, in order of how misleading the failure would
  // otherwise be.
  //
  // A stale `.node` from an earlier checkout is refused by comparing the
  // engine's compiled-in version against this package's manifest — the two
  // are the same workspace version by construction (a test asserts it), so
  // an inequality can only mean the binary predates the sources around it.
  const engineVersion = loaded.engineVersion();
  const packageVersion = (
    JSON.parse(readFileSync(join(PACKAGE_ROOT, "package.json"), "utf8")) as { version: string }
  ).version;
  if (engineVersion !== packageVersion) {
    throw new Error(
      `The native engine at ${file} is version ${engineVersion}, but this package is ` +
        `${packageVersion} — a stale build.\n\n${BUILD_HINT}`
    );
  }

  // The Node-API floor: the addon declares the napi version it was compiled
  // against, and a runtime older than that floor would otherwise fail with a
  // missing-symbol error at the first call instead of a sentence at import.
  const requiredNapi = loaded.napiVersion();
  const runtimeNapi = Number(process.versions.napi ?? NaN);
  if (!Number.isNaN(runtimeNapi) && runtimeNapi < requiredNapi) {
    throw new Error(
      `The native engine needs Node-API ${requiredNapi}, but this runtime provides ` +
        `${runtimeNapi}. Upgrade the runtime (engines.node in package.json is the floor).`
    );
  }

  if (typeof loaded.Walker !== "function") {
    throw new Error(
      `The native engine at ${file} is missing the Walker class. ` +
        "It was probably built from a different revision — rebuild it."
    );
  }

  // Temp directories are the other half of the loader's contract: the exit
  // flush that `temp.ts` installs calls `flushTempDirs()` from a bare
  // `process.on("exit")` callback, where a missing symbol would surface as an
  // unexplained TypeError at exit rather than as a message here.
  if (typeof loaded.TempDir !== "function" || typeof loaded.flushTempDirs !== "function") {
    throw new Error(
      `The native engine at ${file} is missing the TempDir class or flushTempDirs(). ` +
        "It was probably built from a different revision — rebuild it."
    );
  }

  cached = loaded as NativeEngine;
  return cached;
}

/** Whether a built addon is present, for callers that want to degrade rather than throw. */
export function engineAvailable(): boolean {
  return existsSync(join(NATIVE_DIR, addonFileName(process.platform, process.arch)));
}
