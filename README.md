# Offline sandbox (orphan branch)

Built 2026-10-03T20:21:58Z from commit `879036b` for platform `linux-64`.
`pixi.lock` sha256 `9e15a944ef4e38c00157ae8ac843300589b91179439464f43a06b52aac598c74`.

The verified self-bootstrap binary is stored at `.pixi-sandbox/tools/linux-64/pixi-sandbox`. The branch root intentionally contains documentation only.

| env | platform | packed | unpacked | files |
| --- | --- | ---: | ---: | ---: |
| `default` | linux-64 | 412.7 MiB | 1726.7 MiB | 51 |

Cargo dependencies: **132 crates**, 194.1 MiB (loose) from `Cargo.lock` sha256 `8e27c11532ab…`; restore materialises them to `.pixi-sandbox/vendor/`. Built with cargo 1.98.1 (797e8a9bc 2026-08-05); rustc 1.98.1 (48a229cea 2026-09-01).

## Restore on the disconnected machine

```bash
./.pixi-sandbox/tools/linux-64/pixi-sandbox doctor --branch-location . --verify
./.pixi-sandbox/tools/linux-64/pixi-sandbox restore --branch-location . --output-path <project> --force
# then, from <project> with no network, use pixi as the sole entrypoint:
pixi install --frozen --offline
pixi run --frozen -- cargo build --offline
```

Every manifest blob is verified before it is written into the working tree.
