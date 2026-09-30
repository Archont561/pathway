---
type: Feature Spec
title: "Pluggable Patterns: Transformers, Hashers, Detectors, Validators, Resolvers, CAS"
description: "Extension points: transformers, hashers, detectors, validators, resolvers (unrs-resolver adapter), and content-addressed storage."
tags: [pluggable, transformer, hasher, detector, validator, resolver, pattern]
status: draft
generated:
  by: pathway_kb/1.0
  at: 2026-09-16T00:00:00Z
verified:
  - by: human:archont561
    at: 2025-07-11T00:00:00Z
  - by: process:gap-analysis-2026-09
    at: 2026-09-16T00:00:00Z
domain: features
decision: proposed  # legacy KB status (decided|proposed|deprecated)
created: 2025-07-11
source: conversation
depends_on:
  - architecture/core-layers
  - features/serializers
  - features/killer-features
---

# Pluggable Strategy Patterns

## The Unifying Philosophy

The serializer pattern works because it is:
1. **Pluggable** — swap implementations without changing call sites.
2. **Composable** — chain multiple strategies together.
3. **Invisible when unused** — zero overhead if you don't opt in.
4. **Implementation-agnostic** — JS or Rust backend, same interface.

This same philosophy applies to at least five more domains. Each follows
the identical structural pattern: **a named strategy object with typed
directional functions**.

---

## A. Transformers (Encode/Decode on Read/Write)

### The Problem

Files are often stored in transformed form: compressed, encrypted, encoded.
Currently, handling this requires manual pipeline management:

```ts
// Manual, verbose, error-prone
const compressed = await fs.readFile("data.json.gz");
const decompressed = zlib.gunzipSync(compressed);
const data = JSON.parse(decompressed.toString());
```

### The Interface

```ts
interface Transformer {
  readonly name: string;
  encode(data: Buffer): Buffer | Promise<Buffer>;
  decode(data: Buffer): Buffer | Promise<Buffer>;
}
```

### Built-in Implementations

```ts
import { gzip, brotli } from "@archont561/pathway";

const gzip: Transformer = {
  name: "gzip",
  encode: (data) => zlib.gzipSync(data),
  decode: (data) => zlib.gunzipSync(data),
};

// Native Rust version (flate2 crate, faster for large files)
const gzipNative: Transformer = {
  name: "gzip-native",
  encode: (data) => nativeGzipEncode(data),
  decode: (data) => nativeGzipDecode(data),
};
```

### Usage

```ts
// Single transform
const data = await file.read(json, { transform: [gzip] });
await file.write(json, data, { transform: [gzip] });

// Chained transforms (applied in order for encode, reverse for decode)
const data = await file.read(json, { transform: [encrypt, gzip] });
// Read pipeline: file bytes → gzip.decode → encrypt.decode → JSON.parse
// Write pipeline: JSON.stringify → encrypt.encode → gzip.encode → file bytes
```

### Execution Order

```
Read:  file bytes → T[n].decode → ... → T[1].decode → T[0].decode → serializer.parse → JS object
Write: JS object → serializer.stringify → T[0].encode → T[1].encode → ... → T[n].encode → file bytes
```

Transforms are applied **outside** the serializer. The serializer always
operates on the logical string representation.

### Phase Target: v0.3

---

## B. Hashers (Pluggable Digest Algorithms)

### The Problem

Different use cases need different hash algorithms:
- **Build caches:** Need speed above all → `xxhash` (fastest non-crypto)
- **Content-addressable storage:** Need collision resistance → `blake3`
- **Security/compliance:** Need NIST approval → `sha256`
- **Deduplication:** Need speed + low collision → `xxhash128`

Hardcoding one algorithm forces users into the wrong trade-off.

### The Interface

```ts
interface Hasher {
  readonly name: string;
  readonly digestLength: number;

  digest(data: Buffer): string;
  digestStream(): WritableStream;  // For large files
}
```

### Implementations

