use eframe::egui::{self, ImageSource, Response, Ui};

use super::handler_view;
use crate::handler::Handler;

pub const GLYPH_HEIGHT: f32 = 12.0;
pub const HANDLER_ICON_SIZE: f32 = 16.0;

/// Checkbox for a setting; hovering it shows `help` in the info panel.
pub fn setting_checkbox(
    ui: &mut Ui,
    hint: &mut String,
    value: &mut bool,
    label: &str,
    help: &str,
) -> Response {
    let response = ui.checkbox(value, label);
    if response.hovered() {
        *hint = help.to_string();
    }
    response
}

pub struct RadioRow {
    pub hovered: bool,
    pub clicked: bool,
}

/// A label followed by one radio button per option on a single row.
pub fn radio_row<'a, T: PartialEq + Clone>(
    ui: &mut Ui,
    value: &mut T,
    label: &str,
    options: impl IntoIterator<Item = (T, &'a str)>,
) -> RadioRow {
    ui.horizontal(|ui| {
        let mut row = RadioRow {
            hovered: ui.label(label).hovered(),
            clicked: false,
        };
        for (option, text) in options {
            let response = ui.radio_value(value, option, text);
            row.hovered |= response.hovered();
            row.clicked |= response.clicked();
        }
        row
    })
    .inner
}

/// Radio row for a setting; hovering any part of it shows `help` in the info panel.
pub fn setting_radio_group<'a, T: PartialEq + Clone>(
    ui: &mut Ui,
    hint: &mut String,
    value: &mut T,
    label: &str,
    options: impl IntoIterator<Item = (T, &'a str)>,
    help: &str,
) -> RadioRow {
    let row = radio_row(ui, value, label, options);
    if row.hovered {
        *hint = help.to_string();
    }
    row
}

pub fn glyph(ui: &mut Ui, icon: ImageSource<'static>) {
    ui.add(egui::Image::new(icon).max_height(GLYPH_HEIGHT));
}

pub fn button_hint(ui: &mut Ui, icon: ImageSource<'static>, key_label: &str, text: &str) {
    glyph(ui, icon);
    ui.label(key_label);
    ui.label(text);
}

pub fn handler_icon(ui: &mut Ui, handler: &Handler) {
    ui.add(
        egui::Image::new(handler_view::icon(handler))
            .max_width(HANDLER_ICON_SIZE)
            .corner_radius(2),
    );
}

/// Content anchored to the bottom of `ui`, optionally separated from the page.
pub fn bottom_footer<R>(ui: &mut Ui, separator: bool, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
        let result = add(ui);
        if separator {
            ui.separator();
        }
        result
    })
    .inner
}

pub fn disabled_while_busy(ui: &mut Ui, busy: bool) {
    if busy {
        ui.disable();
    }
}
