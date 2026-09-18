#!/usr/bin/env bash
# Local dev setup: fetch the third-party deps and link them next to the debug
# binary so `cargo run -p partydeck` finds bin/ and res/ like a release does.
#   packaging/scripts/dev.sh [profile]   (default: debug)
# gamescope-kbm is taken from PATH; to ship the fork instead run
# packaging/scripts/build_gamescope.sh target/<profile>.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PROFILE="${1:-debug}"
OUT="${CARGO_TARGET_DIR:-$REPO_ROOT/target}/$PROFILE"

"$REPO_ROOT/packaging/scripts/fetch_deps.sh"
"$REPO_ROOT/packaging/scripts/install_deps.sh" "$OUT" --link
mkdir -p "$OUT/bin" "$OUT/res"
ln -sfn "$REPO_ROOT/res/avatars" "$OUT/res/avatars"
ln -sfn ../partydeck-comp "$OUT/bin/partydeck-comp"   # cargo builds it next to partydeck
echo "dev: linked deps into $OUT; now: cargo build --workspace && cargo run -p partydeck"
