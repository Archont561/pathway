import { Path } from "./path.js";
import { Sandbox } from "./sandbox.js";
import { json } from "./serializers/json.js";
import { SerializerRegistry } from "./serializers/registry.js";
import type { FileSystemOptions, Serializer } from "./types.js";

/**
 * A filesystem view with its own serializer registry.
 *
 * The registry belongs to the view rather than to Path or the module. This
 * keeps applications, plugins, and tests from changing one another's codec
 * resolution.
 */
export class FileSystem {
  private constructor(private readonly serializers: SerializerRegistry) {}

  /** Create an isolated filesystem view. A view without options has JSON enabled. */
  static create(options: FileSystemOptions = {}): FileSystem {
    const serializers = options.serializers ?? [json];
    return new FileSystem(new SerializerRegistry(serializers));
  }

  /** Create a Path that uses this filesystem view. */
  path(value: string): Path {
    return new Path(value, this.serializers);
  }

  /** Create a path-resolution view confined to an existing directory. */
  sandbox(root: string): Sandbox {
    return new Sandbox(root, this.serializers);
  }

  /** The current working directory in this filesystem view. */
  cwd(): Path {
    return this.path(process.cwd());
  }

  /** Add or replace extension mappings in this filesystem view only. */
  register<T>(serializer: Serializer<T>, extensions?: readonly string[]): this {
    this.serializers.register(serializer, extensions);
    return this;
  }

  /** Resolve a serializer for a path, primarily for adapters and diagnostics. */
  resolveSerializer(filePath: string): Serializer<unknown> | undefined {
    return this.serializers.resolve(filePath);
  }
}
