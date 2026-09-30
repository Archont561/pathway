//! Serialization codecs (D4).
//!
//! A serializer is a strategy object with `parse` and `stringify`, not a mode
//! baked into `Path`. `read(toml)` and `write(json, data)` are the same call
//! with a different strategy, and the strategy is a value the caller passes —
//! which is what makes `Serializer<T>` a TypeScript generic on one surface and a
//! `serde::DeserializeOwned` bound on the other.
//!
//! ## Registry placement
//!
//! Registries are **per `FileSystem` instance**, never global (D6). A global
//! `Path.register()` is mutable process-wide state: two instances in one
//! process could not disagree about what `read("json")` resolves to, and a
//! library that mutates a global on import is a library that cannot be tested
//! twice in the same runner.
//!
//! ## JSON stays in the JavaScript layer
//!
//! `serde_json::Value` → N-API object creation is *slower* than V8's own
//! `JSON.parse` for small files, because the conversion is a walk across the
//! boundary. Native codecs earn their place on TOML, YAML and `MessagePack`,
//! where there is no native parser at all. See Q4 in the context file.
//!
//! ## Planned layout
//!
//! - `toml.rs` — `toml-rs` + `serde`, for `@myorg/path-toml`.
//! - `yaml.rs` — `serde_yaml`, for `@myorg/path-yaml`.
//!
//! JSON is a built-in TypeScript serializer (`packages/path/src/serializers/json.ts`),
//! not a native codec — that is the decision, not an omission.

pub mod toml;
pub mod yaml;
