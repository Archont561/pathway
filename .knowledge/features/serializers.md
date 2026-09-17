---
type: Feature Spec
title: "Serializer Pattern: Pluggable Codecs, Serde, Registry, Generics"
description: "Pluggable Serializer<T> pattern: JS strategy objects and native Serde codecs behind per-FileSystem registries."
tags: [serializer, serde, json, toml, yaml, codec, generics, registry]
status: stable
generated:
  by: pathway_kb/1.0
  at: 2026-09-16T00:00:00Z
verified:
  - by: human:archont561
    at: 2025-07-11T00:00:00Z
  - by: process:gap-analysis-2026-09
    at: 2026-09-16T00:00:00Z
domain: features
decision: decided  # legacy KB status (decided|proposed|deprecated)
created: 2025-07-11
source: conversation
depends_on:
  - architecture/core-layers
  - architecture/napi-boundary
---

# Pluggable Serializer Architecture

## Overview

The serializer system is where `@myorg/path` becomes **unusually interesting**
compared to every other filesystem library. No existing JS filesystem library
offers typed, pluggable serialization as a first-class API.

The core idea: reading and writing structured data should be as simple as
passing a codec object to `read()` and `write()`.

```ts
const config = await file.read(toml);
await file.write(toml, config);
```

---

## The Serializer Interface

### Core Definition

```ts
interface Serializer<T = unknown> {
  readonly name: string;

  parse(input: string): T;
  stringify(value: T): string;
}
```

That's it. Two directional functions and a name. This is the same pattern
as the `Serializer<T>` concept in the killer-features design — a **strategy
object** that can be implemented in JS or backed by native Rust.

### Built-in: JSON

```ts
import { json } from "@myorg/path";

const data = await file.read(json);
await file.write(json, { name: "myapp", version: "1.0.0" });
```

Implementation:

```ts
export const json: Serializer<unknown> = {
  name: "json",
  parse: (input: string) => JSON.parse(input),
  stringify: (value: unknown) => JSON.stringify(value, null, 2),
};
```

**Important:** JSON parsing stays in JavaScript. V8's `JSON.parse()` is
implemented in C++ and is extremely fast — likely faster than converting
`serde_json::Value` through N-API object creation calls for small-to-medium
files. Native Serde is reserved for formats where JS lacks a built-in C++
parser.

### Extension: TOML

```ts
import { toml } from "@myorg/path-toml";

const config = await file.read(toml);
await file.write(toml, config);
```

Implementation (native Rust via Serde):

```rust
// Inside the Rust engine
#[napi]
pub fn parse_toml(input: String) -> Result<napi::JsUnknown> {
    let value: toml::Value = toml::from_str(&input)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
    // Convert toml::Value → N-API JS object
    toml_to_js(env, &value)
}
```

### Extension: YAML, CBOR, MessagePack, JSON5

Each follows the same pattern as separate packages:

```
@myorg/path-json      ← Built into core (JS JSON.parse)
@myorg/path-toml      ← Native Serde (toml-rs)
@myorg/path-yaml      ← Native Serde (serde_yaml)
@myorg/path-cbor      ← Native Serde (ciborium)
@myorg/path-msgpack   ← Native Serde (rmp-serde)
@myorg/path-json5     ← JS (json5 package, no native benefit)
```

---

## Separation from Path Internals

### Public API

```ts
const data = await file.read(toml);
```

### Internal Architecture

```
Path.read(toml)
   │
   ├── native.read()          ← Rust reads file bytes, returns string
   │
   └── serializer.parse()     ← Serializer parses string to object
```

The serializer is **independent** of the `Path` object. This means both
usage patterns are valid:

```ts
// Composed (convenient)
const data = await file.read(toml);

// Decomposed (flexible)
const text = await file.readText();
const data = toml.parse(text);
```

The decomposed form is useful when you need to:
- Inspect the raw text before parsing
- Try multiple parsers on the same content
- Cache the raw text and parse lazily
- Pass the text to a non-Serializer parser

---

## Strong Typing with Generics

