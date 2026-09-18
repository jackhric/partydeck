# Packaging

Release builds target SteamOS (Holo). Build the overlay page on the host first, then build the rest in the Holo container:

    make -C overlay ui
    docker build -t partydeck-build -f packaging/Dockerfile .
    docker run --rm -v "$PWD:/workspace" partydeck-build bash packaging/scripts/build_all.sh

Native (any distro with rust, pnpm, meson, ninja, 7z and gamescope's build deps): `make -C overlay ui && packaging/scripts/build_all.sh`. Output: `build/holo/release/` and `build/appimage/`.

| Script | Does |
|---|---|
| `scripts/fetch_deps.sh [--latest]` | Downloads pinned Goldberg (linux + win) and UMU into `deps/releases/`, sha256 checked. |
| `scripts/install_deps.sh <out> [--link]` | Copies (or symlinks) those into `<out>/bin/umu-run` and `<out>/res/goldberg/`. |
| `scripts/build_gamescope.sh <out>` | Applies `deps/deps.patch`, meson+ninja builds `deps/gamescope` into `build/gamescope`, installs `bin/gamescope-kbm`. |
| `scripts/build_dist.sh` | fetch_deps, `cargo build --release`, the two above, `make -C overlay package`; assembles `build/holo/release/`. |
| `scripts/build_appimage.sh` | Wraps that skeleton into an AppImage with sharun. `build_all.sh` runs both. |
| `scripts/dev.sh` | Fetches deps and symlinks them into `target/debug/` so `cargo run -p partydeck` works locally. |

`holo-deps.txt` is the single pacman package list read by the Dockerfile and by `.github/actions/build-partydeck`.
