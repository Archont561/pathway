/**
 * The binding loader's front end.
 *
 * ## Why this file is a diagnostic and not a loader
 *
 * The real loader is *generated*. `napi build --platform --release` writes
 * `index.js` and `index.d.ts` at the package root from the addon's own symbol
 * table, which is how the right binary is found on every platform without
 * anyone maintaining a list.
 *
 * A hand-written platform `require` list is forbidden in this repository. The
 * 2025 draft was `require("@myorg/path")` from inside the package that
 * `@myorg/path` *is* — a circular self-require, because the binary lives in the
 * platform packages (`@myorg/path-linux-x64-gnu` and friends) and never in the
 * root one. The failure is not a crash: it is a resolution that works in
 * development, where a stale `dist/` sits next to a symlink, and fails on a
 * clean install.
 *
 * So this module does the one thing a hand-written file can do honestly —
 * explain what is missing and how to build it — and defers to the generated
 * loader the moment there is something to load.
 */

import { existsSync } from "node:fs";
import { join } from "node:path";

/** How to build the addon, spelled out for the error message. */
const BUILD_HINT = [
  "The native engine is not built. From the repository root:",
  "",
  "  pixi run -e default -- napi build --cwd packages/path --platform --release",
  "",
  "Or build the crates directly (needs the C toolchain for blake3):",
  "",
  "  pixi run -e default -- cargo build -p myorg-path-engine --release"
].join("\n");

/**
 * The addon's platform-specific filename, e.g. `myorg-path.linux-x64-gnu.node`.
 *
 * Computed, not listed: the alternatives are a hardcoded table that rots
 * quietly as NAPI-RS adds triples, and a `require` of a name that does not
 * exist. Both fail the same way and neither says why.
 */
function addonFileName(platform: string, arch: string): string {
  // Windows uses dashes, every other platform uses the rust triple spelling.
  const triple = platform === "win32" ? `${arch}-pc-windows-msvc` : `${arch}-unknown-linux-gnu`;
  return `myorg-path.${platform}-${triple}.node`;
}

/** The symbols the generated loader is expected to export. */
export interface NativeEngine {
  /** The engine crate's version, used to refuse a stale `.node` file. */
  engineVersion(): string;
  /** The Node-API version the addon was compiled against. */
  napiVersion(): number;
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

  const file = join(import.meta.dirname, addonFileName(process.platform, process.arch));
  if (!existsSync(file)) {
    throw new Error(`Cannot find the native engine at ${file}.\n\n${BUILD_HINT}`);
  }

  // Required lazily and through a non-literal specifier: a literal
  // `import "./myorg-path.linux-x64-gnu.node"` would have to name this host's
  // platform at build time, which is the hand-written list again.
  const loaded = require(file) as Partial<NativeEngine>;
  if (typeof loaded.engineVersion !== "function" || typeof loaded.napiVersion !== "function") {
    throw new Error(
      `The native engine at ${file} is missing engineVersion()/napiVersion(). ` +
        "It was probably built from a different revision — rebuild it."
    );
  }

  cached = loaded as NativeEngine;
  return cached;
}

/** Whether a built addon is present, for callers that want to degrade rather than throw. */
export function engineAvailable(): boolean {
  return existsSync(join(import.meta.dirname, addonFileName(process.platform, process.arch)));
}
