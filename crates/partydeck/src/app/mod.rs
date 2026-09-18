#[allow(clippy::module_inception)]
mod app;
mod app_pages;
mod app_panels;
pub mod dialogs;
pub mod handler_view;

pub use app::PartyApp;

use crate::handler::Handler;
use crate::monitor::{detect_monitors, get_x11_dpi_scale};

/// Zoom used for the windowed launcher.
const WINDOWED_ZOOM: f32 = 1.3;
/// Logical screen height the fullscreen layout was designed for; the zoom
/// factor scales the real panel height down to it.
const FULLSCREEN_REFERENCE_HEIGHT: f32 = 560.0;

const WINDOW_SIZE: [f32; 2] = [1080.0, 540.0];
const MIN_WINDOW_SIZE: [f32; 2] = [640.0, 360.0];

fn zoom_factor(fullscreen: bool, screen_height: u32) -> f32 {
    if fullscreen {
        screen_height as f32 / FULLSCREEN_REFERENCE_HEIGHT / get_x11_dpi_scale()
    } else {
        WINDOWED_ZOOM
    }
}

pub fn run(fullscreen: bool, handler_lite: Option<Handler>) -> eframe::Result {
    let monitors = detect_monitors();
    eprintln!("[partydeck] Monitors detected:");
    for monitor in &monitors {
        eprintln!(
            "[partydeck] {} ({}x{})",
            monitor.name(),
            monitor.width(),
            monitor.height()
        );
    }
    let screen_height = monitors.first().map_or(0, |m| m.height());
    let zoom = zoom_factor(fullscreen, screen_height);

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size(WINDOW_SIZE)
        .with_min_inner_size(MIN_WINDOW_SIZE)
        .with_fullscreen(fullscreen);
    match eframe::icon_data::from_png_bytes(include_bytes!("../../assets/icons/icon.png")) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => eprintln!("[partydeck] Failed to load window icon: {e}"),
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eprintln!("[partydeck] Starting eframe app...");
    eframe::run_native(
        "PartyDeck",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.set_zoom_factor(zoom);
            Ok(Box::new(PartyApp::new(monitors, handler_lite)))
        }),
    )
}
