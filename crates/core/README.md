# pathway-fs-core

The napi-free filesystem engine behind [`pathway-fs`](https://crates.io/crates/pathway-fs) and the TypeScript package [`@archont561/pathway`](https://www.npmjs.com/package/@archont561/pathway).

`pathway-fs-core` owns the shared filesystem logic:

- fused traversal, matching, and pruning;
- content hashing;
- atomic I/O, temporary files, locks, and sandboxing; and
- serde-backed filesystem codecs.

The crate is an `rlib` and has no Node.js or NAPI dependency. Its tests run with
plain Cargo:

```bash
cargo test -p pathway-fs-core
```

## Status

This crate is in the Phase 1 foundation stage. The public module layout and
engine boundary are established while the remaining filesystem capabilities are
implemented.

## License

MIT