```ts
import { blake3, xxhash, sha256 } from "@archont561/pathway";

const blake3: Hasher = {
  name: "blake3",
  digestLength: 32,
  digest: (data) => nativeBlake3(data),     // Rust blake3 crate
  digestStream: () => nativeBlake3Stream(), // Streaming for large files
};

const xxhash: Hasher = {
  name: "xxhash64",
  digestLength: 8,
  digest: (data) => nativeXxHash(data),     // Rust xxhash-rust crate
  digestStream: () => nativeXxHashStream(),
};

const sha256: Hasher = {
  name: "sha256",
  digestLength: 32,
  digest: (data) => nativeSha256(data),     // Rust sha2 crate
  digestStream: () => nativeSha256Stream(),
};
```

### Usage

```ts
// Single file
const hash = await file.hash(blake3);
// → "a3f8c2e1..."

// Tree hash (fused walk + parallel hashing)
const treeHash = await project.hashTree({
  glob: "**/*.ts",
  exclude: ["node_modules"],
  hasher: xxhash,
});
// → "7d2f9a1b..."  (deterministic hash of all file contents + paths)
```

### Performance Comparison

| Algorithm | 1GB File | 100k × 1KB Files | Collision Resistance |
|-----------|----------|-------------------|---------------------|
| `xxhash64` | ~2.5s | ~80ms | Non-crypto |
| `blake3` | ~3.5s | ~120ms | Crypto-grade |
| `sha256` | ~8.0s | ~350ms | NIST standard |
| JS `crypto.createHash("sha256")` | ~12.0s | ~900ms | NIST standard |

All native implementations use Rust crates with SIMD acceleration.

### Phase Target: v0.2

---

## C. Detectors (MIME Type, Encoding, Structure)

### The Problem

Determining what a file *is* (beyond its extension) requires reading magic
bytes and heuristics. The JS ecosystem has `file-type` and `mime-types`,
but they're disconnected from the filesystem API.

### The Interface

```ts
interface Detector<T> {
  readonly name: string;
  detect(data: Buffer): T | null;  // null = inconclusive
}
```

### Implementations

```ts
const mimeDetector: Detector<string> = {
  name: "mime",
  detect: (data) => nativeInferMime(data),  // Rust `infer` crate
};

const encodingDetector: Detector<string> = {
  name: "encoding",
  detect: (data) => nativeDetectEncoding(data),  // Rust `encoding_rs`
};

const languageDetector: Detector<string> = {
  name: "language",
  detect: (data) => {
    // Heuristic: check for shebang, import syntax, etc.
    const head = data.slice(0, 256).toString();
    if (head.startsWith("#!/usr/bin/env python")) return "python";
    if (head.includes("package ")) return "go";
    return null;
  },
};
```

### Usage

```ts
const info = await file.detect([mimeDetector, encodingDetector]);
// info.mime → "image/png"
// info.encoding → "utf-8"

// With the fused walk
for await (const entry of project.walk({
  glob: "**/*",
  detect: [mimeDetector],
})) {
  if (entry.detected?.mime?.startsWith("image/")) {
    console.log("Image:", entry.path);
  }
}
```

### Why Native

The `infer` crate detects 100+ MIME types from magic bytes in ~1μs per file.
Doing this in JS requires reading the first 4–12 bytes of every file and
running a series of `Buffer.compare()` checks — 10–50x slower.

### Phase Target: v0.4

---

## D. Validators (Schema Validation on Read)

### The Problem

Reading a config file and getting `unknown` back means every consumer has
to validate manually. Schema validation should be a first-class part of
the read pipeline.

### The Interface

```ts
interface Validator<T> {
  readonly name: string;
  validate(data: unknown): T;  // Throws on failure
}
```

### Implementations

```ts
import { z } from "zod";

const zodValidator = <T>(schema: z.ZodType<T>): Validator<T> => ({
  name: "zod",
  validate: (data) => schema.parse(data),
});

// Usage with Valibot
import * as v from "valibot";

const valibotValidator = <T>(schema: v.GenericSchema<T>): Validator<T> => ({
  name: "valibot",
  validate: (data) => v.parse(schema, data),
});
```

