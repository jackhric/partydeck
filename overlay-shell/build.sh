#!/bin/sh
# Build the CEF overlay shell against an extracted CEF binary distribution.
# Usage: CEF_ROOT=/path/to/cef_binary_… ./build.sh [out-dir]
# The Deck resolves libcef's deps at runtime, the build host does not
# (hence --allow-shlib-undefined).
set -eu
CEF_ROOT="${CEF_ROOT:?path to extracted CEF binary distribution}"
SRC_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
OUT_DIR="${1:-$SRC_DIR/build}"
mkdir -p "$OUT_DIR"
XDG_XML=/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml
wayland-scanner client-header "$XDG_XML" "$OUT_DIR/xdg-shell-client-protocol.h"
wayland-scanner private-code "$XDG_XML" "$OUT_DIR/xdg-shell-protocol.c"
gcc -O2 -o "$OUT_DIR/cef-shell" "$SRC_DIR/cef-shell.c" "$OUT_DIR/xdg-shell-protocol.c" \
    -I"$OUT_DIR" -I"$CEF_ROOT" -L"$CEF_ROOT/Release" -lcef -lwayland-client \
    -Wl,--allow-shlib-undefined -Wl,-rpath,'$ORIGIN'
echo "built: $OUT_DIR/cef-shell (deploy next to libcef.so + merged Resources)"
