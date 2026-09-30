//! NAPI-RS build script.
//!
//! `napi_build::setup()` writes the platform-triple suffix into the build
//! output, which is what lets `@napi-rs/cli` rename the cdylib artifact to
//! `<napi.name>.<platform-triple>.node` — here `pathway.linux-x64-gnu.node`,
//! from the `napi.name` field of `packages/path/package.json` — so the loader
//! can pick the right one without a hand-maintained platform list.
//!
//! A hand-written platform require list is forbidden in this repository. The
//! 2025 draft of the binding loader was a `require("@archont561/pathway")`
//! from inside the package that `@archont561/pathway` *is* — a circular
//! self-require, because the binary lives in the platform packages
//! (`@archont561/pathway-linux-x64-gnu` and friends), not in the root one.

fn main() {
    napi_build::setup();
}
