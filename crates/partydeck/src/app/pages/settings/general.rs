use eframe::egui::{self, Ui};

use crate::app::app::PartyApp;
use crate::app::dialogs::open_in_file_manager;
use crate::app::help;
use crate::app::widgets::{setting_checkbox, setting_radio_group};
use crate::config::PadFilterType;
use crate::paths::PATH_PARTY;

const PAD_FILTERS: [(PadFilterType, &str); 3] = [
    (PadFilterType::All, "All controllers"),
    (PadFilterType::NoSteamInput, "No Steam Input"),
    (PadFilterType::OnlySteamInput, "Only Steam Input"),
];

pub(super) fn show(app: &mut PartyApp, ui: &mut Ui) {
    let hint = &mut app.info_text;
    let cfg = &mut app.cfg;

    setting_checkbox(
        ui,
        hint,
        &mut cfg.check_for_updates,
        "Check for partydeck updates",
        help::CHECK_FOR_UPDATES,
    );

    ui.horizontal(|ui| {
        let label = ui.label("Split layout");
        egui::ComboBox::from_id_salt("layout_preset")
            .selected_text(cfg.layout_preset.clone())
            .show_ui(ui, |ui| {
                for preset in help::LAYOUT_PRESETS {
                    ui.selectable_value(&mut cfg.layout_preset, preset.to_string(), *preset);
                }
            });
        if label.hovered() {
            *hint = help::LAYOUT_PRESET.to_string();
        }
    });

    let filter = setting_radio_group(
        ui,
        hint,
        &mut cfg.pad_filter_type,
        "Controller filter",
        PAD_FILTERS,
        help::CONTROLLER_FILTER,
    );
    if filter.clicked {
        app.rescan_input_devices();
    }

    let hint = &mut app.info_text;
    let cfg = &mut app.cfg;
    setting_checkbox(
        ui,
        hint,
        &mut cfg.profile_unique_dirs,
        "Unique per-profile environments",
        help::PROFILE_UNIQUE_DIRS,
    );
    setting_checkbox(
        ui,
        hint,
        &mut cfg.allow_multiple_instances_on_same_device,
        "(Debug) Allow multiple instances from one gamepad",
        help::ALLOW_SAME_DEVICE,
    );
    setting_checkbox(
        ui,
        hint,
        &mut cfg.disable_mount_gamedirs,
        "(Debug) Force run instances from original game directory",
        help::DISABLE_MOUNT_GAMEDIRS,
    );

    ui.separator();

    if ui.button("Open PartyDeck Data Folder").clicked() {
        open_in_file_manager(PATH_PARTY.as_path(), "Couldn't open PartyDeck Data Folder!");
    }
}
