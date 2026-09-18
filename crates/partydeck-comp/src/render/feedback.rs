use std::time::Duration;

use smithay::backend::renderer::element::RenderElementStates;
use smithay::desktop::utils::{
    OutputPresentationFeedback, surface_presentation_feedback_flags_from_states,
    take_presentation_feedback_surface_tree,
};
use smithay::desktop::{Space, Window};
use smithay::output::Output;
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Clock, Monotonic};
use smithay::wayland::presentation::Refresh;

use crate::state::CompState;

// Feedback must never sit pending longer than this: nested gamescope's
// present_wait deadlocks if a requested feedback never arrives.
pub const STALL_TIMEOUT: Duration = Duration::from_millis(250);

fn refresh_of(output: &Output) -> Refresh {
    output
        .current_mode()
        .filter(|mode| mode.refresh > 0)
        .map(|mode| Refresh::fixed(Duration::from_secs_f64(1_000f64 / mode.refresh as f64)))
        .unwrap_or(Refresh::Unknown)
}

pub fn present(
    space: &Space<Window>,
    output: &Output,
    clock: &Clock<Monotonic>,
    frame_seq: u64,
    rendered: &[Window],
    states: &RenderElementStates,
) -> (u32, u32) {
    let mut presented = OutputPresentationFeedback::new(output);
    let mut discarded = OutputPresentationFeedback::new(output);
    let mut np = 0u32;
    let mut nd = 0u32;
    for window in space.elements() {
        if rendered.contains(window) {
            window.take_presentation_feedback(
                &mut presented,
                |_, _| Some(output.clone()),
                |surface, _| surface_presentation_feedback_flags_from_states(surface, states),
            );
            np += 1;
        } else {
            window.take_presentation_feedback(
                &mut discarded,
                |_, _| Some(output.clone()),
                |_, _| wp_presentation_feedback::Kind::empty(),
            );
            nd += 1;
        }
    }
    presented.presented(
        clock.now(),
        refresh_of(output),
        frame_seq,
        wp_presentation_feedback::Kind::Vsync,
    );
    discarded.discarded();
    (np, nd)
}

// Legacy PARTYDECK_COMP_LEGACY_ACK=1 path: ack presents on commit instead of
// on composite, so children never wait on our render cadence.
pub fn ack_present(state: &mut CompState, window: &Window) {
    let output = state.backend.output.clone();
    let mut feedback = OutputPresentationFeedback::new(&output);
    window.take_presentation_feedback(
        &mut feedback,
        |_, _| Some(output.clone()),
        |_, _| wp_presentation_feedback::Kind::Vsync,
    );
    state.frame_seq += 1;
    feedback.presented(
        state.clock.now(),
        refresh_of(&output),
        state.frame_seq,
        wp_presentation_feedback::Kind::Vsync,
    );
    let _ = state.display_handle.flush_clients();
}

pub fn discard(state: &mut CompState, surface: &WlSurface) {
    let output = state.backend.output.clone();
    let mut feedback = OutputPresentationFeedback::new(&output);
    take_presentation_feedback_surface_tree(
        surface,
        &mut feedback,
        |_, _| Some(output.clone()),
        |_, _| wp_presentation_feedback::Kind::empty(),
    );
    feedback.discarded();
    let _ = state.display_handle.flush_clients();
}

// Watchdog path while the host withholds redraw acks (window hidden, host
// stalled): keep frame callbacks flowing and discard pending feedback so
// children neither freeze nor deadlock in present_wait.
pub fn stall_tick(state: &mut CompState, display_handle: &mut DisplayHandle) {
    let output = state.backend.output.clone();
    let mut feedback = OutputPresentationFeedback::new(&output);
    for window in state.space.elements() {
        window.send_frame(
            &output,
            state.start_time.elapsed(),
            Some(Duration::ZERO),
            |_, _| Some(output.clone()),
        );
        window.take_presentation_feedback(
            &mut feedback,
            |_, _| Some(output.clone()),
            |_, _| wp_presentation_feedback::Kind::empty(),
        );
    }
    feedback.discarded();
    let _ = display_handle.flush_clients();
}
