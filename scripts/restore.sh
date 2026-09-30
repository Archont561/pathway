#!/bin/sh
set -eu
ROOT=$(git rev-parse --show-toplevel)
case $(uname -s)-$(uname -m) in
  Linux-x86_64) PLATFORM=linux-64 ;;
  Linux-aarch64|Linux-arm64) PLATFORM=linux-aarch64 ;;
  Darwin-arm64) PLATFORM=osx-arm64 ;;
  Darwin-x86_64) PLATFORM=osx-64 ;;
  *) echo "unsupported airlock platform: $(uname -s)-$(uname -m)" >&2; exit 2 ;;
esac
DEFAULT_BRANCH=sandbox/developer-linux-64
case $DEFAULT_BRANCH in *linux-64) DEFAULT_BRANCH=${DEFAULT_BRANCH%linux-64}$PLATFORM ;; esac
BRANCH=${PIXI_SANDBOX_BRANCH:-}
CONFIG=$ROOT/.pixi-sandbox.toml
if [ -z "$BRANCH" ] && [ -r "$CONFIG" ]; then
  # <branch_prefix>/<bundle>-<platform>, read off the same reviewed plan the publisher uses.
  PREFIX=$(sed -n "s/^[[:space:]]*branch_prefix[[:space:]]*=[[:space:]]*[\"']\([^\"']*\).*/\1/p" "$CONFIG" | sed 1q)
  BUNDLES=$(tr '\n' ' ' <"$CONFIG" | sed 's/\[\[[[:space:]]*bundle[[:space:]]*\]\]/\
/g' | grep -E "platforms[^]]*[\"']$PLATFORM[\"']" |
    sed -n "s/.*name[[:space:]]*=[[:space:]]*[\"']\([^\"']*\).*/\1/p" || true)
  if [ -n "${PIXI_SANDBOX_BUNDLE:-}" ]; then
    BUNDLES=$(printf '%s\n' "$BUNDLES" | grep -Fx "$PIXI_SANDBOX_BUNDLE" || true)
  fi
  if [ "$(printf '%s' "$BUNDLES" | grep -c . || true)" = 1 ]; then
    BRANCH=${PREFIX:-sandbox}/$BUNDLES-$PLATFORM
  elif [ -n "$BUNDLES" ]; then
    echo "several bundles publish $PLATFORM; set PIXI_SANDBOX_BUNDLE to choose" >&2
  fi
fi
BRANCH=${BRANCH:-$DEFAULT_BRANCH}
if ! git -C "$ROOT" rev-parse --verify "$BRANCH^{commit}" >/dev/null 2>&1; then BRANCH=origin/$BRANCH; fi
TRANSPORT="$ROOT/.pixi/.restore-transport"
rm -rf "$TRANSPORT"
mkdir -p "$TRANSPORT"
git -C "$ROOT" archive "$BRANCH" | tar -x -C "$TRANSPORT"
BIN="$TRANSPORT/.pixi-sandbox/tools/$PLATFORM/pixi-sandbox"
exec "$BIN" restore --branch-location "$TRANSPORT" --output-path "$ROOT" --force "$@"
