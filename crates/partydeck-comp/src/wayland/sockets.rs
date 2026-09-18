use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;

use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, LoopHandle, Mode, PostAction};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::{Client, Display, Resource};
use smithay::wayland::compositor::CompositorClientState;
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::wayland::socket::ListeningSocketSource;

use crate::CalloopData;
use crate::state::CompState;

#[derive(Default)]
pub struct ClientState {
    pub slot: Option<usize>,
    pub is_overlay: bool,
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

fn client_data<T>(surface: &ToplevelSurface, f: impl FnOnce(&ClientState) -> T) -> Option<T> {
    let client: Client = surface.wl_surface().client()?;
    client.get_data::<ClientState>().map(f)
}

pub fn slot_of(surface: &ToplevelSurface) -> Option<usize> {
    client_data(surface, |d| d.slot).flatten()
}

pub fn is_overlay(surface: &ToplevelSurface) -> bool {
    client_data(surface, |d| d.is_overlay).unwrap_or(false)
}

pub fn init(
    display: Display<CompState>,
    event_loop: &mut EventLoop<CalloopData>,
    prefix: &str,
    players: usize,
) -> Result<Vec<OsString>, Box<dyn std::error::Error>> {
    let handle = event_loop.handle();
    let mut names = Vec::with_capacity(players + 1);

    for slot in 0..players.max(1) {
        let name = format!("{prefix}-p{slot}");
        names.push(listen(
            &handle,
            &name,
            ClientState {
                slot: Some(slot),
                ..Default::default()
            },
        )?);
    }

    // HUD/menu renderers connect here; their toplevel composites above
    // every slot (see slots::map_toplevel) and they opt out of input via
    // an empty input region client-side.
    let overlay_name = format!("{prefix}-overlay");
    names.push(listen(
        &handle,
        &overlay_name,
        ClientState {
            is_overlay: true,
            ..Default::default()
        },
    )?);

    handle
        .insert_source(
            Generic::new(display, Interest::READ, Mode::Level),
            |_, display, data| {
                // SAFETY: get_mut only requires that the Display (the registered fd)
                // is neither dropped nor replaced while borrowed; we only dispatch.
                let result = unsafe { display.get_mut().dispatch_clients(&mut data.state) };
                if let Err(e) = result {
                    eprintln!("[comp] dispatch_clients failed: {e}");
                }
                Ok(PostAction::Continue)
            },
        )
        .map_err(|e| format!("failed to register wayland display: {e}"))?;

    Ok(names)
}

fn listen(
    handle: &LoopHandle<'_, CalloopData>,
    name: &str,
    client_state: ClientState,
) -> Result<OsString, Box<dyn std::error::Error>> {
    let socket = ListeningSocketSource::with_name(name)
        .map_err(|e| format!("failed to bind wayland socket {name}: {e}"))?;
    let socket_name = socket.socket_name().to_os_string();
    let template = Arc::new(client_state);
    handle
        .insert_source(socket, move |stream, _, data| {
            let client = ClientState {
                slot: template.slot,
                is_overlay: template.is_overlay,
                compositor_state: CompositorClientState::default(),
            };
            if let Err(e) = data
                .state
                .display_handle
                .insert_client(stream, Arc::new(client))
            {
                eprintln!(
                    "[comp] rejecting client on {}: {e}",
                    template
                        .slot
                        .map_or("overlay".to_string(), |s| format!("slot {s}"))
                );
            }
        })
        .map_err(|e| format!("failed to register wayland socket {name}: {e}"))?;
    Ok(socket_name)
}

pub fn remove_socket_files(names: &[OsString]) {
    let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") else {
        return;
    };
    let dir = Path::new(&runtime_dir);
    for name in names {
        let _ = std::fs::remove_file(dir.join(name));
        let mut lock = name.clone();
        lock.push(".lock");
        let _ = std::fs::remove_file(dir.join(lock));
    }
}
