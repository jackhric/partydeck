use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::ImportDma;
use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::PopupKind;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1;
use smithay::reexports::wayland_server::protocol::{wl_buffer, wl_seat, wl_surface::WlSurface};
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::utils::Serial;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    CompositorClientState, CompositorHandler, CompositorState, get_parent, is_sync_subsurface,
    with_states,
};
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
    set_data_device_focus,
};
use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    XdgToplevelSurfaceData,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{
    delegate_compositor, delegate_data_device, delegate_dmabuf, delegate_output,
    delegate_presentation, delegate_seat, delegate_shm, delegate_viewporter,
    delegate_xdg_decoration, delegate_xdg_shell,
};

use super::{ClientState, popups};
use crate::layout::slots;
use crate::render::feedback;
use crate::state::CompState;

impl CompositorHandler for CompState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<ClientState>()
            .expect("clients are created with ClientState")
            .compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        self.telemetry.note_surface_commit();
        on_commit_buffer_handler::<Self>(surface);
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            match self.window_for_surface(&root) {
                Some(window) => {
                    window.on_commit();
                    slots::position_on_commit(self, &window);
                    self.telemetry.note_window_commit();
                    if self.telemetry.legacy_ack() {
                        feedback::ack_present(self, &window);
                    }
                }
                // Surfaces we never composite must not keep feedback pending:
                // nested gamescope's present_wait deadlocks on it.
                None => feedback::discard(self, &root),
            }
        }

        if let Some(window) = self.window_for_surface(surface) {
            let initial_configure_sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .and_then(|d| d.lock().ok())
                    .is_none_or(|d| d.initial_configure_sent)
            });
            if !initial_configure_sent && let Some(toplevel) = window.toplevel() {
                toplevel.send_configure();
            }
        }
        popups::on_commit(self, surface);
    }
}
delegate_compositor!(CompState);

impl BufferHandler for CompState {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for CompState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}
delegate_shm!(CompState);

impl SeatHandler for CompState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<CompState> {
        &mut self.seat_state
    }

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client);
    }
}
delegate_seat!(CompState);

impl SelectionHandler for CompState {
    type SelectionUserData = ();
}

impl DataDeviceHandler for CompState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for CompState {}
impl ServerDndGrabHandler for CompState {}
delegate_data_device!(CompState);

impl OutputHandler for CompState {}
delegate_output!(CompState);

delegate_presentation!(CompState);
delegate_viewporter!(CompState);

impl XdgShellHandler for CompState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        slots::map_toplevel(self, surface);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        slots::unmap_toplevel(self, &surface);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        popups::unconstrain(self, &surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        popups::unconstrain(self, &surface);
        surface.send_repositioned(token);
    }

    fn move_request(&mut self, _surface: ToplevelSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}
}
delegate_xdg_shell!(CompState);

impl DmabufHandler for CompState {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.backend.dmabuf_state.0
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        if self
            .backend
            .winit
            .renderer()
            .import_dmabuf(&dmabuf, None)
            .is_ok()
        {
            let _ = notifier.successful::<CompState>();
        } else {
            notifier.failed();
        }
    }
}
delegate_dmabuf!(CompState);

// Children render inside grid slots; client-side decorations are never wanted.
fn force_server_side(toplevel: &ToplevelSurface) {
    toplevel.with_pending_state(|state| {
        state.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ServerSide);
    });
    if toplevel.is_initial_configure_sent() {
        toplevel.send_pending_configure();
    }
}

impl XdgDecorationHandler for CompState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ServerSide);
        });
    }

    fn request_mode(
        &mut self,
        toplevel: ToplevelSurface,
        _mode: zxdg_toplevel_decoration_v1::Mode,
    ) {
        force_server_side(&toplevel);
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        force_server_side(&toplevel);
    }
}
delegate_xdg_decoration!(CompState);
