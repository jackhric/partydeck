use smithay::backend::egl::EGLDevice;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::ImportDma;
use smithay::backend::winit::{self, WinitEvent, WinitEventLoop};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::winit::dpi::LogicalSize;
use smithay::reexports::winit::platform::wayland::WindowAttributesExtWayland;
use smithay::reexports::winit::window::{Fullscreen, Window as WinitWindow};
use smithay::utils::Transform;
use smithay::wayland::dmabuf::{DmabufFeedbackBuilder, DmabufState};

use super::Backend;
use crate::state::CompState;
use crate::{slots, CalloopData};

pub fn init(
    display_handle: &DisplayHandle,
    size: (u32, u32),
    fullscreen: bool,
) -> Result<(Backend, WinitEventLoop), Box<dyn std::error::Error>> {
    let mut attrs = WinitWindow::default_attributes()
        .with_inner_size(LogicalSize::new(size.0 as f64, size.1 as f64))
        .with_title("partydeck-comp");
    if fullscreen {
        attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
    }
    if let Some(app_id) = steam_app_id() {
        eprintln!("[comp] app_id {app_id}");
        // Also covers X11/XWayland: WindowAttributesExtX11::with_name writes
        // the same `name` field, which becomes WM_CLASS there.
        attrs = attrs.with_name(&app_id, "");
    }
    let (mut backend, winit) = winit::init_from_attributes::<GlesRenderer>(attrs)?;

    let refresh = monitor_refresh_mhz(backend.window());
    eprintln!("[comp] output refresh {refresh} mHz");
    let mode = Mode {
        size: backend.window_size(),
        refresh,
    };
    let output = Output::new(
        "partydeck-comp".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "PartyDeck".into(),
            model: "Comp".into(),
        },
    );
    let _global = output.create_global::<CompState>(display_handle);
    output.change_current_state(Some(mode), Some(Transform::Flipped180), None, Some((0, 0).into()));
    output.set_preferred(mode);

    let render_node = EGLDevice::device_for_display(backend.renderer().egl_context().display())
        .and_then(|device| device.try_get_render_node());

    let dmabuf_state = match render_node {
        Ok(Some(node)) => {
            let formats = backend.renderer().dmabuf_formats();
            let feedback = DmabufFeedbackBuilder::new(node.dev_id(), formats).build()?;
            let mut dmabuf_state = DmabufState::new();
            let global =
                dmabuf_state.create_global_with_default_feedback::<CompState>(display_handle, &feedback);
            eprintln!("[comp] dmabuf: default-feedback via render node {node:?}");
            (dmabuf_state, global, Some(feedback))
        }
        other => {
            eprintln!("[comp] dmabuf: v3 fallback (render node query: {other:?})");
            let formats = backend.renderer().dmabuf_formats();
            let mut dmabuf_state = DmabufState::new();
            let global = dmabuf_state.create_global::<CompState>(display_handle, formats);
            (dmabuf_state, global, None)
        }
    };

    let damage_tracker = OutputDamageTracker::from_output(&output);

    Ok((
        Backend {
            winit: backend,
            output,
            damage_tracker,
            dmabuf_state,
        },
        winit,
    ))
}

pub fn monitor_refresh_mhz(window: &WinitWindow) -> i32 {
    window
        .current_monitor()
        .and_then(|m| m.refresh_rate_millihertz())
        .map(|r| r as i32)
        .filter(|&r| r > 0)
        .unwrap_or(60_000)
}

// Non-Steam shortcuts report a 64-bit SteamGameId with the 32-bit shortcut
// appid in the upper half; real Steam apps report the appid directly.
fn appid_from_gameid(gameid: u64) -> u32 {
    if gameid > u32::MAX as u64 {
        (gameid >> 32) as u32
    } else {
        gameid as u32
    }
}

fn steam_app_id() -> Option<String> {
    let appid = std::env::var("SteamAppId")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&id| id != 0)
        .or_else(|| {
            std::env::var("SteamGameId")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .map(appid_from_gameid)
                .filter(|&id| id != 0)
        })?;
    Some(format!("steam_app_{appid}"))
}

pub fn insert_source(
    event_loop: &mut EventLoop<CalloopData>,
    winit: WinitEventLoop,
) -> Result<(), Box<dyn std::error::Error>> {
    event_loop.handle().insert_source(winit, move |event, _, data| {
        match event {
            WinitEvent::Resized { size, .. } => slots::handle_output_resize(&mut data.state, size),
            WinitEvent::Input(event) => data.state.process_input_event(event),
            // Rendering runs on our own timer; Redraw only marks the host as
            // ready so a hidden window's blocking submit can't wedge the loop.
            WinitEvent::Redraw => data.state.host_ready = true,
            WinitEvent::CloseRequested => data.state.loop_signal.stop(),
            _ => (),
        };
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::appid_from_gameid;

    #[test]
    fn gameid_to_appid() {
        assert_eq!(appid_from_gameid((2488242132u64 << 32) | (1 << 25)), 2488242132);
        assert_eq!(appid_from_gameid(620), 620);
    }
}
