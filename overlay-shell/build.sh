#!/bin/sh
# Build the CEF overlay shell against an extracted CEF binary distribution.
# Usage: CEF_ROOT=/path/to/cef_binary_… ./build.sh
# Run in the holo image; the Deck resolves libcef's deps at runtime, the
# build image does not (hence --allow-shlib-undefined).
set -eu
CEF_ROOT="${CEF_ROOT:?path to extracted CEF binary distribution}"
XDG_XML=/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml
wayland-scanner client-header "$XDG_XML" xdg-shell-client-protocol.h
wayland-scanner private-code "$XDG_XML" xdg-shell-protocol.c
gcc -O2 -o cef-shell cef-shell.c xdg-shell-protocol.c \
    -I. -I"$CEF_ROOT" -L"$CEF_ROOT/Release" -lcef -lwayland-client \
    -Wl,--allow-shlib-undefined -Wl,-rpath,'$ORIGIN'
echo "built: cef-shell (deploy next to libcef.so + merged Resources)"
