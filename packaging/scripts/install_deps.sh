#!/usr/bin/env bash
# Place the fetched third-party files (see fetch_deps.sh) where the partydeck
# binary looks for them: <out>/bin/umu-run and <out>/res/goldberg/... (paths.rs:
# exe-adjacent bin/ and res/). Used by build_dist.sh (copy) and dev.sh (--link).
#   packaging/scripts/install_deps.sh <out> [--link]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RELEASES="$REPO_ROOT/deps/releases"
OUT="${1:?usage: install_deps.sh <out> [--link]}"
MODE="${2:-}"

# source (relative to deps/releases) -> destination (relative to <out>)
FILES=(
    "gbe-linux/regular/x64/steamclient.so|res/goldberg/linux64/steamclient.so"
    "gbe-linux/regular/x86/steamclient.so|res/goldberg/linux32/steamclient.so"
    "gbe-win/steamclient_experimental/steamclient.dll|res/goldberg/win/steamclient.dll"
    "gbe-win/steamclient_experimental/steamclient64.dll|res/goldberg/win/steamclient64.dll"
    "gbe-win/steamclient_experimental/GameOverlayRenderer.dll|res/goldberg/win/GameOverlayRenderer.dll"
    "gbe-win/steamclient_experimental/GameOverlayRenderer64.dll|res/goldberg/win/GameOverlayRenderer64.dll"
    "umu/umu-run|bin/umu-run"
)

for pair in "${FILES[@]}"; do
    src="$RELEASES/${pair%%|*}"
    dst="$OUT/${pair##*|}"
    [ -e "$src" ] || { echo "install_deps.sh: missing $src (run packaging/scripts/fetch_deps.sh)" >&2; exit 1; }
    mkdir -p "$(dirname "$dst")"
    rm -f "$dst"
    if [ "$MODE" = "--link" ]; then ln -s "$src" "$dst"; else cp "$src" "$dst"; fi
done
chmod +x "$OUT/bin/umu-run" 2>/dev/null || true
