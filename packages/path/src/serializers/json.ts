/**
 * The built-in JSON serializer.
 *
 * JSON is the one format that stays in JavaScript, and that is a decision
 * rather than an omission. `JSON.parse` is already native, and it is
 * *faster* than converting a `serde_json::Value` into a JavaScript object: that
 * conversion is a walk across the N-API boundary, one property at a time, and
 * for the small files where JSON is most common it costs more than it saves.
 *
 * Native codecs earn their place on TOML, YAML and MessagePack, where neither
 * Node nor Bun ships a parser at all — there the alternative is a pure-JavaScript
 * parser in the dependency tree.
 *
 * The implementation is four lines. It is still here, as a value implementing
 * {@link Serializer}, because that is the point: `json` is a *registered
 * serializer*, not a mode baked into `Path`, and a user-supplied
 * `Serializer<T>` is interchangeable with this one.
 *
 * ## Why the type parameter is `never`
 *
 * `Serializer<never>` is what makes `read<MyShape>(json)` compile. Interface
 * methods are checked bivariantly, `never` is assignable to every type, and
 * `parse` returning `never` is trivially safe at runtime because the cast
 * happens where the caller supplies the type. Typing it as
 * `Serializer<unknown>` instead would force every call site to be
 * `read<unknown>(json)` plus a cast — moving the type error to the consumer,
 * which is the opposite of what a generic is for.
 *
 * `readJson<T>()` is the ergonomic spelling and is what most callers want.
 */

import type { Serializer } from "../types.js";

/** The built-in JSON serializer. `never` as the value type; see the note above. */
export const json: Serializer<never> = {
  name: "json",
  native: false,

  parse(bytes) {
    return JSON.parse(new TextDecoder().decode(bytes)) as never;
  },

  stringify(value) {
    return new TextEncoder().encode(JSON.stringify(value));
  }
};
