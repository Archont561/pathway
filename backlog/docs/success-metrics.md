---
id: doc-success-metrics
title: "Release Success Metrics v0.1 / v0.5 / v1.0"
type: document
status: active
created_date: '2025-07-11'
updated_date: '2026-10-06'
tags: [metrics, success-criteria, release, adoption]
---

> Extracted from `.knowledge/competitive/positioning.md` on 2026-10-02. These
> are checkable release gates rather than market analysis, so they sit beside
> the milestones they gate. The positioning argument they came from - audience,
> moats, Bun-as-ally posture - stays in the knowledge base.
>
> The v0.1 row is the acceptance bar for milestone `m-0`; the v0.5 and v1.0 rows
> are adoption outcomes, tracked but not owned by any single task.

# Success Metrics

### v0.1 (Proof of Concept)
- [ ] Fused walk benchmark ≥5x faster than `node:fs.glob` + `fs.stat` +
      `crypto` (and all JS incumbents)
- [x] Works on Node 24 (LTS), Node 26 (Current), Bun 1.3.x, Bun 1.4.x — runtime matrix run 37382716742
- [x] CI passes on Linux, macOS, Windows — runtime matrix run 37382716742
- [ ] `Path`, `walkFiles`, `read(json)`, `write(json)` all functional

### v0.5 (Early Adoption)
- [ ] 1,000+ GitHub stars
- [ ] 100k+ weekly npm downloads
- [ ] Adopted by at least 2 build tools or monorepo frameworks
- [ ] TOML and YAML native serializers published

### v1.0 (Ecosystem Replacement)
- [ ] 10,000+ GitHub stars
- [ ] 1M+ weekly npm downloads
- [ ] Recognized as the default filesystem library for new TypeScript projects
- [ ] `fs-extra` downloads declining in our favor
