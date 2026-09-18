use eframe::egui::Ui;

use crate::app::app::PartyApp;
use crate::app::help;
use crate::app::widgets::setting_checkbox;

pub(super) fn show(app: &mut PartyApp, ui: &mut Ui) {
    let hint = &mut app.info_text;
    let cfg = &mut app.cfg;

    setting_checkbox(
        ui,
        hint,
        &mut cfg.gamescope_fix_lowres,
        "Automatically fix low resolution instances",
        help::GAMESCOPE_FIX_LOWRES,
    );
    setting_checkbox(
        ui,
        hint,
        &mut cfg.kbm_support,
        "Enable keyboard and mouse support through custom Gamescope",
        help::KBM_SUPPORT,
    );
    setting_checkbox(
        ui,
        hint,
        &mut cfg.gamescope_force_grab_cursor,
        "Force grab cursor for Gamescope",
        help::GAMESCOPE_FORCE_GRAB_CURSOR,
    );
}
