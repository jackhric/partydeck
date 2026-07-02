use std::ffi::OsString;
use std::sync::Arc;

use smithay::backend::renderer::ImportMemWl;
use smithay::desktop::{PopupManager, Space, Window, WindowSurfaceType};
use smithay::input::{Seat, SeatState};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, LoopSignal, Mode, PostAction};
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::utils::{Clock, Logical, Monotonic, Point};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::presentation::PresentationState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::viewporter::ViewporterState;

use partydeck_comp::layout::Layout;

use crate::backend::Backend;
use crate::CalloopData;

pub struct CompState {
    pub start_time: std::time::Instant,
    pub frames: u32,
    pub frame_seq: u64,
    pub child_commits: u32,
    pub commit_gaps: [u32; 3],
    pub last_commit_at: Option<std::time::Instant>,
    pub last_fps_report: std::time::Instant,
    pub socket_names: Vec<OsString>,
    pub display_handle: DisplayHandle,

    pub host_ready: bool,
    pub assets: crate::overlay::Assets,
    pub layout: Layout,
    pub slot_windows: Vec<Option<Window>>,
    pub slot_status: Vec<Option<(String, Option<String>)>>,
    pub clear_color: [f32; 4],

    pub space: Space<Window>,
    pub loop_signal: LoopSignal,
    pub backend: Backend,

    pub clock: Clock<Monotonic>,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub presentation_state: PresentationState,
    pub viewporter_state: ViewporterState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<CompState>,
    pub data_device_state: DataDeviceState,
    pub popups: PopupManager,

    pub seat: Seat<Self>,
}

impl CompState {
    pub fn new(
        event_loop: &mut EventLoop<CalloopData>,
        display: Display<Self>,
        socket_prefix: &str,
        layout: Layout,
        mut backend: Backend,
    ) -> Self {
        let start_time = std::time::Instant::now();
        let dh = display.handle();

        let clock = Clock::new();
        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);
        // gamescope's Vulkan swapchain hard-requires wp_presentation; without it
        // the child falls back to X11 and aborts.
        let presentation_state = PresentationState::new::<Self>(&dh, clock.id() as u32);
        let viewporter_state = ViewporterState::new::<Self>(&dh);
        let mut shm_state = ShmState::new::<Self>(&dh, vec![]);
        shm_state.update_formats(backend.winit.renderer().shm_formats());
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let popups = PopupManager::default();

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "partydeck-comp");
        seat.add_keyboard(Default::default(), 200, 25).unwrap();
        seat.add_pointer();

        let mut space = Space::default();
        space.map_output(&backend.output, (0, 0));

        let socket_names = Self::init_wayland_listeners(display, event_loop, socket_prefix, layout.slots.len());
        let loop_signal = event_loop.get_signal();

        let clear_color = layout
            .background
            .as_deref()
            .and_then(parse_color)
            .unwrap_or([0.05, 0.05, 0.08, 1.0]);
        let slot_windows = vec![None; layout.slots.len()];
        let slot_status = vec![None; layout.slots.len()];

        Self {
            start_time,
            frames: 0,
            frame_seq: 0,
            child_commits: 0,
            commit_gaps: [0; 3],
            last_commit_at: None,
            last_fps_report: start_time,
            display_handle: dh,

            host_ready: true,
            assets: crate::overlay::Assets::load(),
            layout,
            slot_windows,
            slot_status,
            clear_color,

            space,
            loop_signal,
            socket_names,
            backend,

            clock,
            compositor_state,
            xdg_shell_state,
            xdg_decoration_state,
            presentation_state,
            viewporter_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            popups,
            seat,
        }
    }

    fn init_wayland_listeners(
        display: Display<CompState>,
        event_loop: &mut EventLoop<CalloopData>,
        prefix: &str,
        players: usize,
    ) -> Vec<OsString> {
        let loop_handle = event_loop.handle();
        let mut names = Vec::with_capacity(players);

        for slot in 0..players.max(1) {
            let name = format!("{prefix}-p{slot}");
            let listening_socket = ListeningSocketSource::with_name(&name)
                .unwrap_or_else(|e| panic!("failed to bind wayland socket {name}: {e}"));
            names.push(listening_socket.socket_name().to_os_string());

            loop_handle
                .insert_source(listening_socket, move |client_stream, _, state| {
                    state
                        .display_handle
                        .insert_client(
                            client_stream,
                            Arc::new(ClientState { slot: Some(slot), ..Default::default() }),
                        )
                        .unwrap();
                })
                .expect("failed to init the wayland event source");
        }

        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // Safety: we don't drop the display
                    unsafe {
                        display.get_mut().dispatch_clients(&mut state.state).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();

        names
    }

    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.space.element_under(pos).and_then(|(window, location)| {
            window
                .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, p)| (s, (p + location).to_f64()))
        })
    }
}

#[derive(Default)]
pub struct ClientState {
    pub slot: Option<usize>,
    pub compositor_state: CompositorClientState,
}

fn parse_color(s: &str) -> Option<[f32; 4]> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some([
        ((v >> 16) & 0xff) as f32 / 255.0,
        ((v >> 8) & 0xff) as f32 / 255.0,
        (v & 0xff) as f32 / 255.0,
        1.0,
    ])
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
