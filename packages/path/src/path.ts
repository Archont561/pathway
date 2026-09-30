/**
 * `Path` — the ergonomic surface. Scaffold.
 *
 * ## Why only string operations exist here
 *
 * Everything in this file is pure JavaScript, and that is decision D2 rather
 * than a staging artefact. Rust exists in this project for *large-scale*
 * filesystem operations: traversal, pruning, content hashing, parallel I/O.
 * Single-string path manipulation stays in TypeScript, because a boundary
 * crossing costs far more than the string operation it would wrap — so
 * `path.join("a").join("b").join("c")` crossing N-API three times is not a slow
 * path, it is an architecture that has given up.
 *
 * That is also why `pathe` and not `std::path` semantics: a JavaScript string
 * is a string, and normalising separators and drive letters is what makes
 * comparisons and template literals behave the same on every platform. The
 * Rust surface deliberately does the opposite and uses `std::path`, because
 * there a `PathBuf` already knows what it is.
 *
 * ## What is not here yet
 *
 * No file I/O, no `read`/`write`, no `contains`/`relativeTo`. The I/O methods
 * are specified in `.knowledge/implementation/code-ts-path.md`; they are absent
 * rather than stubbed so that nothing in the tree depends on a method that
 * throws. The one exception is `walk`, which has a stub of its own because its
 * *signature* is what the Step 0 NAPI-RS spike has to decide against.
 */

import * as pathe from "pathe";

/**
 * A filesystem path with object methods.
 *
 * Immutable: every method returns a new `Path` or a plain value, and nothing
 * mutates. That is what makes a `Path` usable as a `Map` key and safe to share
 * between concurrent walks.
 */
export class Path {
  /** The normalised, absolute path this instance refers to. */
  readonly value: string;

  constructor(value: string) {
    // Normalising in the constructor is what makes `value` comparable: without
    // it `/a/b`, `/a/./b` and `/a/c/../b` are three keys for one path.
    this.value = pathe.normalize(
      pathe.isAbsolute(value) ? value : pathe.join(process.cwd(), value)
    );
  }

  /** The current working directory as a `Path`. */
  static cwd(): Path {
    return new Path(process.cwd());
  }

  /** The directory containing this path. */
  get parent(): Path {
    return new Path(pathe.dirname(this.value));
  }

  /** The final component of this path. */
  get name(): string {
    return pathe.basename(this.value);
  }

  /** The final component without its extension. */
  get stem(): string {
    return pathe.basename(this.value, pathe.extname(this.value));
  }

  /** The extension, lowercase and including the dot, or `""` when there is none. */
  get ext(): string {
    return pathe.extname(this.value).toLowerCase();
  }

  /**
   * Join path segments onto this path.
   *
   * An absolute segment resets the path, which is `pathe.join`'s behaviour and
   * the same behaviour `node:path` has. Called out because it is the one place
   * the conventions are surprising, and silently resetting is not what most
   * callers mean.
   */
  join(...segments: string[]): Path {
    return new Path(pathe.join(this.value, ...segments));
  }

  /** This path as a plain string. */
  toString(): string {
    return this.value;
  }
}
