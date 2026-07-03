use std::time::Duration;

use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::utils::{CropRenderElement, RescaleRenderElement};
use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::utils::{
    surface_presentation_feedback_flags_from_states, surface_primary_scanout_output,
    OutputPresentationFeedback,
};
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::render_elements;
use smithay::utils::Rectangle;
use smithay::wayland::presentation::Refresh;

use crate::state::CompState;

render_elements! {
    pub CompElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Scaled=CropRenderElement<RescaleRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>>,
}

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
        popups,
        clock,
        frame_seq,
        clear_color,
        layout,
        slot_windows,
        overlay_window,
        ..
    } = state;

    let size = backend.winit.window_size();
    let damage = Rectangle::from_size(size);
    let mut elements: Vec<CompElement> = Vec::new();

    // The overlay client renders 1:1 above everything.
    if let Some(overlay) = overlay_window {
        elements.extend(
            overlay
                .render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
                    backend.winit.renderer(),
                    (0, 0).into(),
                    1.0.into(),
                    1.0,
                )
                .into_iter()
                .map(CompElement::Surface),
        );
    }

    // Slot windows are contain-fitted: nested gamescope never honors resize
    // configures, so whatever buffer size an instance was launched with gets
    // scaled and cropped into its slot here.
    let rects = layout.resolve(size.w.max(1) as u32, size.h.max(1) as u32);
    for (i, entry) in slot_windows.iter().enumerate() {
        let (Some(window), Some(r)) = (entry.as_ref(), rects.get(i)) else {
            continue;
        };
        let geo = window.geometry().size;
        if geo.w <= 0 || geo.h <= 0 {
            continue;
        }
        let fit = (r.w as f64 / geo.w as f64).min(r.h as f64 / geo.h as f64);
        let origin = smithay::utils::Point::<i32, smithay::utils::Physical>::from((
            r.x + ((r.w as f64 - geo.w as f64 * fit) / 2.0) as i32,
            r.y + ((r.h as f64 - geo.h as f64 * fit) / 2.0) as i32,
        ));
        let crop = Rectangle::new((r.x, r.y).into(), (r.w, r.h).into());
        elements.extend(
            window
                .render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
                    backend.winit.renderer(),
                    origin,
                    1.0.into(),
                    1.0,
                )
                .into_iter()
                .filter_map(|e| {
                    CropRenderElement::from_element(
                        RescaleRenderElement::from_element(e, origin, fit),
                        1.0,
                        crop,
                    )
                })
                .map(CompElement::Scaled),
        );
    }

    let states = {
        let (renderer, mut framebuffer) = backend.winit.bind().unwrap();
        backend
            .damage_tracker
            .render_output::<CompElement, _>(
                renderer,
                &mut framebuffer,
                0,
                &elements,
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
