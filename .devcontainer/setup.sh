#!/usr/bin/env bash
# Provision the devcontainer for the non-root `vscode` user.
#
# The base image (`ghcr.io/prefix-dev/pixi`) is Ubuntu plus the pixi binary and
# nothing else: no `git`, no C compiler. Everything a contributor or an agent
# needs is installed here, in the order that makes each step's dependency
# available to the next.
#
#   1. host tools    — git/gh (Source Control, lefthook, scripts/restore.sh) and a
#                      C toolchain (blake3-sys shells out to `cc`/`ar` at build
#                      time, so without it `cargo build` cannot even compile).
#   2. environments  — materialise pixi.toml/pixi.lock.
#   3. bun workspace — install turbo/biome/backlog/skills so `pixi run` has them.
#   4. opencode      — the agent CLI, installed last because it needs bun from the
#                      materialised environment.
set -euo pipefail

# ── 1. host tools ───────────────────────────────────────────────────────────
# `pixi global` installs into /root/.pixi/bin, which the image already has on
# PATH, so these are ordinary commands in every shell of the container.
pixi g i git gh

# ── 2. project environments ─────────────────────────────────────────────────
# `--locked` fails rather than re-solving, so the container cannot drift from the
# `pixi.lock` the sandbox branch is built from.
pixi install --locked --all

# ── 3. bun workspace ────────────────────────────────────────────────────────
# `bun` lives in the materialised environment, which is on PATH inside a
# `pixi run` task and nowhere else — so this goes through a task, not a bare
# `bun install`. Root package.json declares the workspace, so this is one
# install for packages/* and benches/* and hoists into the root node_modules.
pixi run bun-install

# ── 4. opencode ─────────────────────────────────────────────────────────────
# OpenCode is an npm package and the devcontainer has no Node.js, so it is
# installed with bun. Installing into BUN_INSTALL rather than the pixi prefix
# keeps `opencode` on PATH for ordinary shells, not just `pixi run` ones.
# bashrc is the right file for an interactive login-less devcontainer shell;
# bash_profile would be skipped by shells started without a login flag.
export BUN_INSTALL="$HOME/.bun"
mkdir -p "$BUN_INSTALL/bin"

# Keep the CLI reproducible. Bump deliberately; the model catalogue below is
# refreshed independently of the installed CLI version.
pixi run -e default env BUN_INSTALL="$BUN_INSTALL" \
  bun install --global --no-audit --no-fund opencode-ai@1.18.32

grep -q 'HOME/.bun/bin' "$HOME/.bashrc" 2>/dev/null ||
  printf '\nexport PATH="$HOME/.bun/bin:$PATH"\n' >>"$HOME/.bashrc"
export PATH="$BUN_INSTALL/bin:$PATH"

opencode --version

# Refreshing needs access to the model catalogue, not provider credentials.
# OpenCode may fall back to its bundled list even when the fetch fails, so don't
# claim a successful download merely because the command exits successfully.
# Suppress its own "Models cache refreshed" banner, which appears on fallback.
if opencode models --refresh >/dev/null 2>&1; then
  echo "OpenCode model refresh attempted; rerun 'opencode models --refresh' if offline."
else
  echo "OpenCode model refresh unavailable; rerun 'opencode models --refresh' when online." >&2
fi
