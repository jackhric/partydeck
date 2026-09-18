use eframe::egui;

use super::INFO_PANEL_HEIGHT;
use crate::app::app::PartyApp;
use crate::app::help;
use crate::app::state::MenuPage;
use crate::app::widgets::disabled_while_busy;

pub(in crate::app) fn show(app: &mut PartyApp, ctx: &egui::Context) {
    egui::TopBottomPanel::bottom("info_panel")
        .exact_height(INFO_PANEL_HEIGHT)
        .show(ctx, |ui| {
            disabled_while_busy(ui, app.is_busy());
            match app.cur_page {
                MenuPage::Game => {
                    app.info_text = app
                        .cur_handler()
                        .map(|h| h.info.clone())
                        .unwrap_or_default();
                }
                MenuPage::Profiles => app.info_text = help::PROFILES_INFO.to_string(),
                _ => {}
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                if app.cur_page == MenuPage::EditHandler
                    && let Some(handler) = &mut app.handler_edit
                {
                    ui.add(
                        egui::TextEdit::multiline(&mut handler.info)
                            .hint_text("Put game info/instructions here"),
                    );
                } else {
                    ui.label(&app.info_text);
                }
            });
        });
}