### Usage

```ts
const ConfigSchema = z.object({
  server: z.object({
    port: z.number().min(1).max(65535),
    host: z.string(),
  }),
  database: z.object({
    url: z.string().url(),
  }),
});

const config = await file.read(toml, {
  validate: zodValidator(ConfigSchema),
});
// Type: { server: { port: number; host: string }; database: { url: string } }
// Throws ZodError with structured validation details on mismatch
```

### Architecture Decision: JS-Only

Validators run **entirely in JavaScript** after the native parser returns
the raw object. Rationale:
1. Validation libraries (Zod, Valibot, TypeBox, ArkType) are JS-native.
2. Coupling the Rust engine to a specific validation library is a bad idea.
3. Validation is typically fast compared to I/O and parsing.
4. Error messages need to be JS-friendly (stack traces, Zod formatting).

### Phase Target: v0.4

---

## E. Resolvers (Path Resolution Strategies)

### The Problem

Resolving a module specifier like `@acme/utils` to an actual file path
requires understanding the resolution algorithm (Node's `node_modules`,
TypeScript's `paths`, Bun's module resolution, etc.). Currently, every
tool reimplements this.

### The Interface

```ts
interface Resolver {
  readonly name: string;
  resolve(from: Path, specifier: string): Path | null;
  // null = this resolver can't handle this specifier
}
```

### Implementation Decision (Sept 2026): Integrate, Don't Build

The 2025 draft planned a from-scratch native resolver. **`unrs-resolver`
(MIT, Rust + NAPI-RS, published to npm) already implements exactly this:**

- ESM + CJS resolution per spec (package.json `exports` conditionals,
  `main`, `browser` fields)
- tsconfig `paths` / `baseUrl`, `extends`, project references
  (tsconfck-style discovery)
- Yarn Plug'n'Play (`.pnp.cjs`)
- Concurrent LRU caching, tracing instrumentation, 74 platform targets,
  WASM + JS fallbacks

It is maintained and used by Rspack-class tooling. Building our own would
reimagine a solved problem and lose PnP/`extends` compatibility.

**The v1.0 deliverable is an adapter, not an engine:**

1. Add the `unrs_resolver` **Rust crate** as an engine dependency (primary —
   zero extra FFI hops) or the npm package (fallback for TS-only builds).
2. Expose it through the `Resolver` interface above:
   ```ts
   export const unrsNodeResolver: Resolver;      // ESM/CJS + exports conditions
   export const unrsTsconfigResolver: Resolver;  // + tsconfig paths/references
   ```
3. Keep the interface open for other resolvers (Bun-specific, custom,
   in-memory FS).

### Usage

```ts
const resolved = project.resolve("@acme/utils", {
  resolvers: [unrsTsconfigResolver, unrsNodeResolver],
});
// Tries tsconfig first, falls back to node resolution
// Returns Path or throws ResolutionError

// With the fused walk — resolve all imports in a codebase
for await (const file of project.walkFiles({ glob: "**/*.ts" })) {
  const imports = await file.resolveImports({
    resolvers: [unrsTsconfigResolver, unrsNodeResolver],
  });
  for (const imp of imports) {
    console.log(`${file} → ${imp.resolved}`);
  }
}
```

### Why Native (via unrs-resolver)

Module resolution involves walking `node_modules` trees, reading hundreds of
`package.json` files, evaluating `exports` conditionals, and resolving
tsconfig aliases. `unrs-resolver` does all of it in Rust with concurrent
LRU caching — we get the performance without maintaining a second resolver
engine.

### Phase Target: v1.0 (adapter work; can be pulled forward if adoption requires)

---

## F. Content-Addressed Store (CAS) (added Sept 2026)

### The Problem

positioning.md promises "content-addressed caching primitives" to monorepo
teams, but no feature spec existed. Turbo-class tools keep this internal;
publishing it is a direct differentiator.

### The Interface

