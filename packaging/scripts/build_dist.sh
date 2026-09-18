#!/usr/bin/env bash
# Build the release skeleton (on-device layout) into build/$BUILD_NAME/release/.
# Run from the repo root, normally inside the Holo container (packaging/Dockerfile).
# overlay/ui/dist/overlay.html must already exist (`make -C overlay ui`, host only).
set -euo pipefail

BUILD_NAME="${BUILD_NAME:-holo}"
BUILD_DIR="${BUILD_DIR:-build/$BUILD_NAME}"
case "$BUILD_DIR" in /*) ;; *) BUILD_DIR="$PWD/$BUILD_DIR" ;; esac
RELEASE_DIR="$BUILD_DIR/release"

test -f overlay/ui/dist/overlay.html || {
    echo "overlay/ui/dist/overlay.html missing: run 'make -C overlay ui' first" >&2
    exit 1
}
SCRIPTS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Cargo caches beside the output so a bind-mounted /workspace persists them.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$BUILD_DIR/target}"
export CARGO_HOME="${CARGO_HOME:-$BUILD_DIR/home}"

"$SCRIPTS/fetch_deps.sh"
cargo build --release -p partydeck -p partydeck-comp

rm -rf "$RELEASE_DIR"
mkdir -p "$RELEASE_DIR/bin" "$RELEASE_DIR/res"

cp "$CARGO_TARGET_DIR/release/partydeck" "$RELEASE_DIR/partydeck"

# partydeck-comp must only link libraries stock SteamOS ships; a stray NEEDED
# entry (e.g. libseat, libdisplay-info) means a feature crept in and the binary
# will not load on the Deck.
for lib in $(objdump -p "$CARGO_TARGET_DIR/release/partydeck-comp" | awk '/NEEDED/{print $2}'); do
    case "$lib" in
        libxkbcommon.so.*|libgcc_s.so.*|libm.so.*|libc.so.*|ld-linux-*.so.*) ;;
        *) echo "ERROR: partydeck-comp links unexpected library: $lib" >&2; exit 1 ;;
    esac
done
cp "$CARGO_TARGET_DIR/release/partydeck-comp" "$RELEASE_DIR/bin/partydeck-comp"

"$SCRIPTS/install_deps.sh" "$RELEASE_DIR"         # bin/umu-run, res/goldberg/...
"$SCRIPTS/build_gamescope.sh" "$RELEASE_DIR"      # bin/gamescope-kbm, bin/gamescopereaper
make -C overlay package OUT="$RELEASE_DIR/bin/cef-overlay"

cp -r res/. "$RELEASE_DIR/res/"                   # runtime data (avatars)
cp packaging/steamos/GamingModeLauncher.sh "$RELEASE_DIR/GamingModeLauncher.sh"
cp LICENSE                  "$RELEASE_DIR/LICENSE"
cp THIRD_PARTY_LICENSES.md  "$RELEASE_DIR/thirdparty.txt"

echo "Release skeleton: $RELEASE_DIR"
