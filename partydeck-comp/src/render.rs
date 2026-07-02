use std::time::Duration;

use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::utils::{
    surface_presentation_feedback_flags_from_states, surface_primary_scanout_output,
    OutputPresentationFeedback,
};
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::Rectangle;
use smithay::wayland::presentation::Refresh;

use crate::state::CompState;

// Presented feedback is acked promptly on commit so a child's present_wait
// never stalls on our render cadence; frame callbacks stay redraw-driven so
// children pace to the output refresh instead of free-running.
pub fn ack_present(state: &mut CompState, window: &smithay::desktop::Window) {
    let output = state.backend.output.clone();
    let mut feedback = OutputPresentationFeedback::new(&output);
    window.take_presentation_feedback(
        &mut feedback,
        |_, _| Some(output.clone()),
        |_, _| wp_presentation_feedback::Kind::Vsync,
    );
    let refresh = output
        .current_mode()
        .map(|mode| Refresh::fixed(Duration::from_secs_f64(1_000f64 / mode.refresh as f64)))
        .unwrap_or(Refresh::Unknown);
    state.frame_seq += 1;
    feedback.presented(state.clock.now(), refresh, state.frame_seq, wp_presentation_feedback::Kind::Vsync);
    let _ = state.display_handle.flush_clients();

    let now = std::time::Instant::now();
    if let Some(prev) = state.last_commit_at.replace(now) {
        let gap = now.duration_since(prev).as_millis() as u64;
        let bucket = match gap {
            0..=19 => 0,
            20..=49 => 1,
            _ => 2,
        };
        state.commit_gaps[bucket] += 1;
    }
}

pub fn redraw(state: &mut CompState, display_handle: &mut DisplayHandle) {
    state.frames += 1;
    let since = state.last_fps_report.elapsed();
    if since.as_secs() >= 5 {
        eprintln!(
            "[comp] fps: {:.1}, child commits/s: {:.1}, gaps <20ms/20-50/>50: {}/{}/{}",
            state.frames as f64 / since.as_secs_f64(),
            state.child_commits as f64 / since.as_secs_f64(),
            state.commit_gaps[0],
            state.commit_gaps[1],
            state.commit_gaps[2]
        );
        state.frames = 0;
        state.child_commits = 0;
        state.commit_gaps = [0; 3];
        state.last_fps_report = std::time::Instant::now();
    }

    state.frame_seq += 1;
    let CompState {
        backend,
        space,
        start_time,
        popups,
        clock,
        frame_seq,
        clear_color,
        layout,
        slot_windows,
        assets,
        ..
    } = state;

    let size = backend.winit.window_size();
    let damage = Rectangle::from_size(size);
    let overlay_elements =
        crate::overlay::build(layout, slot_windows, assets, *start_time, size, backend.winit.renderer());

    let states = {
        let (renderer, mut framebuffer) = backend.winit.bind().unwrap();
        smithay::desktop::space::render_output::<_, crate::overlay::OverlayElement, _, _>(
            &backend.output,
            renderer,
            &mut framebuffer,
            1.0,
            0,
            [&*space],
            &overlay_elements,
            &mut backend.damage_tracker,
            *clear_color,
        )
        .unwrap()
        .states
    };
    backend.winit.submit(Some(&[damage])).unwrap();

    let mut feedback = OutputPresentationFeedback::new(&backend.output);
    space.elements().for_each(|window| {
        window.take_presentation_feedback(&mut feedback, surface_primary_scanout_output, |surface, _| {
            surface_presentation_feedback_flags_from_states(surface, &states)
        });
    });
    let refresh = backend
        .output
        .current_mode()
        .map(|mode| Refresh::fixed(Duration::from_secs_f64(1_000f64 / mode.refresh as f64)))
        .unwrap_or(Refresh::Unknown);
    feedback.presented(clock.now(), refresh, *frame_seq, wp_presentation_feedback::Kind::Vsync);

    space.refresh();
    popups.cleanup();
    let _ = display_handle.flush_clients();

    backend.winit.window().request_redraw();
}

// Child frame callbacks are timer-driven, independent of our composite: they
// must keep flowing at the refresh rate even when the host paces (or hides) us.
pub fn tick_children(state: &mut CompState, display_handle: &mut DisplayHandle) {
    let output = state.backend.output.clone();
    state.space.elements().for_each(|window| {
        window.send_frame(&output, state.start_time.elapsed(), Some(Duration::ZERO), |_, _| {
            Some(output.clone())
        })
    });
    let _ = display_handle.flush_clients();
}
