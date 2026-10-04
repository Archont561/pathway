/**
 * A filesystem view confined to one existing directory.
 *
 * The sandbox rejects lexical escapes and resolves the deepest existing path
 * component before returning a path. That catches intermediate symlink escapes,
 * loops, and broken links without relying on a fragile string-prefix check.
 *
 * This is a path-resolution guard, not an openat-backed capability handle:
 * another process can still replace a path component after validation and
 * before a later filesystem operation. Callers needing race-free containment
 * must use the native descriptor-based read primitive instead of Node's
 * ordinary path-based I/O.
 */

import { lstatSync, realpathSync, statSync } from "node:fs";
import * as pathe from "pathe";
import { Path } from "./path.js";
import type { SerializerRegistry } from "./serializers/registry.js";

/** An attempted path resolution escaped its sandbox root. */
export class ContainmentError extends Error {
  /** The path that was rejected. */
  readonly path: string;

  /** The sandbox root the path was required to remain inside. */
  readonly root: string;

  constructor(path: string, root: string, reason: string) {
    super(`Path "${path}" escapes sandbox root "${root}": ${reason}`);
    this.name = "ContainmentError";
    this.path = path;
    this.root = root;
  }
}

function comparisonPath(value: string): string {
  const normalized = value.normalize("NFC");
  return process.platform === "win32" || process.platform === "darwin"
    ? normalized.toLocaleLowerCase("en-US")
    : normalized;
}

function isWithin(root: string, candidate: string): boolean {
  const relative = pathe.relative(comparisonPath(root), comparisonPath(candidate));
  return relative === "" || (!relative.startsWith("..") && !pathe.isAbsolute(relative));
}

function missingPath(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    ((error as { code?: unknown }).code === "ENOENT" ||
      (error as { code?: unknown }).code === "ENOTDIR")
  );
}

/** Finds the deepest existing component, retaining symlinks for realpath. */
function deepestExisting(path: string): string {
  let current = path;
  for (;;) {
    try {
      lstatSync(current);
      return current;
    } catch (error) {
      if (!missingPath(error)) {
        throw error;
      }
      const parent = pathe.dirname(current);
      if (parent === current) {
        return current;
      }
      current = parent;
    }
  }
}

/**
 * Validates one sandbox path.
 *
 * Existing ancestors are resolved as a unit so an intermediate symlink to an
 * outside directory cannot pass merely because the textual path has a safe
 * prefix. Non-existing final components are allowed for writes when their
 * existing ancestor is safe.
 */
function assertContained(root: string, candidate: string): void {
  if (!isWithin(root, candidate)) {
    throw new ContainmentError(candidate, root, "lexical traversal or prefix collision");
  }

  let ancestor: string;
  try {
    ancestor = deepestExisting(candidate);
  } catch (error) {
    throw new ContainmentError(
      candidate,
      root,
      `cannot inspect an existing component: ${String(error)}`
    );
  }
  let resolved: string;
  try {
    resolved = realpathSync(ancestor);
  } catch (error) {
    throw new ContainmentError(
      candidate,
      root,
      `cannot resolve an existing component: ${String(error)}`
    );
  }

  if (!isWithin(root, resolved)) {
    throw new ContainmentError(candidate, root, "an existing component resolves through a symlink");
  }
}

/** A `Path` whose derivations remain inside one sandbox root. */
export class SandboxPath extends Path {
  /** The canonical sandbox root. */
  readonly root: string;

  /** @internal Constructed by `Sandbox`; callers use `sandbox.resolve()` or `sandbox.join()`. */
  constructor(value: string, root: string, serializers: SerializerRegistry) {
    super(value, serializers);
    this.root = root;
    assertContained(this.root, this.value);
  }

  /** The parent path, rejected if it would leave the sandbox. */
  override get parent(): SandboxPath {
    return new SandboxPath(pathe.dirname(this.value), this.root, this.serializers);
  }

  /** Join segments while preserving the sandbox guarantee. */
  override join(...segments: string[]): SandboxPath {
    return new SandboxPath(pathe.resolve(this.value, ...segments), this.root, this.serializers);
  }

  /** Resolve a user-supplied path relative to this path. */
  resolve(specifier: string): SandboxPath {
    return this.join(specifier);
  }

  /** Re-check existing components immediately before a filesystem operation. */
  protected override assertSafe(): void {
    assertContained(this.root, this.value);
  }
}

/** A confined filesystem view rooted at one existing directory. */
export class Sandbox {
  /** The canonical root path. */
  readonly root: SandboxPath;

  private readonly serializers: SerializerRegistry;

  /** @internal Created by `FileSystem.sandbox()`. */
  constructor(root: string, serializers: SerializerRegistry) {
    this.serializers = serializers;
    const absolute = pathe.resolve(root);
    let canonical: string;
    try {
      canonical = realpathSync(absolute);
      if (!statSync(canonical).isDirectory()) {
        throw new Error("root is not a directory");
      }
    } catch (error) {
      throw new Error(`Cannot create sandbox at "${absolute}": ${String(error)}`);
    }
    this.root = new SandboxPath(canonical, canonical, serializers);
  }

  /** Resolve a path relative to the sandbox root. */
  resolve(specifier: string): SandboxPath {
    return this.join(specifier);
  }

  /** Join path segments to the sandbox root. */
  join(...segments: string[]): SandboxPath {
    return new SandboxPath(
      pathe.resolve(this.root.value, ...segments),
      this.root.value,
      this.serializers
    );
  }

  /** Resolve an explicit path through the same containment checks. */
  path(value: string): SandboxPath {
    return this.resolve(value);
  }
}
