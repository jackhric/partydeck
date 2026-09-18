#!/usr/bin/env bash
# Build the keyboard/mouse gamescope fork from the deps/gamescope submodule and
# install it as <out>/bin/gamescope-kbm (+ gamescopereaper).
#   packaging/scripts/build_gamescope.sh <out>
# Applies deps/deps.patch once (skipped when already applied), configures meson
# into build/gamescope on first run, then runs ninja (incremental afterwards).
# Needs: git, meson, ninja, gcc/g++ and gamescope's build deps (packaging/holo-deps.txt).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="${1:?usage: build_gamescope.sh <out>}"
SRC="$REPO_ROOT/deps/gamescope"
PATCH="$REPO_ROOT/deps/deps.patch"
BUILD_DIR="$REPO_ROOT/build/gamescope"

[ -f "$SRC/meson.build" ] || {
    echo "build_gamescope.sh: $SRC is empty; run: git submodule update --init --recursive" >&2
    exit 1
}

# deps.patch is written against the superproject (a/deps/gamescope/...), so
# strip three components to apply it from inside the submodule.
if git -C "$SRC" apply --check --reverse -p3 "$PATCH" 2>/dev/null; then
    echo "build_gamescope: deps.patch already applied"
else
    git -C "$SRC" apply --check -p3 "$PATCH"
    git -C "$SRC" apply -p3 "$PATCH"
    echo "build_gamescope: applied deps.patch"
fi

if [ ! -f "$BUILD_DIR/build.ninja" ]; then
    # PipeWire screen capture is unused by PartyDeck; disabling it keeps the
    # build working on-device.
    CC=gcc CXX=g++ meson setup "$BUILD_DIR" "$SRC" \
        -Dinput_emulation=disabled \
        -Dbenchmark=disabled \
        -Dpipewire=disabled \
        --auto-features=enabled
fi
ninja -C "$BUILD_DIR"

install -Dm755 "$BUILD_DIR/src/gamescope"       "$OUT/bin/gamescope-kbm"
install -Dm755 "$BUILD_DIR/src/gamescopereaper" "$OUT/bin/gamescopereaper"
echo "build_gamescope: installed $OUT/bin/gamescope-kbm"
