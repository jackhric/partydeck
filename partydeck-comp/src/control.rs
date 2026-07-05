use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use smithay::reexports::calloop::EventLoop;
use smithay::utils::SERIAL_COUNTER;
use smithay::wayland::socket::ListeningSocketSource;

use partydeck_comp_proto::ipc::{Command, Response};

use crate::state::{CompState, SlotStatus};
use crate::{slots, CalloopData};

pub fn init(
    event_loop: &mut EventLoop<CalloopData>,
    socket_prefix: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let name = format!("{socket_prefix}.ctl");
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").map_err(|_| "XDG_RUNTIME_DIR not set")?;
    let path = PathBuf::from(runtime_dir).join(&name);

    let listener = ListeningSocketSource::with_name(&name)?;
    event_loop.handle().insert_source(listener, move |stream, _, data| {
        // One command per connection, handled synchronously: sources inserted
        // mid-dispatch never fire, and the only client is PartyDeck main.
        handle_connection(&stream, &mut data.state);
    })?;
    Ok(path)
}

fn handle_connection(mut stream: &UnixStream, state: &mut CompState) {
    stream
        .set_read_timeout(Some(std::time::Duration::from_millis(300)))
        .ok();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    while !buf.contains(&b'\n') && buf.len() < 64 * 1024 {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
    }
    let Some(pos) = buf.iter().position(|&b| b == b'\n') else {
        return;
    };
    let line = match std::str::from_utf8(&buf[..pos])
        .map_err(|e| e.to_string())
        .and_then(|s| partydeck_comp_proto::ipc::decode_command(s).map_err(|e| e.to_string()))
    {
        Ok(Command::GetState) => format!("{}\n", state_json(state)),
        Ok(cmd) => partydeck_comp_proto::ipc::encode(&apply(state, cmd)).unwrap_or_default(),
        Err(e) => partydeck_comp_proto::ipc::encode(&Response::Err(e)).unwrap_or_default(),
    };
    let _ = stream.write_all(line.as_bytes());
}

fn state_json(state: &CompState) -> String {
    let size = state.backend.winit.window_size();
    let rects = state.layout.resolve(size.w.max(1) as u32, size.h.max(1) as u32);
    let slots: Vec<serde_json::Value> = rects
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let live = state.slot_windows.get(i).map(|w| w.is_some()).unwrap_or(false);
            let entry = state.slot_status.get(i).and_then(|s| s.as_ref());
            let status = entry.map(|s| s.status.clone());
            let label = entry.and_then(|s| s.label.clone());
            let avatar = entry.and_then(|s| s.avatar.clone());
            serde_json::json!({
                "rect": {"x": r.x, "y": r.y, "w": r.w, "h": r.h},
                "live": live,
                "status": status,
                "label": label,
                "avatar": avatar,
            })
        })
        .collect();
    serde_json::json!({
        "size": {"w": size.w, "h": size.h},
        "focus": state.layout.focus,
        "border": state.border_style,
        "slots": slots,
    })
    .to_string()
}

fn apply(state: &mut CompState, cmd: Command) -> Response {
    match cmd {
        // Handled before apply(); a stray arrival is harmless.
        Command::GetState => Response::Ok,
        Command::Ping => Response::Ok,
        Command::Quit => {
            state.loop_signal.stop();
            Response::Ok
        }
        Command::SetFocus { slot } => set_focus(state, slot),
        Command::SetLayout { layout } => {
            if let Err(e) = layout.validate(state.slot_windows.len()) {
                return Response::Err(e);
            }
            state.layout = layout;
            slots::relayout(state);
            set_focus(state, state.layout.focus)
        }
        Command::SetSlotStatus { slot, status, label, avatar } => {
            if slot >= state.slot_status.len() {
                return Response::Err(format!("slot {slot} out of range"));
            }
            state.slot_status[slot] = Some(SlotStatus { status, label, avatar });
            Response::Ok
        }
    }
}

fn set_focus(state: &mut CompState, slot: usize) -> Response {
    let Some(entry) = state.slot_windows.get(slot) else {
        return Response::Err(format!("slot {slot} out of range"));
    };
    state.layout.focus = slot;
    if let Some(surface) = entry
        .as_ref()
        .and_then(|w| w.toplevel().map(|t| t.wl_surface().clone()))
    {
        let keyboard = state.seat.get_keyboard().unwrap();
        keyboard.set_focus(state, Some(surface), SERIAL_COUNTER.next_serial());
    }
    Response::Ok
}
