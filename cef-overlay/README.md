# cef-overlay

Transparent HUD overlay for PartyDeck sessions. A small C shell runs CEF in
off-screen rendering mode and blits the BGRA frames into a wl_shm surface on
partydeck-comp's `<prefix>-overlay` Wayland socket (OSR is the only way to get
per-pixel alpha out of CEF on Wayland). Session state is polled from the
compositor's ctl socket and handed to the page as raw JSON via
`window.__pdState(<state>)` — the page owns all presentation logic.

## Layout

- `src/` — the C shell (`main.c`). Wayland plumbing + CEF glue; rarely changes.
- `ui/` — Vite + React + TypeScript app for the overlay page. Builds to a
  single self-contained `ui/dist/overlay.html` (everything inlined — no
  fetches from `file://`). Iterate freely.
- `build/` — generated protocol sources, binary, default package output.

## Dev loop

| Change              | Command                     | What it does |
|---------------------|-----------------------------|--------------|
| `ui/src/*`          | `make deploy-ui`            | pnpm-build the UI, rsync the HTML to the Deck |
| `src/main.c`        | `make deploy`               | package inside the holo image, rsync the bundle |
| anything else       | `buildbinary` vscode task   | full release skeleton sync |

For UI work off-Deck: `pnpm -C ui dev`, then open
`http://localhost:5173/overlay.html?mock=1` (or `?mock=4` for four slots) —
the mock harness drives `window.__pdState` with a scripted loading timeline.

`make ui` (pnpm install + build) produces `ui/dist/overlay.html`. It is
host-only — the holo container has no node — so **build the UI on the host
before docker packaging**; `package` fails loudly if `ui/dist/overlay.html`
is missing. `sync-partydeck-binary.sh` does this automatically.

`make shell` builds the binary locally; `make package [OUT=dir]` assembles the
runtime bundle. Both need the pinned CEF dist, which `cef-dist` downloads,
extracts, and strips once into `../dist/build_generated/cef/` and reuses.

## Runtime bundle

`package` produces: `cef-overlay` (the shell), stripped `libcef.so`,
`overlay.html`, `v8_context_snapshot.bin`, `icudtl.dat`, `resources.pak`,
`chrome_100_percent.pak`, `chrome_200_percent.pak`, `locales/en-US.pak`,
`LICENSE.cef.txt`. Deliberately excluded: libEGL/libGLESv2/swiftshader/vulkan
(we run `--disable-gpu`), `chrome-sandbox` (`--no-sandbox`), and the 219 other
locales. `dist/scripts/build_dist.sh` runs the same `package` target into the
release skeleton at `bin/cef-overlay/`.

## Spawn contract

partydeck spawns `bin/cef-overlay/cef-overlay` (relative to the partydeck-comp
binary; override with `PARTYDECK_CEF_OVERLAY`) after the compositor reports
ready, with `WAYLAND_DISPLAY=<prefix>-overlay`, `OVERLAY_URL` (defaults to the
bundled `overlay.html`), and `--ozone-platform=headless --disable-gpu
--no-sandbox`. The overlay is optional: no binary means the session runs bare.
The shell tracks compositor-assigned sizes at runtime (including per-app
resolution changes in Gaming Mode), reallocating its buffers and re-rastering
CEF on each resize. It exits on its own when the compositor socket goes away.
