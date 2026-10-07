import { Path } from "./path.js";
import { Sandbox } from "./sandbox.js";
import { json } from "./serializers/json.js";
import { SerializerRegistry } from "./serializers/registry.js";
import { type TempOptions, withTempDirectory } from "./temp.js";
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

  /**
   * Run `callback` with a temporary directory bound to this view.
   *
   * The `Path` the callback receives resolves serializers through this view's
   * registry, exactly like every other path it hands out; the cleanup
   * guarantee is the one {@link Path.temp} documents.
   */
  async temp<T>(callback: (dir: Path) => Promise<T>): Promise<T>;
  /** Run `callback` with a temporary directory created from `options`. */
  async temp<T>(options: TempOptions, callback: (dir: Path) => Promise<T>): Promise<T>;
  async temp<T>(
    optionsOrCallback: TempOptions | ((dir: Path) => Promise<T>),
    maybeCallback?: (dir: Path) => Promise<T>
  ): Promise<T> {
    const callback = typeof optionsOrCallback === "function" ? optionsOrCallback : maybeCallback;
    if (callback === undefined) {
      throw new TypeError(
        "FileSystem#temp needs a callback; pass (options, callback) or (callback)"
      );
    }
    const options = typeof optionsOrCallback === "function" ? undefined : optionsOrCallback;

    return withTempDirectory(options, (path) => callback(this.path(path)));
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