### The Problem

```ts
const config = await file.read(toml);
// Type: unknown ← not useful
```

### Solution 1: Generic Type Parameter

```ts
interface Config {
  server: { port: number; host: string };
  database: { url: string };
}

const config = await file.read<Config>(toml);
// Type: Config ← fully typed
```

The `read` method signature:

```ts
class Path {
  async read<T = unknown>(serializer: Serializer<T>): Promise<T>;
}
```

### Solution 2: Serializer Carries the Type

```ts
// User creates a typed serializer
const configToml: Serializer<Config> = {
  name: "config-toml",
  parse: (input) => {
    const raw = toml.parse(input);
    return ConfigSchema.parse(raw);  // Zod validation
  },
  stringify: (value) => toml.stringify(value),
};

const config = await file.read(configToml);
// Type: Config ← inferred from serializer
```

### Solution 3: Schema Validation Layer

```ts
const config = await file.read(toml, {
  schema: ConfigSchema,   // Zod, Valibot, TypeBox, etc.
});
// Type: Config ← inferred from schema
// Throws structured validation error on mismatch
```

**Decision:** Schema validation stays **outside the Rust core**. It runs in
JS after the native parser returns the raw object. This keeps the Rust engine
focused on parsing performance and avoids coupling to any specific validation
library.

---

## Automatic Serialization (Second Layer)

### Explicit (Default, v0.1)

```ts
const data = await file.read(json);     // You choose the codec
const data = await file.read(toml);     // Explicit is clear
```

### Automatic (v0.2+, Opt-In)

```ts
const data = await file.read();         // Codec inferred from extension
```

This requires a **registry** that maps file extensions to serializers:

```ts
const fs = FileSystem.create({
  serializers: [json, toml, yaml],
});

const config = await fs.path("config.toml").read();
// Registry maps ".toml" → toml serializer
// Type: unknown (no type inference from extension alone)
```

### Registry Implementation

```ts
class SerializerRegistry {
  private map = new Map<string, Serializer>();

  register(serializer: Serializer, extensions: string[]): void {
    for (const ext of extensions) {
      this.map.set(ext.startsWith(".") ? ext : `.${ext}`, serializer);
    }
  }

  resolve(filePath: string): Serializer | undefined {
    const ext = path.extname(filePath);
    return this.map.get(ext);
  }
}
```

### Default Extension Mappings

| Extension | Serializer | Package |
|-----------|-----------|---------|
| `.json` | `json` | `@myorg/path` (built-in) |
| `.toml` | `toml` | `@myorg/path-toml` |
| `.yaml`, `.yml` | `yaml` | `@myorg/path-yaml` |
| `.json5` | `json5` | `@myorg/path-json5` |
| `.cbor` | `cbor` | `@myorg/path-cbor` |
| `.msgpack` | `msgpack` | `@myorg/path-msgpack` |

---

## No Global Registration

### The Anti-Pattern

```ts
// ❌ DO NOT DO THIS
Path.register(".toml", toml);
```

This is **global mutable state**. It causes:
1. **Cross-contamination:** Two libraries in the same process register
   different TOML parsers. Last one wins.
2. **Test pollution:** One test registers a mock serializer that leaks
   into other tests.
3. **Non-determinism:** Behavior depends on import order.
4. **Incompatibility with ESM:** Module evaluation order is not guaranteed.

### The Correct Pattern

```ts
// ✅ Per-instance registry
const fs = FileSystem.create({
  serializers: [json, toml, yaml],
});

const config = await fs.path("config.toml").read();
```

Now two applications in the same process can have completely different
registries:

```ts
const appFs = FileSystem.create({
  serializers: [json, toml],
});

const testFs = FileSystem.create({
  serializers: [json, mockToml],  // Mock for testing
});
```

### Fallback for Simple Usage

For users who don't want to create a `FileSystem` instance, the default
`Path` class ships with JSON only:

```ts
const data = await Path.cwd().join("package.json").read(json);  // Works
const data = await Path.cwd().join("config.toml").read();       // Throws: no serializer for .toml
```

