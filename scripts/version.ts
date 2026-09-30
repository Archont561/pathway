/**
 * Print the version of pathway, from the one place it is written down.
 *
 * `[workspace.package] version` in the root Cargo.toml is the single source of
 * truth. Cargo ignores `version` in a member manifest, so every crate inherits
 * it and there is exactly one line to bump; the npm package and the docs site
 * both derive from it rather than restating it. A test in packages/path
 * asserts the npm package agrees.
 *
 * Used by `pixi run docs-build` / `docs-dev` / `docs-preview` (which export the
 * result as PATHWAY_VERSION for the site) and by apps/docs as its fallback.
 */
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

/**
 * Find the workspace manifest by walking up from `startDir`.
 *
 * Not `new URL("../Cargo.toml", import.meta.url)`: the docs site imports this
 * module, and Vite inlines it into `dist/.prerender/chunks/*.mjs`, at which
 * point `import.meta.url` points into the build output and the relative path
 * resolves to a Cargo.toml that does not exist. Anchoring on the working
 * directory survives bundling, and works whether the caller is a pixi task
 * (`cwd = "apps/docs"`), a bare `bun run build`, or this script run from the
 * repository root.
 *
 * The `[workspace.package]` test is what makes "nearest ancestor" safe: a
 * transitive dependency vendored under the repo would have a Cargo.toml with no
 * such table, and picking that one would silently produce nothing.
 */
export function workspaceManifestPath(startDir: string = process.cwd()): string {
  let dir = resolve(startDir);
  for (;;) {
    const candidate = join(dir, "Cargo.toml");
    if (existsSync(candidate)) {
      const manifest = readFileSync(candidate, "utf8");
      if (/^\[workspace\.package\]$/m.test(manifest)) {
        return candidate;
      }
    }
    const parent = dirname(dir);
    if (parent === dir) {
      throw new Error(`no Cargo.toml with a [workspace.package] table at or above ${startDir}`);
    }
    dir = parent;
  }
}

/** Read `[workspace.package] version` out of the workspace manifest. */
export function workspaceVersion(startDir: string = process.cwd()): string {
  const manifestPath = workspaceManifestPath(startDir);
  const manifest = readFileSync(manifestPath, "utf8");
  // Scoped to the table rather than the first `version` key in the file, since
  // `[workspace.dependencies]` and every member manifest carry their own. A line
  // scan rather than a TOML parser on purpose: this is a build-time detail of a
  // docs site, and one dependency to get right for one field is a bad trade.
  const table = manifest.match(/^\[workspace\.package\]$([\s\S]*?)^\[/m)?.[1];
  const version = table?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version) {
    throw new Error(`no [workspace.package] version in ${manifestPath}`);
  }
  return version;
}

if (import.meta.main) {
  process.stdout.write(workspaceVersion());
}
