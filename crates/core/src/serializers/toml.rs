//! TOML — for `@archont561/pathway-toml` (v0.4).
//!
//! A *native* codec, and that is the qualification that matters: TOML has no
//! parser in Node or Bun, so a JavaScript serializer here would mean shipping a
//! TOML parser as a runtime dependency to compete with what is already in the
//! native binary. This is the shape native codecs exist for — contrast JSON,
//! which stays in TypeScript because `JSON.parse` is already native and faster
//! than converting `serde_json::Value` across the boundary (Q4).

/// The extension this codec claims by default, lowercase and with the dot.
pub const EXTENSION: &str = ".toml";
