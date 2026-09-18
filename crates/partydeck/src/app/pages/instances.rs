use eframe::egui::{self, RichText, Ui};

use crate::app::app::PartyApp;
use crate::app::icons;
use crate::app::widgets::{bottom_footer, button_hint, glyph};
use crate::input::InputDevice;

pub(super) fn show(app: &mut PartyApp, ui: &mut Ui) {
    ui.heading("Instances");
    ui.separator();
    legend(app, ui);
    ui.separator();

    let mut to_remove: Vec<(usize, usize)> = Vec::new();
    for i in 0..app.draft.instances.len() {
        instance_row(app, ui, i, &mut to_remove);
    }
    for (instance, dev) in to_remove {
        app.draft.remove_device_from(instance, dev);
    }

    if !app.draft.instances.is_empty() {
        footer(app, ui);
    }
}

fn legend(app: &PartyApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        glyph(ui, icons::BTN_SOUTH);
        ui.label("[Z]");
        glyph(ui, icons::MOUSE_RIGHT);
        let add_text = match app.draft.adding_device_to {
            None => "Add New Instance".to_string(),
            Some(i) => format!("Add to Instance {}", i + 1),
        };
        ui.label(add_text);
        ui.add(egui::Separator::default().vertical());

        let remove_text = match app.draft.adding_device_to {
            None => "Remove",
            Some(_) => "Cancel",
        };
        button_hint(ui, icons::BTN_EAST, "[X]", remove_text);
        ui.add(egui::Separator::default().vertical());
    });
}

fn instance_row(app: &mut PartyApp, ui: &mut Ui, i: usize, to_remove: &mut Vec<(usize, usize)>) {
    let adding = app.draft.adding_device_to;
    let choices = &app.profile_choices;
    let instance = &mut app.draft.instances[i];
    ui.horizontal(|ui| {
        ui.label(format!("{}", i + 1));
        ui.label("👤");
        let mut selection = choices.index_of(&instance.profile);
        egui::ComboBox::from_id_salt(format!("{i}")).show_index(
            ui,
            &mut selection,
            choices.len(),
            |i| choices.label(i),
        );
        instance.profile = choices.choice_at(selection);

        if adding.is_none() {
            let invite = ui.add(egui::Button::image_and_text(
                icons::BTN_NORTH,
                "[A] Invite New Device",
            ));
            if invite.clicked() {
                app.draft.adding_device_to = Some(i);
            }
        } else if adding == Some(i) {
            ui.label("Adding new device...");
            if ui.button("🗙").clicked() {
                app.draft.adding_device_to = None;
            }
        }
    });

    for &dev in &app.draft.instances[i].devices {
        ui.horizontal(|ui| {
            ui.label("    ");
            ui.label(device_text(&app.input_devices[dev]));
            if ui.button("🗑").clicked() {
                to_remove.push((i, dev));
            }
        });
    }
}

fn device_text(device: &InputDevice) -> RichText {
    let text = RichText::new(format!("{} {}", device.emoji(), device.fancyname()));
    if device.has_button_held() {
        text.strong()
    } else {
        text
    }
}

fn footer(app: &mut PartyApp, ui: &mut Ui) {
    let start = bottom_footer(ui, true, |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(icons::BTN_START).max_height(16.0));
            ui.button("Start").clicked()
        })
        .inner
    });
    if start {
        app.prepare_game_launch();
    }
}
