use eframe::egui::{self, Ui};

use crate::app::app::PartyApp;
use crate::app::dialogs::{msg, open_in_file_manager, prompt_text};
use crate::profile::{create_profile, profile_dir};

/// Height kept free below the list for the New button.
const FOOTER_HEIGHT: f32 = 16.0;

pub(super) fn show(app: &mut PartyApp, ui: &mut Ui) {
    ui.heading("Profiles");
    ui.separator();
    egui::ScrollArea::vertical()
        .max_height(ui.available_height() - FOOTER_HEIGHT)
        .auto_shrink(false)
        .show(ui, |ui| {
            for profile in &app.profiles {
                if ui.selectable_value(&mut 0, 1, profile).clicked() {
                    open_in_file_manager(&profile_dir(profile), "Couldn't open profile directory!");
                }
            }
        });
    if ui.button("New").clicked() {
        if let Some(name) = prompt_text("New Profile", "Enter name (must be alphanumeric):") {
            new_profile(&name);
        }
        app.refresh_profiles();
    }
}

fn new_profile(name: &str) {
    if name.is_empty() || !name.chars().all(char::is_alphanumeric) {
        msg("Error", "Invalid name");
    } else if let Err(e) = create_profile(name) {
        msg("Error", &format!("Couldn't create profile: {e}"));
    }
}
