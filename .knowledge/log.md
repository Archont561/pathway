# Knowledge Base Update Log

## 2026-09-17
* **Update**: Migrated the bundle to the Open Knowledge Format (OKF) v0.2, declared as `okf_version: "0.2"` in [index.md](/index.md). Every concept now carries the required `type` plus the recommended `description` and `tags`; trust and lifecycle are recorded via `generated` / `verified` / `status` (§5). The legacy decision `status` (decided / proposed) is preserved as the `decision` extension key; legacy `updated` dates became `generated.at` (ISO 8601). `INDEX.md` was renamed to the reserved [index.md](/index.md) (frontmatter dropped per §8) and a reserved [log.md](/log.md) added (§9). All internal links now use the recommended bundle-root-absolute form (`/architecture/fused-walk.md`); `depends_on` IDs normalized (`meta/CONTEXT` → `CONTEXT`). [verified-data.md](/competitive/verified-data.md) gained structured `sources` plus `stale_after: 2026-12-16` (also on [landscape.md](/competitive/landscape.md)) for the quarterly market-data re-pull. The File Conventions section of [CONTEXT.md](/CONTEXT.md) documents the new schema.

## 2026-09-16
* **Update**: Integrated the Sept 2026 gap analysis across the bundle — runtime lines (Node 24 LTS / Node 26 Current, Node 20 EOL), stable `node:fs.glob` in Node core, Bun 1.3/1.4 (Rust rewrite; CI and benchmarks on both), NAPI-RS experimental iterators + `AsyncTask` guidance, `unrs-resolver` adapter decision for the v1.0 Resolver, reference-code fixes, benchmark re-baseline. The standalone gap-analysis file was removed, its content folded into [verified-data.md](/competitive/verified-data.md) and [CONTEXT.md](/CONTEXT.md).

## 2025-07-11
* **Initialization**: Created the knowledge base: [CONTEXT.md](/CONTEXT.md) plus 15 concepts across architecture, features, competitive, and implementation.
