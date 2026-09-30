/**
 * The version these docs describe.
 *
 * Two sources, in order:
 *
 * 1. `PATHWAY_VERSION` from the environment. This is the path the pixi tasks
 *    use (`pixi run docs-build`), and it exists because the version of what you
 *    are reading about should be decided by the environment you built in, not
 *    restated in the site. `apps/docs` never computes it: it is handed it.
 * 2. The root `Cargo.toml`. The fallback is not laziness, it is the reason the
 *    first source can be trusted: `bun run build` from the workspace root goes
 *    through turbo, and nothing would otherwise guarantee the variable survived
 *    that trip. Reading the same single source of truth directly means a build
 *    started outside pixi shows the real version instead of a placeholder.
 *
 * If neither resolves, this throws. A docs site that quietly says `0.0.0` is
 * worse than one that refuses to build.
 */
import { workspaceVersion } from "@workspace/version";

function resolveVersion(): string {
  const fromEnv = process.env.PATHWAY_VERSION?.trim();
  if (fromEnv) {
    return fromEnv;
  }
  try {
    return workspaceVersion();
  } catch {
    throw new Error(
      "pathway version unresolved: PATHWAY_VERSION is unset and the workspace " +
        "Cargo.toml could not be read. Run the docs through `pixi run docs-build`."
    );
  }
}

export const version: string = resolveVersion();
