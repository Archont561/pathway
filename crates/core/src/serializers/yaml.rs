//! YAML — for `@archont561/pathway-yaml` (v0.4).
//!
//! Same reasoning as [`super::toml`]: no native YAML parser in either runtime, so
//! the codec earns its place on the native side. `serde_yaml` is behind an
//! unmaintained-deprecated marker upstream, so the Phase 4 decision is whether
//! to track it or pin a fork — that is a backlog item, not something to settle
//! by picking whichever import path happens to compile today.

/// The extensions this codec claims by default, lowercase and with the dots.
///
/// # Examples
///
/// ```
/// // Both community spellings, `.yaml` first as the canonical one.
/// assert_eq!(pathway_fs_core::serializers::yaml::EXTENSIONS, [".yaml", ".yml"]);
/// ```
pub const EXTENSIONS: [&str; 2] = [".yaml", ".yml"];
