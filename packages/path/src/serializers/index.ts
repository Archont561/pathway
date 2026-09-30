/**
 * Serializers: the strategy objects `read`/`write` take.
 *
 * Re-exported from the package root so `import { json } from "@archont561/pathway"`
 * works, which is the only spelling a consumer should need.
 */

export type { Serializer } from "../types.js";
export { json } from "./json.js";
