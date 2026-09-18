use eframe::egui::{self, Ui};

use crate::app::app::PartyApp;
use crate::app::dialogs::msg;
use crate::app::handler_view;
use crate::app::help;
use crate::app::icons;
use crate::handler::{HANDLER_SPEC_CURRENT_VERSION, Handler};
use crate::monitor::detect_monitors;

const SCREENSHOT_ASPECT: f32 = 1.77;

pub(super) fn show(app: &mut PartyApp, ui: &mut Ui) {
    let Some(h) = app.cur_handler() else {
        ui.label("No game selected.");
        return;
    };

    ui.horizontal(|ui| {
        ui.image(handler_view::icon(h));
        ui.heading(h.display());
    });
    ui.separator();

    let play = ui
        .horizontal(|ui| {
            let play = play_button(ui);
            meta_row(ui, h);
            play
        })
        .inner;
    screenshots(ui, h);

    if play {
        on_play(app);
    }
}

fn play_button(ui: &mut Ui) -> bool {
    ui.add(egui::Button::image_and_text(icons::BTN_START, "Play"))
        .clicked()
}

fn meta_row(ui: &mut Ui, h: &Handler) {
    ui.add(egui::Separator::default().vertical());
    if h.win() {
        ui.label(" Proton");
    } else {
        ui.label("🐧 Native");
    }
    if !h.author.is_empty() {
        ui.add(egui::Separator::default().vertical());
        ui.label(format!("Author: {}", h.author));
    }
    if !h.version.is_empty() {
        ui.add(egui::Separator::default().vertical());
        ui.label(format!("Version: {}", h.version));
    }
}

fn screenshots(ui: &mut Ui, h: &Handler) {
    egui::ScrollArea::horizontal()
        .max_width(f32::INFINITY)
        .show(ui, |ui| {
            let height = ui.available_height();
            ui.horizontal(|ui| {
                for img in &h.img_paths {
                    ui.add(
                        egui::Image::new(format!("file://{}", img.display()))
                            .fit_to_exact_size(egui::vec2(height * SCREENSHOT_ASPECT, height))
                            .maintain_aspect_ratio(true),
                    );
                }
            });
        });
}

fn on_play(app: &mut PartyApp) {
    let Some(h) = app.cur_handler() else {
        return;
    };
    if h.spec_ver != HANDLER_SPEC_CURRENT_VERSION {
        msg(
            "Handler version mismatch",
            &help::handler_version_mismatch(h.spec_ver < HANDLER_SPEC_CURRENT_VERSION),
        );
    }
    if h.steam_appid.is_none() && h.path_gameroot.is_empty() {
        msg(
            "Game root path not found",
            "Please specify the game's root folder.",
        );
        let h = h.clone();
        app.edit_handler(h);
    } else {
        app.rescan_input_devices();
        app.monitors = detect_monitors();
        app.open_instances_page();
    }
}
