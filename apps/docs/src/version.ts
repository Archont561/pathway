/**
 * The version these docs describe.
 *
 * One source: `[workspace.package] version` in the root `Cargo.toml`, read
 * through `scripts/version.ts`. That is the single place the version is
 * written down — every crate inherits it, and a test in `packages/path`
 * asserts the npm manifest agrees.
 *
 * There used to be a `PATHWAY_VERSION` environment variable in front of this,
 * exported by the pixi docs tasks. It was removed because it was redundant and
 * actively harmful: it resolved from *this same manifest*, so it could only
 * ever produce the same string, while making the build hash differently
 * depending on whether it happened to be set. Reading the manifest directly is
 * what the fallback already did on every build that went through turbo.
 *
 * Cache correctness now lives where it belongs, in `apps/docs/turbo.json`:
 * the root `Cargo.toml` and `scripts/version.ts` are declared build inputs, so
 * a version bump invalidates the docs build instead of serving a cached site
 * that quotes the old number.
 *
 * If the manifest cannot be read, this throws. A docs site that quietly says
 * `0.0.0` is worse than one that refuses to build.
 */
import { workspaceVersion } from "@workspace/version";

function resolveVersion(): string {
  try {
    return workspaceVersion();
  } catch (cause) {
    throw new Error(
      "pathway version unresolved: the workspace Cargo.toml could not be read. " +
        "Build the site from inside the repository (`pixi run docs-build`).",
      { cause }
    );
  }
}

export const version: string = resolveVersion();
