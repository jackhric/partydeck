# Builds both release artifacts (skeleton + AppImage) against SteamOS Holo glibc.
#   Build:    docker build --build-arg UID=$(id -u) --build-arg GID=$(id -g) -t partydeck-build -f dist/Dockerfile .
#   Skeleton: docker run --rm -v "$PWD:/workspace" partydeck-build
#   + AppImage: docker run --rm -v "$PWD:/workspace" partydeck-build bash dist/scripts/build_all.sh

# Pinned for reproducibility / security
FROM ghcr.io/steamdeckhomebrew/holo-toolchain-rust@sha256:818de45e147f35798c66498380b31bd1fe8bf75afdf0a22b027053706836faf1 AS base

# Image sets RUSTUP_HOME/CARGO_HOME=/.rustup,/.cargo; put cargo on PATH.
ENV RUSTUP_HOME=/.rustup \
    PATH=/.cargo/bin:$PATH

# libpipewire is reinstalled with --overwrite: the base image skips its headers on a file conflict.
RUN pacman -Sy --noconfirm --needed \
        git meson clang glslang libcap sdl2 vulkan-headers \
        libx11 libxmu libxcomposite libxrender libxres libxtst libxkbcommon \
        libdrm libinput wayland-protocols benchmark xorg-xwayland pipewire \
        lcms2 hwdata seatd xcb-util-wm xcb-util-errors \
        cmake libavif libheif aom rav1e libdecor libxdamage luajit libva \
        libxss wget curl xorg-server-xvfb fuse-overlayfs bubblewrap zsync \
        pulseaudio pipewire-audio alsa-plugins libxkbcommon-x11 libvdpau \
        libvdpau-va-gl sudo pkgconf openssl zip base-devel \
    && pacman -S --noconfirm --overwrite '*' libpipewire \
    && yes | pacman -Scc

# MUST remove: if present, gamescope dynamically links gamescope-kbm against the
# base image's libdisplay-info.so.2, but the Deck only ships .so.3, so the binary
# fails to load and KBM games crash. Without it, meson statically links its
# vendored copy. (Real pacman — the COPY'd shim below intercepts later.)
RUN /usr/bin/pacman -Rdd --noconfirm libdisplay-info 2>/dev/null || true

# Lets build_appimage.sh's debloated packages install here: see dist/scripts/pacman_shim.sh.
COPY dist/scripts/pacman_shim.sh /usr/local/bin/pacman
RUN chmod +x /usr/local/bin/pacman

FROM base AS runtime

ARG UID=1000
ARG GID=1000

RUN groupadd -g "$GID" appgroup 2>/dev/null || true; \
    useradd -m -u "$UID" -g "$GID" appuser 2>/dev/null || true; \
    usermod -aG wheel appuser; \
    echo "%wheel ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/wheel; \
    chmod 0440 /etc/sudoers.d/wheel; \
    chown -R "$UID":"$GID" /.cargo /.rustup

USER appuser
WORKDIR /workspace

ENV BUILD_NAME=holo

# Default: skeleton only. Override the command to also build the AppImage.
CMD ["bash", "dist/scripts/build_dist.sh"]
