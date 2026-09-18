pub mod feedback;
pub mod telemetry;

use std::time::{Duration, Instant};

use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::utils::{CropRenderElement, RescaleRenderElement};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::Window;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::render_elements;
use smithay::utils::{Physical, Point, Rectangle};

use crate::backend::Backend;
use crate::layout::fit;
use crate::state::CompState;
use telemetry::FrameRecord;

const CLEAR_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

render_elements! {
    pub CompElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Scaled=CropRenderElement<RescaleRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>>,
}

pub fn frame_interval(state: &CompState) -> Duration {
    let mhz = state
        .backend
        .output
        .current_mode()
        .map(|m| m.refresh)
        .filter(|&r| r > 0)
        .unwrap_or(60_000) as u64;
    Duration::from_nanos(1_000_000_000_000 / mhz)
}

pub fn redraw(state: &mut CompState, display_handle: &mut DisplayHandle, tick: Instant) {
    state.frame_seq += 1;
    let rects = state.slot_rects();
    let legacy_ack = state.telemetry.legacy_ack();
    let CompState {
        backend,
        space,
        popups,
        clock,
        frame_seq,
        slot_windows,
        overlay_window,
        start_time,
        telemetry,
        last_composite_at,
        ..
    } = state;

    let redraw_start = start_time.elapsed();
    let output = backend.output.clone();
    let size = backend.winit.window_size();
    let mut elements: Vec<CompElement> = Vec::new();
    let mut rendered: Vec<Window> = Vec::new();

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
        rendered.push(overlay.clone());
    }

    for (window, r) in slot_windows.iter().zip(&rects) {
        let Some(window) = window else { continue };
        let geo = window.geometry().size;
        let Some(fit) = fit::contain(*r, geo.w, geo.h) else {
            continue;
        };
        let origin = Point::<i32, Physical>::from((fit.x, fit.y));
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
                        RescaleRenderElement::from_element(e, origin, fit.scale),
                        1.0,
                        crop,
                    )
                })
                .map(CompElement::Scaled),
        );
        rendered.push(window.clone());
    }

    let composited: Result<_, String> = (|| {
        let (renderer, mut framebuffer) = backend
            .winit
            .bind()
            .map_err(|e| format!("bind failed: {e}"))?;
        let result = backend
            .damage_tracker
            .render_output::<CompElement, _>(renderer, &mut framebuffer, 0, &elements, CLEAR_COLOR)
            .map_err(|e| format!("render_output failed: {e}"))?;
        Ok(result.states)
    })();
    let states = match composited {
        Ok(states) => states,
        Err(e) => {
            eprintln!("[comp] {e}, skipping frame");
            skip_frame(display_handle, backend);
            return;
        }
    };
    if let Err(e) = backend.winit.submit(Some(&[Rectangle::from_size(size)])) {
        eprintln!("[comp] submit failed, skipping frame: {e}");
        skip_frame(display_handle, backend);
        return;
    }
    let submit_done = start_time.elapsed();
    *last_composite_at = tick;

    let callbacks_sent = space.elements().count();
    space.elements().for_each(|window| {
        window.send_frame(
            &output,
            start_time.elapsed(),
            Some(Duration::ZERO),
            |_, _| Some(output.clone()),
        )
    });

    let (presented, discarded) = if legacy_ack {
        (0, 0)
    } else {
        feedback::present(space, &output, clock, *frame_seq, &rendered, &states)
    };

    telemetry.frame_done(FrameRecord {
        frame_seq: *frame_seq,
        tick: tick.duration_since(*start_time),
        redraw_start,
        submit_done,
        callbacks_sent,
        presented,
        discarded,
    });

    space.refresh();
    popups.cleanup();
    let _ = display_handle.flush_clients();

    backend.winit.window().request_redraw();
}

// A skipped frame must still re-arm the redraw request, or the output stays
// frozen until the host resizes the window.
fn skip_frame(display_handle: &mut DisplayHandle, backend: &mut Backend) {
    let _ = display_handle.flush_clients();
    backend.winit.window().request_redraw();
}
