mod edit_handler;
mod game;
mod home;
mod instances;
mod profiles;
mod settings;

use eframe::egui::Ui;

use super::app::PartyApp;
use super::state::MenuPage;

pub(super) fn show(app: &mut PartyApp, ui: &mut Ui) {
    match app.cur_page {
        MenuPage::Home => home::show(ui),
        MenuPage::Settings => settings::show(app, ui),
        MenuPage::Profiles => profiles::show(app, ui),
        MenuPage::EditHandler => edit_handler::show(app, ui),
        MenuPage::Game => game::show(app, ui),
        MenuPage::Instances => instances::show(app, ui),
    }
}
