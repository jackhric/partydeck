use eframe::egui::{self, RichText, Ui};

use super::{DEVICES_PANEL_WIDTH, PANEL_TOP_SPACING};
use crate::app::app::PartyApp;
use crate::app::help;
use crate::app::widgets::{bottom_footer, disabled_while_busy};
use crate::input::InputDevice;

pub(in crate::app) fn show(app: &mut PartyApp, ctx: &egui::Context) {
    egui::SidePanel::right("devices_panel")
        .resizable(false)
        .exact_width(DEVICES_PANEL_WIDTH)
        .show(ctx, |ui| {
            disabled_while_busy(ui, app.is_busy());
            ui.add_space(PANEL_TOP_SPACING);
            ui.heading("Devices");
            ui.separator();
            for device in &app.input_devices {
                ui.label(device_text(device));
            }
            bottom_footer(ui, false, troubleshooting_links);
        });
}

fn device_text(device: &InputDevice) -> RichText {
    let text = RichText::new(format!(
        "{} {} ({})",
        device.emoji(),
        device.fancyname(),
        device.path().trim_start_matches("/dev/input/event")
    ))
    .small();
    if !device.enabled() {
        text.weak()
    } else if device.has_button_held() {
        text.strong()
    } else {
        text
    }
}

fn troubleshooting_links(ui: &mut Ui) {
    ui.link("ℹ Incorrect/missing controller mappings in-game?")
        .on_hover_ui(|ui| {
            ui.label(help::CONTROLLER_MAPPINGS);
        });
    ui.link("ℹ Devices not being detected?").on_hover_ui(|ui| {
        ui.style_mut().interaction.selectable_labels = true;
        ui.label("Try adding your user to the `input` group.");
        ui.label("In a terminal, enter the following command:");
        ui.horizontal(|ui| {
            ui.code(help::INPUT_GROUP_COMMAND);
            if ui.button("📎").clicked() {
                ui.ctx().copy_text(help::INPUT_GROUP_COMMAND.to_string());
            }
        });
    });
}
