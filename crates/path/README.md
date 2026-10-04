# pathway-fs

A pathlib-inspired Rust filesystem API over the shared
[`pathway-fs-core`](https://crates.io/crates/pathway-fs-core) engine.

The planned ergonomic surface combines platform-native Rust paths with the
engine's fused walking, hashing, typed serde I/O, temporary-directory,
locking, and sandbox primitives.

The core is re-exported for advanced callers:

```rust
use pathway_fs::core;

let version = core::VERSION;
```

## Status

This crate is currently a Phase 1 scaffold. The crate boundary and dependency
on `pathway-fs-core` are in place; the full pathlib-style API is being built for
the Rust preview release.

## License

MIT