To enable automatic resolution on the default `Path`, users opt in:

```ts
import { toml } from "@myorg/path-toml";
Path.configure({ serializers: [toml] });  // Explicit opt-in, documented warning
```

This is a **conscious trade-off**: convenience vs purity. The per-instance
API is the recommended pattern; the global configure is the escape hatch.

---

## JS vs Native Serializer Decision Matrix

| Format | JS Parser Speed | Native Benefit | Decision |
|--------|----------------|----------------|----------|
| JSON | ⚡⚡⚡ (V8 C++) | Minimal | **JS** (`JSON.parse`) |
| TOML | ⚡ (pure JS) | Significant | **Native** (`toml-rs`) |
| YAML | ⚡ (js-yaml, C++ bindings) | Moderate | **Native** (`serde_yaml`) for consistency |
| CBOR | ⚡ (pure JS) | Significant | **Native** (`ciborium`) |
| MessagePack | ⚡⚡ (msgpackr, C++) | Moderate | **Native** (`rmp-serde`) |
| JSON5 | ⚡ (pure JS) | Minimal | **JS** (`json5` package) |

### The Serde→JS Bridge Challenge

Converting Rust's `serde_json::Value` (or `toml::Value`) to N-API JS objects
requires walking the entire value tree and calling `napi_create_*` for each
node. For large objects, this can be **slower** than returning the raw string
and letting V8 parse it.

**Strategy:**
1. For **JSON**: Always use JS `JSON.parse()`. Return raw string from Rust.
2. For **TOML/YAML/CBOR/MessagePack**: Use native Serde parsing. The JS
   ecosystem lacks fast C++ parsers for these formats, so the N-API object
   creation overhead is worth it. (Sept 2026: the `serde-json` feature of
   `napi` provides the `serde_json::Value` → `JsUnknown` conversion this
   path relies on — verified against current napi-rs docs.)
3. For **very large files** (>10MB): Consider returning raw string and
   letting a WASM or JS parser handle it, regardless of format.

---

## Write Path

### Simple Write

```ts
await file.write(toml, configData);
```

### Atomic Write

```ts
await file.write(toml, configData, { atomic: true });
```

Implementation:
```
1. Serialize to string (JS or Rust)
2. Write to temporary file (same directory, random suffix)
3. fsync (if requested)
4. Atomic rename (temp → target)
```

### Write Options

```ts
interface WriteOptions {
  atomic?: boolean;       // Write to temp, then rename (default: false)
  fsync?: boolean;        // fsync before rename (default: false)
  mode?: number;          // File permissions (default: 0o644)
  encoding?: string;      // String encoding (default: "utf-8")
}
```

---

## Serializer Composition

Serializers can be composed with transformers (see
[pluggable-patterns.md](/features/pluggable-patterns.md)):

```ts
// Read a gzipped JSON file
const data = await file.read(json, { transform: [gzip] });

// Write encrypted TOML
await file.write(toml, config, { transform: [encrypt], atomic: true });
```

The execution order is:

```
Read:  file bytes → transform.decode() → serializer.parse() → JS object
Write: JS object → serializer.stringify() → transform.encode() → file bytes
```

---

## Decision Summary

| Decision | Rationale |
|----------|-----------|
| `Serializer<T>` interface | Minimal, composable, familiar strategy pattern |
| JSON stays in JS | V8 `JSON.parse()` is faster than Serde→N-API for most files |
| TOML/YAML/CBOR go native | No fast JS C++ parsers; Serde ecosystem is mature |
| Serialization separate from Path | Enables decomposed usage; keeps Path focused |
| Generics on `read<T>()` | Type safety without runtime cost |
| Schema validation in JS | Avoids coupling Rust to Zod/Valibot/TypeBox |
| Auto-detect is second layer | Explicit is default; auto is opt-in convenience |
| No global registration | Prevents cross-contamination; per-instance registries |
| Atomic writes via temp+rename | OS-level atomicity; safe for config files |
| Separate packages per codec | Tree-shakeable; users only install what they need |
