use eframe::egui::{self, Ui};

use crate::app::app::PartyApp;
use crate::app::icons;
use crate::app::state::MenuPage;
use crate::app::widgets::disabled_while_busy;
use crate::monitor::detect_monitors;

const RELEASES_URL: &str = "https://github.com/wunnr/partydeck/releases";
const HANDLERS_URL: &str = "https://drive.proton.me/urls/D9HBKM18YR#zG8XC8yVy9WL";
const KOFI_URL: &str = "https://ko-fi.com/wunner";
const LICENSES_URL: &str = "https://github.com/wunnr/partydeck/tree/main?tab=License-2-ov-file";
const GITHUB_URL: &str = "https://github.com/wunnr/partydeck";

pub(in crate::app) fn show(app: &mut PartyApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("menu_nav_panel").show(ctx, |ui| {
        disabled_while_busy(ui, app.is_busy());
        ui.horizontal(|ui| {
            nav_buttons(app, ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                links(app, ui);
            });
        });
    });
}

fn nav_buttons(app: &mut PartyApp, ui: &mut Ui) {
    let home_text = if app.is_lite() { "▶" } else { "ℹ" };
    let home = ui.add(
        egui::Button::image_and_text(icons::BTN_EAST, home_text)
            .selected(app.cur_page == MenuPage::Home),
    );
    if home.clicked() {
        app.cur_page = app.home_page();
    }

    let settings = ui.add(
        egui::Button::image_and_text(icons::BTN_NORTH, "⛭")
            .selected(app.cur_page == MenuPage::Settings),
    );
    if settings.clicked() {
        app.cur_page = MenuPage::Settings;
    }

    let profiles = ui.add(
        egui::Button::image_and_text(icons::BTN_WEST, "👥")
            .selected(app.cur_page == MenuPage::Profiles),
    );
    if profiles.clicked() {
        app.open_profiles_page();
    }

    if ui.button("🎮 🔄").clicked() {
        app.draft.clear();
        app.rescan_input_devices();
    }
    if ui.button("🖵 🔄").clicked() {
        app.draft.clear();
        app.monitors = detect_monitors();
    }
}

fn links(app: &PartyApp, ui: &mut Ui) {
    if ui.button("❌").clicked() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
    ui.add(egui::Separator::default().vertical());
    ui.hyperlink_to(version_label(app), RELEASES_URL);
    ui.add(egui::Separator::default().vertical());
    ui.hyperlink_to("⮋", HANDLERS_URL)
        .on_hover_text("Download Game Handlers");
    ui.hyperlink_to("♥", KOFI_URL)
        .on_hover_text("Support PartyDeck Development");
    ui.hyperlink_to("🖹", LICENSES_URL)
        .on_hover_text("Third-Party Licenses");
    ui.hyperlink_to("", GITHUB_URL).on_hover_text("GitHub");
}

fn version_label(app: &PartyApp) -> String {
    let version = env!("CARGO_PKG_VERSION");
    if !app.cfg.check_for_updates {
        format!("(Frozen) v{version}")
    } else if app.needs_update.load(std::sync::atomic::Ordering::Relaxed) {
        format!("v{version} (🆕 available)")
    } else {
        format!("v{version}")
    }
}
