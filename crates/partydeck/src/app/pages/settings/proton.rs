use eframe::egui::{self, Ui};

use crate::app::app::PartyApp;
use crate::app::dialogs::{msg, yesno};
use crate::app::help;
use crate::app::widgets::setting_checkbox;
use crate::launch::erase_prefixes;
use crate::paths::prefixes_dir;

pub(super) fn show(app: &mut PartyApp, ui: &mut Ui) {
    let hint = &mut app.info_text;
    let cfg = &mut app.cfg;

    ui.horizontal(|ui| {
        let label = ui.label("Proton version");
        let edit =
            ui.add(egui::TextEdit::singleline(&mut cfg.proton_version).hint_text("GE-Proton"));
        if label.hovered() || edit.hovered() {
            *hint = help::PROTON_VERSION.to_string();
        }
    });

    setting_checkbox(
        ui,
        hint,
        &mut cfg.proton_separate_pfxs,
        "Run instances in separate Proton prefixes",
        help::PROTON_SEPARATE_PFXS,
    );
    setting_checkbox(
        ui,
        hint,
        &mut cfg.proton_wow64,
        "Run Proton in WoW64 mode",
        help::PROTON_WOW64,
    );

    if ui.button("Erase All Proton Prefix Data").clicked()
        && yesno("Erase Prefix?", help::ERASE_PREFIXES_CONFIRM)
        && prefixes_dir().exists()
    {
        if let Err(err) = erase_prefixes() {
            msg("Error", &format!("Couldn't erase pfx data: {err}"));
        } else {
            msg("Data Erased", "Proton prefix data successfully erased.");
        }
    }
}
