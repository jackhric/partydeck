use std::ffi::OsString;
use std::time::Instant;

use smithay::backend::renderer::ImportMemWl;
use smithay::desktop::{PopupManager, Space, Window, WindowSurfaceType};
use smithay::input::{Seat, SeatState};
use smithay::reexports::calloop::{EventLoop, LoopSignal};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::utils::{Clock, Logical, Monotonic, Physical, Point, Size};
use smithay::wayland::compositor::CompositorState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::presentation::PresentationState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::viewporter::ViewporterState;

use partydeck_comp_proto::layout::{Layout, PixelRect};
use partydeck_comp_proto::state::{BorderStyle, SlotStatus};

use crate::backend::Backend;
use crate::render::telemetry::Telemetry;
use crate::{CalloopData, wayland};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotInfo {
    pub status: SlotStatus,
    pub label: Option<String>,
    pub avatar: Option<String>,
    pub logo: Option<String>,
}

// Held only so the Wayland globals stay advertised for the compositor's
// lifetime; nothing reads them back.
#[allow(dead_code)]
pub struct Globals {
    pub xdg_decoration: XdgDecorationState,
    pub presentation: PresentationState,
    pub viewporter: ViewporterState,
    pub output_manager: OutputManagerState,
}

pub struct CompState {
    pub start_time: Instant,
    pub frame_seq: u64,
    pub last_composite_at: Instant,
    pub host_ready: bool,
    pub telemetry: Telemetry,
    pub socket_names: Vec<OsString>,
    pub display_handle: DisplayHandle,

    pub layout: Layout,
    pub border: BorderStyle,
    pub slot_windows: Vec<Option<Window>>,
    pub overlay_window: Option<Window>,
    pub slot_info: Vec<Option<SlotInfo>>,

    pub space: Space<Window>,
    pub loop_signal: LoopSignal,
    pub backend: Backend,

    pub clock: Clock<Monotonic>,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<CompState>,
    pub data_device_state: DataDeviceState,
    pub popups: PopupManager,
    pub seat: Seat<Self>,
    pub _globals: Globals,
}

impl CompState {
    pub fn new(
        event_loop: &mut EventLoop<CalloopData>,
        display: Display<Self>,
        socket_prefix: &str,
        layout: Layout,
        border: BorderStyle,
        mut backend: Backend,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        let dh = display.handle();

        let clock = Clock::new();
        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let mut shm_state = ShmState::new::<Self>(&dh, vec![]);
        shm_state.update_formats(backend.winit.renderer().shm_formats());
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let globals = Globals {
            xdg_decoration: XdgDecorationState::new::<Self>(&dh),
            // gamescope's Vulkan swapchain hard-requires wp_presentation; without it
            // the child falls back to X11 and aborts.
            presentation: PresentationState::new::<Self>(&dh, clock.id() as u32),
            viewporter: ViewporterState::new::<Self>(&dh),
            output_manager: OutputManagerState::new_with_xdg_output::<Self>(&dh),
        };

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "partydeck-comp");
        seat.add_keyboard(Default::default(), 200, 25)
            .map_err(|e| format!("failed to add keyboard: {e:?}"))?;
        seat.add_pointer();

        let mut space = Space::default();
        space.map_output(&backend.output, (0, 0));

        let players = layout.slots.len();
        let socket_names = wayland::sockets::init(display, event_loop, socket_prefix, players)?;

        Ok(Self {
            start_time,
            frame_seq: 0,
            last_composite_at: start_time,
            host_ready: true,
            telemetry: Telemetry::new(),
            socket_names,
            display_handle: dh,

            layout,
            border,
            slot_windows: vec![None; players],
            overlay_window: None,
            slot_info: vec![None; players],

            space,
            loop_signal: event_loop.get_signal(),
            backend,

            clock,
            compositor_state,
            xdg_shell_state,
            shm_state,
            seat_state,
            data_device_state,
            popups: PopupManager::default(),
            seat,
            _globals: globals,
        })
    }

    pub fn output_size(&self) -> Size<i32, Physical> {
        self.backend.winit.window_size()
    }

    pub fn slot_rects(&self) -> Vec<PixelRect> {
        let size = self.output_size();
        self.layout
            .resolve(size.w.max(1) as u32, size.h.max(1) as u32)
    }

    pub fn window_for_surface(&self, surface: &WlSurface) -> Option<Window> {
        self.space
            .elements()
            .find(|w| window_has_surface(w, surface))
            .cloned()
    }

    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.space
            .element_under(pos)
            .and_then(|(window, location)| {
                window
                    .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            })
    }
}

pub fn window_has_surface(window: &Window, surface: &WlSurface) -> bool {
    window.toplevel().is_some_and(|t| t.wl_surface() == surface)
}
