# partydeck-comp

A nested Wayland compositor (smithay) that tiles per-player nested gamescope
instances inside one host window. PartyDeck runs it under the outer gamescope
session and points each game's gamescope at one of its per-player sockets.

## Crate layout

- `proto/` — `partydeck-comp-proto`: the shared schema (layout, presets, ipc
  wire types). Dependency-light (serde only); the partydeck app depends on this,
  never on the compositor itself.
- `src/` — the compositor binary.

## Running standalone (desktop)

```
cargo run -p partydeck-comp -- --players 2 --size 1280x800
```

Then connect any Wayland client to a player socket:

```
WAYLAND_DISPLAY=partydeck-comp-p0 weston-info
```

## Sockets

With `--socket-prefix <prefix>` (default `partydeck-comp`):

- `<prefix>-p0` .. `<prefix>-pN-1` — one Wayland socket per player slot; a
  client's toplevel is tiled into that slot.
- `<prefix>-overlay` — Wayland socket for the HTML overlay client, composited
  above the slots.
- `<prefix>.ctl` — Unix control socket.

## Control protocol

One ndjson command per connection; the compositor replies with one line
(`{"ok":true}` / `{"ok":false,"error":"..."}`) and closes.

- `{"cmd":"ping"}`
- `{"cmd":"set_layout","layout":{...}}`
- `{"cmd":"set_focus","slot":1}`
- `{"cmd":"set_slot_status","slot":0,"status":"loading","label":"player 1"}`
- `{"cmd":"quit"}`
- `{"cmd":"get_state"}` — replies with a state document instead of ok/err:
  output size, focus, and per-slot pixel rect / liveness / status.

## Layout JSON

Rects are fractions of the output; `slots[k]` is player k's region; `focus` is
the slot index that receives keyboard/pointer input.

```json
{"version":1,"focus":0,"slots":[
  {"rect":{"x":0.0,"y":0.0,"w":0.5,"h":1.0}},
  {"rect":{"x":0.5,"y":0.0,"w":0.5,"h":1.0}}
]}
```

## Pacing rules (do not "simplify")

The compositor drives rendering from its own calloop timer — never from host
frame callbacks, which stall when the host window is occluded and deadlock the
nested clients. `wp_presentation` must stay advertised: gamescope requires it
and exits without it. Toplevels are activated on map, otherwise nested
gamescope keeps its game paused/unfocused.
