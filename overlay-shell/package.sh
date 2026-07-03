#!/bin/sh
# Assemble the minimal CEF runtime + cef-shell into <output-dir>.
# Downloads/extracts/strips into dist/build_generated/cef/ once and reuses it.
# Usage: package.sh <output-dir>
set -eu

OUT_DIR="${1:?usage: package.sh <output-dir>}"

CEF_VERSION="144.0.29+g0b1a012+chromium-144.0.7559.256"
CEF_URL="https://cef-builds.spotifycdn.com/cef_binary_$(printf %s "$CEF_VERSION" | sed 's/+/%2B/g')_linux64_minimal.tar.bz2"
CEF_SHA256="aafa3b996b748119920c9d0d7a7e3af8ffa2b9a49654e67dd7681b3cb6395a09"

SRC_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
CACHE_DIR="$SRC_DIR/../dist/build_generated/cef"
TARBALL="$CACHE_DIR/cef.tar.bz2"
DIST_DIR="$CACHE_DIR/dist"
STRIPPED="$CACHE_DIR/libcef-stripped.so"

mkdir -p "$CACHE_DIR"

checksum_ok() {
    printf '%s  %s\n' "$CEF_SHA256" "$TARBALL" | sha256sum -c --status
}

if [ ! -f "$TARBALL" ] || ! checksum_ok; then
    curl -fL -o "$TARBALL" "$CEF_URL"
    checksum_ok || { echo "ERROR: cef.tar.bz2 sha256 mismatch" >&2; exit 1; }
fi

if [ ! -e "$DIST_DIR/.extracted" ]; then
    rm -rf "$DIST_DIR"
    mkdir -p "$DIST_DIR"
    tar -xjf "$TARBALL" -C "$DIST_DIR" --strip-components=1
    touch "$DIST_DIR/.extracted"
fi

LIBCEF="$DIST_DIR/Release/libcef.so"
if [ ! -f "$STRIPPED" ] || [ "$LIBCEF" -nt "$STRIPPED" ]; then
    strip -o "$STRIPPED" "$LIBCEF"
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
CEF_ROOT="$DIST_DIR" sh "$SRC_DIR/build.sh" "$WORK"

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/locales"
cp "$WORK/cef-shell"    "$OUT_DIR/cef-shell"
cp "$STRIPPED"          "$OUT_DIR/libcef.so"
cp "$SRC_DIR/overlay.html" "$OUT_DIR/overlay.html"
cp "$DIST_DIR/Release/v8_context_snapshot.bin" \
   "$DIST_DIR/Resources/icudtl.dat" \
   "$DIST_DIR/Resources/resources.pak" \
   "$DIST_DIR/Resources/chrome_100_percent.pak" \
   "$DIST_DIR/Resources/chrome_200_percent.pak" \
   "$OUT_DIR/"
cp "$DIST_DIR/Resources/locales/en-US.pak" "$OUT_DIR/locales/en-US.pak"
cp "$DIST_DIR/LICENSE.txt" "$OUT_DIR/LICENSE.cef.txt"
# Excluded on purpose: libEGL/libGLESv2/libvk_swiftshader/libvulkan (we run
# --disable-gpu), chrome-sandbox (--no-sandbox), the 219 other locales.

du -sh "$OUT_DIR"
