use std::ffi::OsString;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Duration;

use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::utils::{CropRenderElement, RescaleRenderElement};
use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::utils::{
    surface_presentation_feedback_flags_from_states, take_presentation_feedback_surface_tree,
    OutputPresentationFeedback,
};
use smithay::desktop::Window;
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::render_elements;
use smithay::utils::Rectangle;
use smithay::wayland::presentation::Refresh;

use crate::state::CompState;

// Feedback must never sit pending longer than this: nested gamescope's
// present_wait deadlocks if a requested feedback never arrives.
pub const STALL_TIMEOUT: Duration = Duration::from_millis(250);

const FRAME_LOG_FLUSH_ROWS: u32 = 60;

render_elements! {
    pub CompElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Scaled=CropRenderElement<RescaleRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>>,
}

pub struct FrameLog {
    writer: BufWriter<File>,
    rows: u32,
}

impl FrameLog {
    pub fn open(path: OsString) -> Option<Self> {
        let file = std::fs::OpenOptions::new().create(true).append(true).open(&path);
        match file {
            Ok(file) => {
                let mut writer = BufWriter::new(file);
                if writer.get_ref().metadata().map(|m| m.len() == 0).unwrap_or(false) {
                    let _ = writeln!(
                        writer,
                        "frame_seq,tick_mono_ns,redraw_start_ns,submit_done_ns,children_committed_since_last,callbacks_sent,feedback_presented,feedback_discarded"
                    );
                }
                Some(Self { writer, rows: 0 })
            }
            Err(e) => {
                eprintln!("[comp] frame log {}: {e}", path.to_string_lossy());
                None
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn log(
        &mut self,
        frame_seq: u64,
        tick_ns: u128,
        redraw_start_ns: u128,
        submit_done_ns: u128,
        commits: u32,
        callbacks: usize,
        presented: u32,
        discarded: u32,
    ) {
        let _ = writeln!(
            self.writer,
            "{frame_seq},{tick_ns},{redraw_start_ns},{submit_done_ns},{commits},{callbacks},{presented},{discarded}"
        );
        self.rows += 1;
        if self.rows >= FRAME_LOG_FLUSH_ROWS {
            let _ = self.writer.flush();
            self.rows = 0;
        }
    }
}

pub fn note_commit(state: &mut CompState) {
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

fn refresh_of(output: &smithay::output::Output) -> Refresh {
    output
        .current_mode()
        .filter(|mode| mode.refresh > 0)
        .map(|mode| Refresh::fixed(Duration::from_secs_f64(1_000f64 / mode.refresh as f64)))
        .unwrap_or(Refresh::Unknown)
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
    let refresh = refresh_of(&output);
    state.frame_seq += 1;
    feedback.presented(state.clock.now(), refresh, state.frame_seq, wp_presentation_feedback::Kind::Vsync);
    let _ = state.display_handle.flush_clients();
}

pub fn discard_feedback(state: &mut CompState, surface: &WlSurface) {
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

pub fn report_telemetry(state: &mut CompState) {
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
}

pub fn redraw(state: &mut CompState, display_handle: &mut DisplayHandle, tick: std::time::Instant) {
    state.frames += 1;
    state.frame_seq += 1;
    let legacy_ack = state.legacy_ack;
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
        start_time,
        commits_since_composite,
        frame_log,
        last_composite_at,
        ..
    } = state;

    let redraw_start = start_time.elapsed();
    let output = backend.output.clone();
    let size = backend.winit.window_size();
    let damage = Rectangle::from_size(size);
    let mut elements: Vec<CompElement> = Vec::new();
    let mut rendered: Vec<Window> = Vec::new();

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
        rendered.push(overlay.clone());
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
        rendered.push(window.clone());
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
    let submit_done = start_time.elapsed();
    *last_composite_at = tick;

    let callbacks_sent = space.elements().count();
    space.elements().for_each(|window| {
        window.send_frame(&output, start_time.elapsed(), Some(Duration::ZERO), |_, _| {
            Some(output.clone())
        })
    });

    let (fb_presented, fb_discarded) = if legacy_ack {
        (0, 0)
    } else {
        let mut presented = OutputPresentationFeedback::new(&output);
        let mut discarded = OutputPresentationFeedback::new(&output);
        let mut np = 0u32;
        let mut nd = 0u32;
        space.elements().for_each(|window| {
            if rendered.contains(window) {
                window.take_presentation_feedback(
                    &mut presented,
                    |_, _| Some(output.clone()),
                    |surface, _| surface_presentation_feedback_flags_from_states(surface, &states),
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
        });
        presented.presented(clock.now(), refresh_of(&output), *frame_seq, wp_presentation_feedback::Kind::Vsync);
        discarded.discarded();
        (np, nd)
    };

    if let Some(log) = frame_log {
        log.log(
            *frame_seq,
            tick.duration_since(*start_time).as_nanos(),
            redraw_start.as_nanos(),
            submit_done.as_nanos(),
            *commits_since_composite,
            callbacks_sent,
            fb_presented,
            fb_discarded,
        );
    }
    *commits_since_composite = 0;

    space.refresh();
    popups.cleanup();
    let _ = display_handle.flush_clients();

    backend.winit.window().request_redraw();
}

// Watchdog path while the host withholds redraw acks (window hidden, host
// stalled): keep frame callbacks flowing and discard pending feedback so
// children neither freeze nor deadlock in present_wait.
pub fn stall_tick(state: &mut CompState, display_handle: &mut DisplayHandle) {
    let output = state.backend.output.clone();
    let mut feedback = OutputPresentationFeedback::new(&output);
    state.space.elements().for_each(|window| {
        window.send_frame(&output, state.start_time.elapsed(), Some(Duration::ZERO), |_, _| {
            Some(output.clone())
        });
        window.take_presentation_feedback(
            &mut feedback,
            |_, _| Some(output.clone()),
            |_, _| wp_presentation_feedback::Kind::empty(),
        );
    });
    feedback.discarded();
    let _ = display_handle.flush_clients();
}
