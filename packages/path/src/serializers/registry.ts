import { extname } from "pathe";
import type { Serializer } from "../types.js";
import { json } from "./json.js";

type RegisteredSerializer = Serializer<unknown>;

function asRegistered<T>(serializer: Serializer<T>): RegisteredSerializer {
  // The registry stores strategies without invoking them. The caller keeps the
  // generic type when it passes a serializer explicitly to Path.read/write.
  return serializer as unknown as RegisteredSerializer;
}

function normalizeExtension(extension: string): string {
  const value = extension.startsWith(".") ? extension : `.${extension}`;
  return value.toLowerCase();
}

/**
 * Isolated extension-to-serializer mappings for one FileSystem instance.
 *
 * There is deliberately no module-level mutable registry. Two filesystem
 * views in the same process can register different codecs for the same suffix
 * without import order or test order changing their behavior.
 */
export class SerializerRegistry {
  private readonly byExtension = new Map<string, RegisteredSerializer>();

  constructor(serializers: readonly RegisteredSerializer[] = []) {
    for (const serializer of serializers) {
      this.register(serializer);
    }
  }

  /** Register a serializer using its declared extensions or its name. */
  register<T>(serializer: Serializer<T>, extensions?: readonly string[]): this {
    const mapping = extensions ?? serializer.extensions ?? [`.${serializer.name}`];
    const registered = asRegistered(serializer);

    for (const extension of mapping) {
      this.byExtension.set(normalizeExtension(extension), registered);
    }

    return this;
  }

  /** Resolve a serializer for a path's final extension. */
  resolve(filePath: string): RegisteredSerializer | undefined {
    const extension = extname(filePath).toLowerCase();
    return this.byExtension.get(extension);
  }
}

/** The default Path view supports the built-in JSON serializer only. */
export const defaultSerializerRegistry = new SerializerRegistry([json]);