```ts
const cas = FileSystem.cas(project.join(".cas"));

const digest = await cas.put(file);              // → "blake3:7d2f9a1b…"
await cas.get("blake3:7d2f9a1b", dest);
cas.has("blake3:7d2f9a1b");                      // → boolean
await cas.gc({ keep: [manifest] });              // drop unreferenced blobs
```

### Design

- Store layout: `<root>/<first 2 hex chars>/<remaining hex>` (fanout 256,
  flat enough for fast lookups, shallow enough for `readdir`).
- `put` = hash (reuses the `Hasher` interface, chunked I/O) + **hardlink**
  into the store (copy fallback across filesystem boundaries) + verify.
- Digests are of **(path, content)** — not content only — so trees with
  identical files at different paths stay distinguishable.
- Blobs are append-only; `gc` acquires the section-6 lock from
  [killer-features.md](/features/killer-features.md) before pruning.
- Deterministic: same tree → same digests, any machine (sorted fold, see
  walk-traversal.md).

### Phase Target: v0.4

---

## The Unified Mental Model

All pluggable patterns collapse into one structural archetype:

```
┌──────────────────────────────────────────────────────────────┐
│                      Strategy Interface                      │
│                                                              │
│  readonly name: string                                       │
│  directional_function_a(input: A): B                         │
│  directional_function_b(input: B): A  (if bidirectional)     │
│                                                              │
│  Implementations: JS | Native Rust | Hybrid                  │
│  Composition: Array<Strategy> applied in order               │
│  Registration: Per-instance, never global                    │
└──────────────────────────────────────────────────────────────┘
```

| Pattern | Interface | Direction | JS or Native | Phase |
|---------|-----------|-----------|-------------|-------|
| Serializer | `Serializer<T>` | parse / stringify | Both | v0.1 |
| Transformer | `Transformer` | encode / decode | Both | v0.3 |
| Hasher | `Hasher` | digest | Native | v0.2 |
| Detector | `Detector<T>` | detect | Native | v0.4 |
| Validator | `Validator<T>` | validate | JS | v0.4 |
| Resolver | `Resolver` | resolve | Native (`unrs-resolver` adapter, Sept 2026) | v1.0 |
| CAS | `FileSystem.cas()` | put / get | Native | v0.4 (added Sept 2026) |

---

## Composition Pipeline

All patterns compose into a single read/write pipeline:

```
READ PIPELINE:
  file bytes
    → Transformer[n].decode()    ← decompress, decrypt
    → ...
    → Transformer[0].decode()
    → Serializer.parse()         ← JSON, TOML, YAML
    → Validator.validate()       ← Zod, Valibot
    → JS object (typed)

WRITE PIPELINE:
  JS object (typed)
    → Serializer.stringify()     ← JSON, TOML, YAML
    → Transformer[0].encode()    ← encrypt, compress
    → ...
    → Transformer[n].encode()
    → file bytes (atomic write)

WALK PIPELINE:
  directory tree
    → Walker (ignore crate)      ← traverse, prune
    → Matcher (glob + regex)     ← filter paths
    → Detector.detect()          ← classify files
    → Hasher.digest()            ← content hash
    → PathEntry (fully populated)
```

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| Same structural pattern for all | Consistency; users learn one pattern, apply everywhere |
| Bidirectional where applicable | Read and write are symmetric operations |
| JS or Native per-implementation | Use native only where it provides measurable speedup |
| Per-instance registration | No global mutable state; testable, composable |
| Composition via arrays | Order matters; explicit is better than magic |
| Validators stay in JS | Avoid coupling to Zod/Valibot; validation is fast enough |
| Hashers are native-first | SIMD-accelerated Rust crates are 3–5x faster than JS crypto |
| Resolvers wrap `unrs-resolver` | A maintained MIT Rust resolver (ESM/CJS + tsconfig + PnP + LRU cache) already exists — integrating beats rebuilding (Sept 2026 audit) |
| CAS ships in v0.4 with lock support | Backs the "content-addressed caching primitives" pitch promise (Sept 2026 audit) |
