import type { Serializer } from "../types.js";

/**
 * The built-in JSON serializer.
 *
 * JSON stays in JavaScript: JSON.parse/stringify are already implemented by
 * the runtime, and crossing the native boundary to build a JS object would
 * cost more for the small-to-medium values this codec targets.
 */
export const json: Serializer<unknown> = {
  name: "json",
  native: false,
  extensions: [".json"],

  parse(bytes) {
    return JSON.parse(new TextDecoder().decode(bytes)) as unknown;
  },

  stringify(value) {
    const text = JSON.stringify(value, null, 2);
    if (text === undefined) {
      throw new TypeError("JSON serializer cannot encode undefined");
    }
    return new TextEncoder().encode(`${text}\n`);
  }
};
