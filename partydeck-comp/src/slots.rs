use smithay::desktop::Window;
use smithay::output::Mode;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Physical, Point, Size, SERIAL_COUNTER};
use smithay::wayland::shell::xdg::ToplevelSurface;

use partydeck_comp_proto::layout::PixelRect;

use crate::state::{ClientState, CompState};

fn slot_rects(state: &CompState) -> Vec<PixelRect> {
    let size = state.backend.winit.window_size();
    state.layout.resolve(size.w.max(1) as u32, size.h.max(1) as u32)
}

fn slot_of(surface: &ToplevelSurface) -> Option<usize> {
    surface
        .wl_surface()
        .client()
        .and_then(|c| c.get_data::<ClientState>().and_then(|d| d.slot))
}

fn is_overlay(surface: &ToplevelSurface) -> bool {
    surface
        .wl_surface()
        .client()
        .and_then(|c| c.get_data::<ClientState>().map(|d| d.is_overlay))
        .unwrap_or(false)
}

pub fn raise_overlay(state: &mut CompState) {
    if let Some(overlay) = state.overlay_window.clone() {
        state.space.raise_element(&overlay, false);
    }
}

pub fn map_toplevel(state: &mut CompState, surface: ToplevelSurface) {
    if is_overlay(&surface) {
        let size = state.backend.winit.window_size();
        surface.with_pending_state(|s| {
            s.states.set(xdg_toplevel::State::Fullscreen);
            s.states.set(xdg_toplevel::State::Activated);
            s.size = Some((size.w, size.h).into());
        });
        let window = Window::new_wayland_window(surface);
        state.space.map_element(window.clone(), (0, 0), false);
        state.space.raise_element(&window, false);
        // A replacement overlay client (crash restart, upgrade) supersedes the
        // old surface entirely; leaving it mapped would stack stale content.
        if let Some(old) = state.overlay_window.replace(window) {
            state.space.unmap_elem(&old);
        }
        return;
    }
    let slot = slot_of(&surface);
    let rects = slot_rects(state);
    let output_size = state.backend.winit.window_size();
    let (origin, size): (Point<i32, Logical>, Size<i32, Logical>) = match slot {
        Some(i) if i < rects.len() => {
            let r = rects[i];
            ((r.x, r.y).into(), (r.w, r.h).into())
        }
        _ => ((0, 0).into(), (output_size.w, output_size.h).into()),
    };

    surface.with_pending_state(|s| {
        s.states.set(xdg_toplevel::State::Fullscreen);
        // All slots stay Activated: gamescope children self-throttle to their
        // unfocused refresh rate without it. Keyboard focus is separate.
        s.states.set(xdg_toplevel::State::Activated);
        s.size = Some(size);
    });

    let wl_surface = surface.wl_surface().clone();
    let window = Window::new_wayland_window(surface);
    state.space.map_element(window.clone(), origin, false);

    raise_overlay(state);
    let focus_this = match slot {
        Some(i) => {
            if i < state.slot_windows.len() {
                state.slot_windows[i] = Some(window);
            }
            i == state.layout.focus
        }
        None => true,
    };
    if focus_this {
        let keyboard = state.seat.get_keyboard().unwrap();
        keyboard.set_focus(state, Some(wl_surface), SERIAL_COUNTER.next_serial());
    }
}

pub fn handle_toplevel_destroyed(state: &mut CompState, surface: ToplevelSurface) {
    let wl = surface.wl_surface();
    let overlay_matches = state
        .overlay_window
        .as_ref()
        .and_then(|w| w.toplevel().map(|t| t.wl_surface() == wl))
        .unwrap_or(false);
    if overlay_matches {
        if let Some(window) = state.overlay_window.take() {
            state.space.unmap_elem(&window);
        }
        return;
    }
    for entry in state.slot_windows.iter_mut() {
        let matches = entry
            .as_ref()
            .and_then(|w| w.toplevel().map(|t| t.wl_surface() == wl))
            .unwrap_or(false);
        if matches {
            if let Some(window) = entry.take() {
                state.space.unmap_elem(&window);
            }
        }
    }
}

// Children normally match their configured size, but during resizes (or with
// stubborn clients) the committed size differs: center within the slot.
pub fn position_on_commit(state: &mut CompState, window: &Window) {
    let Some(slot) = window.toplevel().and_then(|t| slot_of(t)) else {
        return;
    };
    let rects = slot_rects(state);
    let Some(r) = rects.get(slot).copied() else {
        return;
    };
    let geo = window.geometry().size;
    if geo.w <= 0 || geo.h <= 0 {
        return;
    }
    let origin: Point<i32, Logical> =
        (r.x + (r.w - geo.w).max(0) / 2, r.y + (r.h - geo.h).max(0) / 2).into();
    if state.space.element_location(window) != Some(origin) {
        state.space.map_element(window.clone(), origin, false);
    }
}

pub fn relayout(state: &mut CompState) {
    if let Some(overlay) = &state.overlay_window {
        if let Some(toplevel) = overlay.toplevel() {
            let size = state.backend.winit.window_size();
            toplevel.with_pending_state(|s| {
                s.size = Some((size.w, size.h).into());
            });
            toplevel.send_pending_configure();
        }
    }
    let rects = slot_rects(state);
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
    let mode = Mode { size, refresh: 60_000 };
    state.backend.output.change_current_state(Some(mode), None, None, None);
    state.backend.output.set_preferred(mode);
    relayout(state);
}
