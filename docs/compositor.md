# partydeck-comp

A nested Wayland compositor (smithay) that tiles per-player nested gamescope
instances inside one host window. PartyDeck runs it under the outer gamescope
session and points each game's gamescope at one of its per-player sockets.

## Crate layout

- `crates/comp-proto/` (`partydeck-comp-proto`): the wire format shared by the
  compositor, the launcher and the overlay UI. Layout files, presets, control
  commands and the typed session `State` document. The launcher depends on
  this, never on the compositor itself. `cargo test -p partydeck-comp-proto`
  also regenerates `overlay/ui/src/generated/proto.ts` (ts-rs, `ts` feature,
  on by default).
- `crates/partydeck-comp/`: the compositor binary.
  - `src/main.rs`: CLI args and wiring.
  - `src/state.rs`: `CompState`.
  - `src/wayland/`: sockets and client tagging, smithay handlers, popups.
  - `src/ipc/`: control socket, command dispatch, `State` document builder.
  - `src/layout/`: slot mapping and the contain-fit rule.
  - `src/render/`: redraw, presentation feedback, optional telemetry
    (`--features telemetry`: frame CSV log via `PARTYDECK_COMP_FRAME_LOG`, fps
    counters, `PARTYDECK_COMP_LEGACY_ACK`).
  - `src/input/`: keyboard and pointer forwarding.
  - `src/backend/`: winit output and Steam app id lookup.

## Running standalone (desktop)

```
cargo run -p partydeck-comp -- --players 2 --size 1280x800 --border faint
```

Then connect any Wayland client to a player socket:

```
WAYLAND_DISPLAY=partydeck-comp-p0 weston-info
```

## Sockets

With `--socket-prefix <prefix>` (default `partydeck-comp`):

- `<prefix>-p0` .. `<prefix>-pN-1`: one Wayland socket per player slot; a
  client's toplevel is tiled into that slot.
- `<prefix>-overlay`: Wayland socket for the HTML overlay client, composited
  above the slots.
- `<prefix>.ctl`: Unix control socket.

## Control protocol

One ndjson command per connection; the compositor reads one line (300 ms
total deadline, 4 MiB max), replies with one line and closes. Types live in
`partydeck_comp_proto::ipc` (`Command`, `Response`) and
`partydeck_comp_proto::state`.

- `{"cmd":"set_slot_status","slot":0,"status":"loading","label":"player 1","avatar":"<base64 png>","logo":"<base64 png>"}`
  `status` is `"loading"` or `"running"`; `label`, `avatar` and `logo` are optional.
  Reply: `{"ok":true}` or `{"ok":false,"error":"..."}`.
- `{"cmd":"quit"}`. Reply: `{"ok":true}`.
- `{"cmd":"get_state"}`. Reply: the `State` document below instead of ok/err.

`State` (`Response::State`):

```json
{"proto":1,
 "size":{"w":1280,"h":800},
 "focus":0,
 "border":"faint",
 "slots":[
   {"rect":{"x":0,"y":0,"w":640,"h":800},"live":true,"status":"loading",
    "label":"player 1","avatar":null,"logo":null,"controller_disconnected":false}
 ]}
```

`proto` is `partydeck_comp_proto::PROTOCOL_VERSION`. `border` is one of
`off`, `faint`, `medium`, `strong` (the `--border` flag). `controller_disconnected`
is always `false` for now; it stays in the document for the overlay UI.

## Layout JSON

Rects are fractions of the output; `slots[k]` is player k's region; `focus` is
the slot index that receives keyboard input. `Layout::read` / `Layout::write`
in the proto crate handle the file.

```json
{"version":1,"focus":0,"slots":[
  {"rect":{"x":0.0,"y":0.0,"w":0.5,"h":1.0}},
  {"rect":{"x":0.5,"y":0.0,"w":0.5,"h":1.0}}
]}
```

## Pacing rules (do not "simplify")

The compositor drives rendering from its own calloop timer, never from host
frame callbacks, which stall when the host window is occluded and deadlock the
nested clients. `wp_presentation` must stay advertised: gamescope requires it
and exits without it. Toplevels are activated on map, otherwise nested
gamescope keeps its game paused/unfocused. Render failures (bind, render,
submit) log with the `[comp]` prefix and skip the frame; they never abort.
