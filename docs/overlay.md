# cef-overlay

Transparent HUD overlay for PartyDeck sessions. A small C shell runs CEF in
off-screen rendering mode and blits the BGRA frames into a wl_shm surface on
partydeck-comp's `<prefix>-overlay` Wayland socket (OSR is the only way to get
per-pixel alpha out of CEF on Wayland). Session state is polled from the
compositor's control socket and handed to the page as raw JSON via
`window.__pdState(<state>)`; the page owns all presentation logic.

## Layout

- `overlay/src/` is the C shell: `main.c` (CEF glue, entry point), `wl_shm.c`
  (Wayland surface, double buffering, resize), `ctl.c` (control socket client)
  and `overlay.h` (shared declarations and the wire constants). Rarely changes.
- `overlay/ui/` is the Vite + React + TypeScript app for the overlay page. It
  builds to one self-contained `overlay/ui/dist/overlay.html` (everything
  inlined, nothing is fetched from `file://`).
- `overlay/build/` holds the generated xdg-shell sources, the binary and the
  default package output. `build/cef/` at the repo root caches the pinned CEF
  distribution (tarball, extracted dist, stripped `libcef.so`).

## UI workflow

```
pnpm -C overlay/ui install --frozen-lockfile
pnpm -C overlay/ui dev        # http://localhost:5173/ with the dev panel
pnpm -C overlay/ui test       # vitest: layout, avatar colors, state store
pnpm -C overlay/ui build      # tsc --noEmit, then a single-file overlay.html
```

`vite dev` serves the overlay at `/` with a dev sidebar that drives
`window.__pdState` directly: resolution, slot count, border style, per-slot
live/label/avatar/controller state, a scripted loading timeline, and a font
switcher (the @fontsource faces are dev-only and never enter the production
bundle). Append `?nopanel` to hide the panel. Everything under
`overlay/ui/src/dev/` is excluded from the production build.

`overlay/ui/src/state.ts` holds the wire types (`PdState`, `PdSlot`, `PdRect`),
re-exported from `overlay/ui/src/generated/proto.ts`, and the store that
`window.__pdState` feeds. The generated file comes from the types in
`crates/comp-proto/src/state.rs`; regenerate it with
`cargo test -p partydeck-comp-proto --features ts` and never edit it by hand.

## Building the shell and the bundle

- `make -C overlay ui` runs `pnpm install --frozen-lockfile` and `pnpm build`.
  It is host-only: the holo container has no node or pnpm.
- `make -C overlay shell` builds `overlay/build/cef-overlay`. It depends on the
  pinned CEF dist, which the `cef-dist` target downloads, verifies, extracts
  and strips once into `build/cef/` and then reuses.
- `make -C overlay package [OUT=dir]` assembles the runtime bundle (default
  `overlay/build/pkg/`). It copies `overlay/ui/dist/overlay.html` and fails
  loudly if that file is missing, so build the UI on the host before packaging
  inside docker. `packaging/scripts/build_dist.sh` runs this target into the
  release skeleton at `bin/cef-overlay/`.
- `make -C overlay deploy` and `make -C overlay deploy-ui` are conveniences for
  on-device work: package inside the `partydeck-build` image and rsync the
  bundle (or just `overlay.html`) to a Deck. `DECK_HOST` and `DECK_PORT` are
  overridable.

## Runtime bundle

`package` produces: `cef-overlay` (the shell), stripped `libcef.so`,
`overlay.html`, `v8_context_snapshot.bin`, `icudtl.dat`, `resources.pak`,
`chrome_100_percent.pak`, `chrome_200_percent.pak`, `locales/en-US.pak`,
`LICENSE.cef.txt`. Deliberately excluded: libEGL/libGLESv2/swiftshader/vulkan
(we run `--disable-gpu`), `chrome-sandbox` (`--no-sandbox`), and the 219 other
locales.

## Spawn contract

partydeck spawns `bin/cef-overlay/cef-overlay` (relative to the partydeck-comp
binary; override with `PARTYDECK_CEF_OVERLAY`) after the compositor reports
ready, with:

- `WAYLAND_DISPLAY=<prefix>-overlay`, the compositor's overlay socket.
- `PARTYDECK_CTL_SOCKET=<path>`, the compositor's control socket. When unset
  the shell falls back to `$XDG_RUNTIME_DIR/<prefix>.ctl`, derived from
  `WAYLAND_DISPLAY`, so older launchers keep working.
- `OVERLAY_URL`, defaulting to the bundled `overlay.html`.
- `--ozone-platform=headless --disable-gpu --no-sandbox`.

The overlay is optional: no binary means the session runs bare. The shell logs
to stderr, polls `{"cmd":"get_state"}` about four times a second and injects
`window.__pdState(<reply>)` only when the reply changed. It tracks
compositor-assigned sizes at runtime (including per-app resolution changes in
Gaming Mode), reallocating its buffers and re-rastering CEF on each resize, and
exits on its own when the compositor socket goes away or the toplevel is
closed, closing the browser before `cef_shutdown`.
