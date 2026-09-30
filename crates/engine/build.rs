//! NAPI-RS build script.
//!
//! `napi_build::setup()` writes the platform-triple suffix into the build output,
//! which is what lets `napi build` name the `.node` file
//! `myorg_path_engine.linux-x64-gnu.node` and lets the generated TypeScript
//! loader pick the right one without a hand-maintained platform list.
//!
//! A hand-written platform require list is forbidden in this repository. The
//! 2025 draft of the binding loader was a `require("@myorg/path")` inside the
//! package that `@myorg/path` *is* — a circular self-require, because the binary
//! lives in the platform packages, not in the root one.

fn main() {
    napi_build::setup();
}
