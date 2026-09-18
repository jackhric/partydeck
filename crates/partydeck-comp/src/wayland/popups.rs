use smithay::desktop::{PopupKind, find_popup_root_surface, get_popup_toplevel_coords};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::shell::xdg::PopupSurface;

use crate::state::CompState;

pub fn on_commit(state: &mut CompState, surface: &WlSurface) {
    state.popups.commit(surface);
    if let Some(PopupKind::Xdg(xdg)) = state.popups.find_popup(surface)
        && !xdg.is_initial_configure_sent()
        && let Err(e) = xdg.send_configure()
    {
        eprintln!("[comp] popup initial configure failed: {e}");
    }
}

pub fn unconstrain(state: &CompState, popup: &PopupSurface) {
    let kind = PopupKind::Xdg(popup.clone());
    let Ok(root) = find_popup_root_surface(&kind) else {
        return;
    };
    let Some(window) = state.window_for_surface(&root) else {
        return;
    };
    let Some(output_geo) = state
        .space
        .outputs()
        .next()
        .and_then(|o| state.space.output_geometry(o))
    else {
        return;
    };
    let Some(window_geo) = state.space.element_geometry(&window) else {
        return;
    };

    let mut target = output_geo;
    target.loc -= get_popup_toplevel_coords(&kind);
    target.loc -= window_geo.loc;

    popup.with_pending_state(|state| {
        state.geometry = state.positioner.get_unconstrained_geometry(target);
    });
}
