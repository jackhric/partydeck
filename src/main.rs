mod app;
mod cli;
mod compositor;
mod handler;
mod input;
mod instance;
mod launch;
mod monitor;
mod paths;
mod profiles;
mod session;
mod util;

use crate::app::*;
use crate::handler::Handler;
use crate::monitor::{get_monitors_errorless, get_x11_dpi_scale};
use crate::paths::PATH_PARTY;
use crate::profiles::remove_guest_profiles;
use crate::util::*;

fn main() -> eframe::Result {
    use clap::Parser;
    let cli = cli::Cli::parse();

    if let Some(command) = cli.command {
        std::process::exit(cli::run(command));
    }

    let monitors = get_monitors_errorless();

    println!("[partydeck] Monitors detected:");
    for monitor in &monitors {
        println!(
            "[partydeck] {} ({}x{})",
            monitor.name(),
            monitor.width(),
            monitor.height()
        );
    }

    let handler_lite = cli
        .exec
        .as_deref()
        .filter(|exec| !exec.is_empty())
        .map(|exec| Handler::from_cli(exec, &cli.args));

    let fullscreen = cli.fullscreen;

    std::fs::create_dir_all(PATH_PARTY.join("handlers"))
        .expect("Failed to create handlers directory");
    std::fs::create_dir_all(PATH_PARTY.join("profiles"))
        .expect("Failed to create profiles directory");
    if !PATH_PARTY.join("goldberg_data").exists() {
        std::fs::create_dir_all(PATH_PARTY.join("goldberg_data/steam_settings"))
            .expect("Failed to create goldberg data!");
        std::fs::write(PATH_PARTY.join("goldberg_data/steam_settings/auto_accept_invite.txt"), "").expect("failed to create auto_accept_invite.txt");
        std::fs::write(PATH_PARTY.join("goldberg_data/steam_settings/auto_send_invite.txt"), "").expect("failed to create auto_send_invite.txt");
    }

    remove_guest_profiles().unwrap();
    clear_tmp().unwrap();

    let scrheight = monitors[0].height();

    let scale = match fullscreen {
        true => scrheight as f32 / 560.0 / get_x11_dpi_scale(),
        false => 1.3,
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 540.0])
            .with_min_inner_size([640.0, 360.0])
            .with_fullscreen(fullscreen)
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../res/icon.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    println!("[partydeck] Starting eframe app...");

    eframe::run_native(
        "PartyDeck",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.set_zoom_factor(scale);
            Ok(Box::<PartyApp>::new(PartyApp::new(
                monitors.clone(),
                handler_lite,
            )))
        }),
    )
}
