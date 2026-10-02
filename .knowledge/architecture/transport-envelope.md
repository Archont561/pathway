---
type: Architecture Decision
title: "The geoquery Transport Envelope, Applied to pathway"
description: "What the geoquery FFI redesign (protocol crate + one JSON invoke + thin per-language adapters) is actually buying, which parts transfer to pathway, and which parts would break the fused walk."
tags: [ffi, napi-rs, protocol, transport, pyo3, boundary]
status: proposed
domain: architecture
decision: proposed
created: 2026-10-02
source: cross-repo analysis (Archont561/geoquery @ main)
depends_on:
  - architecture/napi-boundary
  - architecture/core-layers
---

# The geoquery Transport Envelope, Applied to pathway

## 1. What geoquery actually built

Four crates, and the whole idea is that **the wire is the interface**.

```text
crates/protocol        serde DTOs + TRANSPORT_VERSION. No napi, no pyo3, no core.
      ▲
crates/engine          fn invoke(&str) -> String.  One match over Operation.
      ▲
      ├── crates/node-native     #[napi] fn invoke(String) -> String      (6 lines)
      └── crates/python-native   #[pyfunction] fn invoke(&str) -> String  (10 lines)
```

`crates/protocol/src/lib.rs`:

```rust
pub const TRANSPORT_VERSION: u32 = 1;

#[serde(rename_all = "camelCase")]
pub struct EngineRequest  { transport_version: u32, operation: Operation, payload: Value }
pub struct EngineResponse { transport_version: u32, ok: bool, result: Value }

#[serde(rename_all = "camelCase")]
pub enum Operation { Ping, ProtocolVersion, ParseDocument }   // closed, on purpose
```

The property being bought is stated in their own doc comment: *adding an operation is a
change to one enum rather than a change to every FFI signature in every language.* With
six planned consumers (CLI, HTTP, MCP, TUI, Python, TypeScript) that is a real multiplier.

Five decisions in there are worth stealing independently of the envelope itself:

| Decision | Why it exists |
|---|---|
| `crate-type = ["cdylib", "rlib"]` on both adapters | a cdylib alone is only loadable by a runtime. The rlib is what makes the adapter reachable from `tests/` and lintable by ordinary workspace tooling. |
| `node-native` restates lints with `unsafe_code = "deny"` | `#[napi]` expands to a module constructor carrying its own `#[allow(unsafe_code)]`, and `forbid` is precisely the level a macro cannot override. An addon under `forbid` **does not compile**, and the error points at the `#[napi]` line while naming no policy. |
| `extension-module` is an opt-in **feature**, not a dependency attribute | with it always on, pyo3 doesn't link libpython, so `cargo nextest run --workspace` fails to link the test binary with a wall of `undefined reference to PyList_New`. maturin turns it on only for the wheel. |
| FFI crates live under `crates/*`, not inside the Python/JS packages | a binding crate has no more reason to live inside a language package than the engine it binds; it also makes them visible to the version sweep. |
| Errors cross as `ok: false` + **core's own wording** | one phrasing of "why the query was rejected", shared by CLI, Python and TS. A second phrasing in TypeScript is a second thing to keep true. |

## 2. The part that does *not* transfer

Geoquery's operations are **coarse, rare, and JSON-shaped**: parse a document, report a
version. `String` in / `String` out costs nothing next to an HTTP round trip to a STAC
service.

pathway's are the opposite, and `napi-boundary.md` already says so in capitals: the
boundary exists for *bulk* operations, and the moat is the fused walk — traversal + stat +
hash + filter in one pass, yielding batches of 512 entries. Routing that through
`invoke(String) -> String` would mean:

- **Bytes get base64'd.** `read()` returns file contents. JSON-encoding a 10 MB file costs
  a ~1.37× size blowup plus two full encode/decode passes, to transport something N-API
  hands over as a zero-copy `Buffer` today.
- **A 100k-entry walk gets serialised twice.** Build a `Value`, `serde_json::to_string` it,
  `JSON.parse` it. That is precisely the per-file cost the fused walk exists to delete.
- **Streaming becomes impossible.** `fn(String) -> String` is sync and single-shot. It
  cannot express `AsyncTask`, cannot express `#[napi(async_iterator)]`, and therefore
  cannot express early `break` cancellation or a bounded prefetch window — the three
  things the Phase 1 Step 0 spike is about.
- **`withLock()` / `temp()` cannot be modelled at all.** They hand back a *live handle* with
  a lifetime. JSON has no representation for "an open `flock()` you must release".

So: adopting the envelope wholesale would trade pathway's only technical justification for
a native dependency in exchange for a code-organisation win.

## 3. The proposal: split the control plane from the data plane

Take the envelope for the plane it suits, and leave the hot path typed.

