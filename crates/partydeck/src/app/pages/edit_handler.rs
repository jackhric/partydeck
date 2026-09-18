use std::path::PathBuf;

use eframe::egui::{self, Ui};

use crate::app::app::PartyApp;
use crate::app::dialogs::{dir_dialog, file_dialog_relative, msg, pick_png};
use crate::app::help;
use crate::app::state::MenuPage;
use crate::app::widgets::{bottom_footer, handler_icon, radio_row};
use crate::handler::{HANDLER_SPEC_CURRENT_VERSION, Handler, SDL2Override};

const NAME_FIELD_WIDTH: f32 = 150.0;
const SHORT_FIELD_WIDTH: f32 = 50.0;
const STEAM_APP_COMBO_WIDTH: f32 = 200.0;

const SDL2_OVERRIDES: [(SDL2Override, &str); 3] = [
    (SDL2Override::No, "None"),
    (SDL2Override::Srt, "Steam Runtime (32-bit)"),
    (SDL2Override::Sys, "System Installation"),
];

const RUNTIMES: [(&str, &str); 5] = [
    ("", "None"),
    ("scout", "1.0 (scout)"),
    ("soldier", "2.0 (soldier)"),
    ("sniper", "3.0 (sniper)"),
    ("steamrt4", "4.0 (steamrt4)"),
];

pub(super) fn show(app: &mut PartyApp, ui: &mut Ui) {
    let Some(h) = &mut app.handler_edit else {
        return;
    };

    let header = match h.is_saved_handler() {
        false => "Add Game".to_string(),
        true => format!("Edit Handler: {}", h.display()),
    };
    ui.heading(header);
    ui.separator();

    header_row(ui, h);
    ui.separator();
    steam_app_row(ui, h, &app.installed_steamapps);
    paths_rows(ui, h);
    runtime_rows(ui, h);

    if footer(ui, h) {
        save(app);
    }
}

fn header_row(ui: &mut Ui, h: &mut Handler) {
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.add(egui::TextEdit::singleline(&mut h.name).desired_width(NAME_FIELD_WIDTH));
        ui.label("Author:");
        ui.add(egui::TextEdit::singleline(&mut h.author).desired_width(SHORT_FIELD_WIDTH));
        ui.label("Version:");
        ui.add(egui::TextEdit::singleline(&mut h.version).desired_width(SHORT_FIELD_WIDTH));
        ui.label("Icon:");
        handler_icon(ui, h);
        if h.is_saved_handler()
            && ui.button("🖼").clicked()
            && let Some(file) = pick_png("Choose Icon:")
        {
            let dest = h.path_handler.join("icon.png");
            if let Err(e) = std::fs::copy(file, dest) {
                eprintln!("[partydeck] Failed to copy icon: {e}");
                msg("Error copying icon", &format!("{e}"));
            }
        }
    });
}

fn steam_app_row(ui: &mut Ui, h: &mut Handler, apps: &[Option<steamlocate::App>]) {
    let mut selected = apps
        .iter()
        .position(|app| match (app, &h.steam_appid) {
            (Some(app), Some(appid)) => app.app_id == *appid,
            (None, None) => true,
            _ => false,
        })
        .unwrap_or(0);

    ui.horizontal(|ui| {
        ui.label("Steam App:");
        egui::ComboBox::from_id_salt("appid")
            .wrap()
            .width(STEAM_APP_COMBO_WIDTH)
            .show_index(ui, &mut selected, apps.len(), |i| match &apps[i] {
                Some(app) => format!("({}) {}", app.app_id, app.install_dir),
                None => "None".to_string(),
            });
        ui.checkbox(&mut h.use_goldberg, "Emulate Steam Client");
        ui.checkbox(&mut h.use_mangohud, "Enable MangoHud");
    });

    h.steam_appid = apps
        .get(selected)
        .and_then(|app| app.as_ref().map(|app| app.app_id));
}

fn paths_rows(ui: &mut Ui, h: &mut Handler) {
    if h.steam_appid.is_none() {
        ui.horizontal(|ui| {
            ui.label("Game root folder:");
            ui.add_enabled(false, egui::TextEdit::singleline(&mut h.path_gameroot));
            if ui.button("🗁").clicked()
                && let Ok(path) = dir_dialog()
            {
                h.path_gameroot = path.to_string_lossy().to_string();
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label("Executable:");
        ui.add_enabled(false, egui::TextEdit::singleline(&mut h.exec));
        if ui.button("🗁").clicked()
            && let Ok(base_path) = h.get_game_rootpath()
            && let Ok(path) = file_dialog_relative(&PathBuf::from(base_path))
        {
            h.exec = path.to_string_lossy().to_string();
        }
    });
    ui.horizontal(|ui| {
        ui.label("Environment variables:");
        ui.add(egui::TextEdit::singleline(&mut h.env));
    });
    ui.horizontal(|ui| {
        ui.label("Arguments:");
        ui.add(egui::TextEdit::singleline(&mut h.args));
    });
}

fn runtime_rows(ui: &mut Ui, h: &mut Handler) {
    if h.win() {
        ui.checkbox(&mut h.enable_hidraw, help::HIDRAW_LABEL);
    } else {
        radio_row(ui, &mut h.sdl2_override, "SDL2 Override:", SDL2_OVERRIDES);
        radio_row(
            ui,
            &mut h.runtime,
            "Linux Runtime:",
            RUNTIMES
                .iter()
                .map(|(value, text)| (value.to_string(), *text)),
        );
    }

    if h.spec_ver != HANDLER_SPEC_CURRENT_VERSION
        && ui.button("Update Handler Specification Version").clicked()
    {
        h.spec_ver = HANDLER_SPEC_CURRENT_VERSION;
        msg(
            "Handler Specification Version Updated",
            "Remember to save your changes.",
        );
    }
}

fn footer(ui: &mut Ui, _h: &Handler) -> bool {
    bottom_footer(ui, false, |ui| ui.button("Save").clicked())
}

fn save(app: &mut PartyApp) {
    let Some(h) = &mut app.handler_edit else {
        return;
    };
    match h.save_to_json() {
        Err(e) => msg("Error saving handler", &format!("{e}")),
        Ok(()) => {
            app.handlers.rescan();
            app.cur_page = MenuPage::Game;
        }
    }
}
