use smithay::desktop::Window;
use smithay::output::Mode;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Physical, Point, SERIAL_COUNTER, Size};
use smithay::wayland::shell::xdg::ToplevelSurface;

use super::fit;
use crate::state::{CompState, window_has_surface};
use crate::wayland::sockets::{is_overlay, slot_of};

pub fn raise_overlay(state: &mut CompState) {
    if let Some(overlay) = state.overlay_window.clone() {
        state.space.raise_element(&overlay, false);
    }
}

fn configure_fullscreen(surface: &ToplevelSurface, size: Size<i32, Logical>) {
    surface.with_pending_state(|s| {
        s.states.set(xdg_toplevel::State::Fullscreen);
        // All slots stay Activated: gamescope children self-throttle to their
        // unfocused refresh rate without it. Keyboard focus is separate.
        s.states.set(xdg_toplevel::State::Activated);
        s.size = Some(size);
    });
}

fn map_overlay(state: &mut CompState, surface: ToplevelSurface) {
    let size = state.output_size();
    configure_fullscreen(&surface, (size.w, size.h).into());
    let window = Window::new_wayland_window(surface);
    state.space.map_element(window.clone(), (0, 0), false);
    state.space.raise_element(&window, false);
    // A replacement overlay client (crash restart, upgrade) supersedes the
    // old surface entirely; leaving it mapped would stack stale content.
    if let Some(old) = state.overlay_window.replace(window) {
        state.space.unmap_elem(&old);
    }
}

pub fn map_toplevel(state: &mut CompState, surface: ToplevelSurface) {
    if is_overlay(&surface) {
        return map_overlay(state, surface);
    }
    let slot = slot_of(&surface);
    let rects = state.slot_rects();
    let output_size = state.output_size();
    let (origin, size): (Point<i32, Logical>, Size<i32, Logical>) = match slot {
        Some(i) if i < rects.len() => {
            let r = rects[i];
            ((r.x, r.y).into(), (r.w, r.h).into())
        }
        _ => ((0, 0).into(), (output_size.w, output_size.h).into()),
    };
    configure_fullscreen(&surface, size);

    let wl_surface = surface.wl_surface().clone();
    let window = Window::new_wayland_window(surface);
    state.space.map_element(window.clone(), origin, false);
    raise_overlay(state);

    let focus_this = match slot {
        Some(i) => {
            if let Some(entry) = state.slot_windows.get_mut(i) {
                *entry = Some(window);
            }
            i == state.layout.focus
        }
        None => true,
    };
    if focus_this && let Some(keyboard) = state.seat.get_keyboard() {
        keyboard.set_focus(state, Some(wl_surface), SERIAL_COUNTER.next_serial());
    }
}

pub fn unmap_toplevel(state: &mut CompState, surface: &ToplevelSurface) {
    let wl = surface.wl_surface();
    if state
        .overlay_window
        .as_ref()
        .is_some_and(|w| window_has_surface(w, wl))
    {
        if let Some(window) = state.overlay_window.take() {
            state.space.unmap_elem(&window);
        }
        return;
    }
    for entry in state.slot_windows.iter_mut() {
        if entry.as_ref().is_some_and(|w| window_has_surface(w, wl))
            && let Some(window) = entry.take()
        {
            state.space.unmap_elem(&window);
        }
    }
}

// Children normally match their configured size, but during resizes (or with
// stubborn clients) the committed size differs: place at the contain-fit origin
// so hit-testing agrees with what render draws.
pub fn position_on_commit(state: &mut CompState, window: &Window) {
    let Some(slot) = window.toplevel().and_then(slot_of) else {
        return;
    };
    let Some(r) = state.slot_rects().get(slot).copied() else {
        return;
    };
    let geo = window.geometry().size;
    let Some(fit) = fit::contain(r, geo.w, geo.h) else {
        return;
    };
    let origin: Point<i32, Logical> = (fit.x, fit.y).into();
    if state.space.element_location(window) != Some(origin) {
        state.space.map_element(window.clone(), origin, false);
    }
}

pub fn relayout(state: &mut CompState) {
    if let Some(toplevel) = state.overlay_window.as_ref().and_then(|w| w.toplevel()) {
        let size = state.output_size();
        toplevel.with_pending_state(|s| {
            s.size = Some((size.w, size.h).into());
        });
        toplevel.send_pending_configure();
    }
    let rects = state.slot_rects();
    for (i, entry) in state.slot_windows.iter().enumerate() {
        let (Some(window), Some(r)) = (entry, rects.get(i)) else {
            continue;
        };
        if let Some(toplevel) = window.toplevel() {
            toplevel.with_pending_state(|s| {
                s.size = Some((r.w, r.h).into());
            });
            toplevel.send_pending_configure();
        }
        state.space.map_element(window.clone(), (r.x, r.y), false);
    }
    // Re-mapping slots puts them at the top of the stack; the overlay must
    // stay above them.
    raise_overlay(state);
}

pub fn handle_output_resize(state: &mut CompState, size: Size<i32, Physical>) {
    // The monitor (and thus refresh) may only be known after mapping; re-read
    // it here so mode, feedback and timer period pick up the real rate.
    let refresh = crate::backend::winit::monitor_refresh_mhz(state.backend.winit.window());
    let mode = Mode { size, refresh };
    state
        .backend
        .output
        .change_current_state(Some(mode), None, None, None);
    state.backend.output.set_preferred(mode);
    relayout(state);
}
