#!/usr/bin/env bash
# Download the third-party runtime pieces PartyDeck ships (Goldberg Steam Emu
# for Linux and Windows, UMU Launcher) into deps/releases/{gbe-linux,gbe-win,umu}.
#
#   packaging/scripts/fetch_deps.sh            # pinned URLs, sha256 verified
#   packaging/scripts/fetch_deps.sh --latest   # newest GitHub release of each
#
# Idempotent: a dependency whose marker file already exists is skipped, so
# delete deps/releases/<name> to refetch one. `--latest` needs curl + jq and
# skips the checksum (there is nothing pinned to check against); without jq it
# is not supported and the script exits with an error.
# Needs: curl, sha256sum, tar (bzip2), and 7z/7za or bsdtar for the .7z.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RELEASES="$REPO_ROOT/deps/releases"

LATEST=0
case "${1:-}" in
    --latest) LATEST=1 ;;
    "") ;;
    -h|--help) sed -n '2,13p' "$0"; exit 0 ;;
    *) echo "fetch_deps.sh: unknown argument '$1' (try --latest)" >&2; exit 2 ;;
esac

# repo | asset name substring | pinned url | pinned sha256 | marker | archive top dir to rename (or -)
DEPS=(
    "Detanup01/gbe_fork|emu-linux-release.tar.bz2|https://github.com/Detanup01/gbe_fork/releases/download/release-2026_05_30/emu-linux-release.tar.bz2|113cf4f0f44ac10285eb03df82148f288a27ddd685470c33060e968f83e97d87|gbe-linux/regular/x64/steamclient.so|release:gbe-linux"
    "Detanup01/gbe_fork|emu-win-release.7z|https://github.com/Detanup01/gbe_fork/releases/download/release-2026_05_30/emu-win-release.7z|38d0ce822f78f5b22dd28d948f4b1c98bc65f5fc3a850b7775286743a60e3516|gbe-win/steamclient_experimental/steamclient.dll|release:gbe-win"
    "Open-Wine-Components/umu-launcher|umu-launcher-|https://github.com/Open-Wine-Components/umu-launcher/releases/download/1.3.0/umu-launcher-1.3.0-zipapp.tar|36502de766f3cc549ff85196a04fb5afdb4eb2a72c023f22fd25895df91fda2f|umu/umu-run|-"
)

need() { command -v "$1" >/dev/null 2>&1 || { echo "fetch_deps.sh: missing tool: $1" >&2; exit 1; }; }
need curl; need sha256sum; need tar

latest_asset_url() { # repo, name substring -> browser_download_url
    need jq
    curl -fsSL -H 'User-Agent: partydeck-build' \
        "https://api.github.com/repos/$1/releases/latest" \
        | jq -er --arg n "$2" '.assets[] | select(.name | contains($n)) | .browser_download_url' \
        | head -n1
}

extract() { # archive, destdir
    case "$1" in
        *.tar.bz2) tar -xjf "$1" -C "$2" ;;
        *.tar)     tar -xf  "$1" -C "$2" ;;
        *.7z)
            if command -v 7z >/dev/null 2>&1;      then 7z x -y -o"$2" "$1" >/dev/null
            elif command -v 7za >/dev/null 2>&1;   then 7za x -y -o"$2" "$1" >/dev/null
            elif command -v bsdtar >/dev/null 2>&1; then bsdtar -xf "$1" -C "$2"
            else echo "fetch_deps.sh: need 7z, 7za or bsdtar to extract $1" >&2; exit 1
            fi ;;
        *) echo "fetch_deps.sh: unknown archive type: $1" >&2; exit 1 ;;
    esac
}

mkdir -p "$RELEASES"
for entry in "${DEPS[@]}"; do
    IFS='|' read -r repo asset url sha marker rename <<<"$entry"
    if [ -e "$RELEASES/$marker" ]; then
        echo "fetch_deps: $marker present, skipping $asset"
        continue
    fi

    if [ "$LATEST" = 1 ]; then
        url="$(latest_asset_url "$repo" "$asset")" || { echo "fetch_deps.sh: no asset matching '$asset' in $repo" >&2; exit 1; }
        sha=""
    fi
    archive="$RELEASES/${url##*/}"
    echo "fetch_deps: downloading $url"
    curl -fL --retry 3 -o "$archive" "$url"
    if [ -n "$sha" ]; then
        echo "$sha  $archive" | sha256sum -c --quiet
    else
        echo "fetch_deps: --latest, checksum not verified for $archive"
    fi

    # Stage in a scratch dir so a half-extracted archive never leaves a marker.
    stage="$(mktemp -d "$RELEASES/.extract.XXXXXX")"
    # shellcheck disable=SC2064
    trap "rm -rf '$stage' '$archive'" EXIT
    extract "$archive" "$stage"
    if [ "$rename" != "-" ]; then
        from="${rename%%:*}"; to="${rename##*:}"
        rm -rf "${RELEASES:?}/$to"
        mv "$stage/$from" "$RELEASES/$to"
    else
        for d in "$stage"/*; do rm -rf "${RELEASES:?}/$(basename "$d")"; mv "$d" "$RELEASES/"; done
    fi
    rm -rf "$stage" "$archive"
    trap - EXIT
    [ -e "$RELEASES/$marker" ] || { echo "fetch_deps.sh: $asset extracted but $marker is missing" >&2; exit 1; }
done
echo "fetch_deps: done ($RELEASES)"