```text
crates/protocol   ← NEW. serde DTOs, napi-free, pyo3-free, core-free.
                     · EngineRequest/EngineResponse/Operation, TRANSPORT_VERSION
                     · AND the option/entry DTOs: WalkOptions, HashRequest,
                       WriteOptions, FusedEntry, EntryError
      ▲
crates/core       ← unchanged. Consumes the DTOs, implements the behaviour.
      ▲
crates/dispatch   ← NEW. fn invoke(&str) -> String. The control plane only.
      ▲
crates/engine     ← stays the napi cdylib, and gains BOTH:
                     · #[napi] fn invoke(String) -> String        (control plane)
                     · #[napi] typed fns + AsyncTask/async_iterator (data plane)
```

**Control plane** — anything low-frequency, config-shaped, or diagnostic. Goes through
`invoke`: `engineVersion`, `napiVersion`, `capabilities`, `registeredHashers`,
`registeredSerializers`, sandbox-root configuration, `transportVersion`. These are called
once at load or once per `FileSystem.create()`. The envelope's cost is unmeasurable and its
versioning is exactly what you want.

**Data plane** — `walk`, `hashTree`, `copyTo`, `snapshot`, `read`, `write`, `withLock`,
`temp`. Stays typed `#[napi]`, stays `Buffer`-based, stays async. Tier A/B/C of
`napi-boundary.md` is unchanged.

### What this buys pathway specifically

1. **It kills the DTO drift that already exists.** `packages/path/src/types.ts` hand-writes
   `HasherName = "blake3" | "xxhash" | "sha256"` while `core/src/error.rs` independently
   hand-writes `UnknownHasher("expected one of blake3, xxhash, sha256")`. Those are two
   copies of one list, and nothing fails when they diverge. Put the enum in `crates/protocol`
   and both sides read it. Note pathway can do *better* than geoquery here: geoquery admits
   its TS types are "the one place a shape is written down twice" because its wire is plain
   JSON — but pathway's wire is N-API, and `napi build` **generates** `index.d.ts` from the
   `#[napi]` symbol table. Moving the DTOs into a protocol crate and deriving `#[napi(object)]`
   on them makes the TypeScript definitions generated rather than mirrored.

2. **It makes `crates/engine` testable.** Today it is `crate-type = ["cdylib"]` — nothing in
   it can be reached by `cargo test`, so the two functions it exports are verified by
   nothing. Adding `"rlib"` (geoquery's choice) fixes that for one line of Cargo.toml.

3. **It upgrades the staleness check.** `binding.ts` currently refuses an addon whose
   `engineVersion()` string differs. That is too strict — it forces a rebuild for a patch
   bump that changed no symbol. A `TRANSPORT_VERSION: u32` bumped only when a shape changes
   means "compatible" is a thing the addon can actually claim.

4. **It is the only way `crates/path` and `packages/path` stay honest.** D7 says a difference
   between the two surfaces is a bug, not a feature. Right now nothing enforces that — they
   are two hand-written surfaces over one core. A shared `Operation` list and shared option
   DTOs give the claim something to rest on.

5. **It pre-pays for a Python surface.** If `pathway-fs-python` is ever wanted, the control
   plane is free and only the data plane needs writing — which is exactly the shape
   geoquery is in.

## 4. Concrete first slice

Smallest change that is worth landing, in order:

1. `crates/engine/Cargo.toml`: `crate-type = ["cdylib", "rlib"]`; add `crates/engine/tests/lib.rs`.
2. Add `[workspace.lints]` to the root `Cargo.toml` (pathway has none today — `deny(missing_docs)`
   is repeated per-crate in `lib.rs`), and have `crates/engine` restate it with
   `unsafe_code = "deny"` rather than `forbid`, with geoquery's comment explaining why.
   Doing this *before* the engine grows `#[napi]` functions avoids hitting the
   un-overridable-`forbid` wall mid-feature.
3. New `crates/protocol`: `TRANSPORT_VERSION`, `EngineRequest`/`EngineResponse`/`Operation`
   with `Ping`, `Capabilities`, `EngineVersion`. Move `HasherName` here.
4. New `crates/dispatch` with `invoke(&str) -> String` + `success`/`failure` helpers;
   `#[napi] fn invoke` in `crates/engine` is then four lines.
5. `packages/path/src/binding.ts`: negotiate `TRANSPORT_VERSION` at load instead of
   string-comparing `engineVersion()`.
6. Workspace `members` is an explicit list (`["crates/core", "crates/path", "crates/engine"]`).
   Geoquery moved to `members = ["crates/*"]` + `exclude`, which makes adding a crate one
   edit in the crate's own directory. Worth doing at step 3 rather than four times.

## 5. The honest counter-argument

pathway has **two** consumers, not six, and they are both in-process. The envelope's
payoff scales with the number of language surfaces, and at two surfaces a shared
`crates/protocol` of plain DTOs captures most of the benefit without a dispatcher at all.
If a Python/WASM/CLI surface is not on the roadmap, implement steps 1–3 and 6 and stop —
the `invoke` dispatcher (steps 4–5) is the part that only pays once a third consumer
exists.
