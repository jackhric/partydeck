mod gamescope;
mod general;
mod proton;

use eframe::egui::{self, Ui};

use crate::app::app::PartyApp;
use crate::app::dialogs::msg;
use crate::app::state::SettingsPage;
use crate::app::widgets::bottom_footer;
use crate::config::{PartyConfig, save_cfg};

/// Height kept free below the scroll area for the Save/Restore footer.
const FOOTER_HEIGHT: f32 = 30.0;

pub(super) fn show(app: &mut PartyApp, ui: &mut Ui) {
    app.info_text.clear();
    ui.horizontal(|ui| {
        ui.heading("Settings");
        ui.selectable_value(&mut app.settings_page, SettingsPage::General, "General");
        ui.selectable_value(&mut app.settings_page, SettingsPage::Proton, "Proton");
        ui.selectable_value(&mut app.settings_page, SettingsPage::Gamescope, "Gamescope");
    });
    ui.separator();

    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - FOOTER_HEIGHT)
        .auto_shrink(false)
        .show(ui, |ui| match app.settings_page {
            SettingsPage::General => general::show(app, ui),
            SettingsPage::Proton => proton::show(app, ui),
            SettingsPage::Gamescope => gamescope::show(app, ui),
        });

    footer(app, ui);
}

fn footer(app: &mut PartyApp, ui: &mut Ui) {
    bottom_footer(ui, true, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Save Settings").clicked()
                && let Err(e) = save_cfg(&app.cfg)
            {
                msg("Error", &format!("Couldn't save settings: {e}"));
            }
            if ui.button("Restore Defaults").clicked() {
                app.cfg = PartyConfig::default();
                app.rescan_input_devices();
            }
        });
    });
}
