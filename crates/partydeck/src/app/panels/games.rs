use eframe::egui::{self, Popup, Ui};

use super::{GAMES_PANEL_WIDTH, PANEL_TOP_SPACING};
use crate::app::app::PartyApp;
use crate::app::dialogs::{
    msg, open_in_file_manager, pick_pd2_save_path, pick_pd2_to_import, yesno,
};
use crate::app::handler_view;
use crate::app::state::MenuPage;
use crate::app::widgets::{disabled_while_busy, handler_icon};
use crate::handler::Handler;
use crate::handler::package::{export_pd2, import_pd2};

pub(in crate::app) fn show(app: &mut PartyApp, ctx: &egui::Context) {
    egui::SidePanel::left("games_panel")
        .resizable(false)
        .exact_width(GAMES_PANEL_WIDTH)
        .show(ctx, |ui| {
            disabled_while_busy(ui, app.is_busy());
            ui.add_space(PANEL_TOP_SPACING);
            header(app, ui);
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| game_list(app, ui));
        });
}

fn header(app: &mut PartyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.heading("Games");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("➕").clicked() {
                app.edit_handler(Handler::default());
            }
            if ui.button("⬇").clicked()
                && let Some(file) = pick_pd2_to_import()
            {
                if let Err(e) = import_pd2(&file) {
                    msg("Error", &format!("Error importing PD2: {e}"));
                } else {
                    app.handlers.rescan();
                }
            }
            if ui.button("🔄").clicked() {
                app.handlers.rescan();
            }
        });
    });
}

fn game_list(app: &mut PartyApp, ui: &mut Ui) {
    for i in 0..app.handlers.list.len() {
        // A context menu action may have rescanned the list mid-loop.
        if i >= app.handlers.list.len() {
            return;
        }
        ui.horizontal(|ui| {
            handler_icon(ui, &app.handlers.list[i]);
            let label = handler_view::display_clamp(&app.handlers.list[i]);
            let btn = ui.selectable_value(&mut app.handlers.selected, i, label);
            if btn.has_focus() {
                btn.scroll_to_me(None);
            }
            if btn.clicked() {
                app.cur_page = MenuPage::Game;
            }
            Popup::context_menu(&btn).show(|ui| context_menu(app, ui, i));
        });
    }
}

fn context_menu(app: &mut PartyApp, ui: &mut Ui, i: usize) {
    if ui.button("Edit").clicked() {
        app.edit_handler(app.handlers.list[i].clone());
    }

    if ui.button("Open Folder").clicked() {
        open_in_file_manager(
            &app.handlers.list[i].path_handler,
            "Couldn't open handler folder!",
        );
    }

    if ui.button("Remove").clicked()
        && yesno(
            "Remove handler?",
            &format!(
                "Are you sure you want to remove {}?",
                app.handlers.list[i].display()
            ),
        )
    {
        if let Err(err) = app.handlers.list[i].remove_handler() {
            eprintln!("[partydeck] Failed to remove handler: {err}");
            msg("Error", &format!("Failed to remove handler: {err}"));
        }
        app.handlers.rescan();
        if app.handlers.is_empty() {
            app.cur_page = MenuPage::Home;
        }
    }

    if ui.button("Export").clicked()
        && let Some(dest) = pick_pd2_save_path()
        && let Err(err) = export_pd2(&app.handlers.list[i], &dest)
    {
        eprintln!("[partydeck] Failed to export handler: {err}");
        msg("Error", &format!("Failed to export handler: {err}"));
    }
}
