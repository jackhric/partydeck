use eframe::egui::{self, Key};

use super::app::PartyApp;
use super::state::{MenuPage, SessionDraft};
use crate::config::PartyConfig;
use crate::input::{DeviceType, PadButton};
use crate::instance::Instance;

enum GuiAction {
    Key(Key),
    GoHome,
    GoProfiles,
    GoSettings,
    StartSession,
}

fn gui_action(button: PadButton) -> Option<GuiAction> {
    Some(match button {
        PadButton::ABtn => GuiAction::Key(Key::Enter),
        PadButton::BBtn => GuiAction::GoHome,
        PadButton::XBtn => GuiAction::GoProfiles,
        PadButton::YBtn => GuiAction::GoSettings,
        PadButton::SelectBtn => GuiAction::Key(Key::Tab),
        PadButton::StartBtn => GuiAction::StartSession,
        PadButton::Up => GuiAction::Key(Key::ArrowUp),
        PadButton::Down => GuiAction::Key(Key::ArrowDown),
        PadButton::Left => GuiAction::Key(Key::ArrowLeft),
        PadButton::Right => GuiAction::Key(Key::ArrowRight),
        _ => return None,
    })
}

impl PartyApp {
    /// Turns pad presses into egui key events and page shortcuts.
    pub(super) fn handle_gamepad_gui(&mut self, raw_input: &mut egui::RawInput) {
        let actions: Vec<GuiAction> = self
            .input_devices
            .iter_mut()
            .filter(|pad| pad.enabled())
            .filter_map(|pad| pad.poll().and_then(gui_action))
            .collect();

        let mut key: Option<Key> = None;
        for action in actions {
            match action {
                GuiAction::Key(k) => key = Some(k),
                GuiAction::GoHome => self.cur_page = self.home_page(),
                GuiAction::GoProfiles => self.open_profiles_page(),
                GuiAction::GoSettings => self.cur_page = MenuPage::Settings,
                GuiAction::StartSession => {
                    if self.cur_page == MenuPage::Game {
                        self.open_instances_page();
                    }
                }
            }
        }

        if let Some(key) = key {
            raw_input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            });
        }
    }

    /// On the Instances page devices claim instances with their own buttons.
    pub(super) fn handle_devices_instance_menu(&mut self) {
        let rules = DeviceRules::from_cfg(&self.cfg);
        for dev in 0..self.input_devices.len() {
            if !self.input_devices[dev].enabled() {
                continue;
            }
            let Some(button) = self.input_devices[dev].poll() else {
                continue;
            };
            let is_gamepad = self.input_devices[dev].device_type() == DeviceType::Gamepad;
            match button {
                PadButton::ABtn | PadButton::ZKey | PadButton::RightClick => {
                    self.draft.add_device(dev, is_gamepad, rules);
                }
                PadButton::BBtn | PadButton::XKey => {
                    if self.draft.adding_device_to.is_some() {
                        self.draft.cancel_adding();
                    } else if self.draft.contains_device(dev) {
                        self.draft.remove_device(dev);
                    } else if self.draft.instances.is_empty() && !self.is_lite() {
                        self.cur_page = MenuPage::Game;
                    }
                }
                PadButton::YBtn | PadButton::AKey => self.draft.begin_adding_from(dev),
                PadButton::StartBtn => {
                    if !self.draft.instances.is_empty() && self.draft.contains_device(dev) {
                        self.prepare_game_launch();
                    }
                }
                _ => {}
            }
        }
    }
}

/// Config flags that decide which devices may join which instances.
#[derive(Clone, Copy, Debug)]
pub struct DeviceRules {
    pub kbm_support: bool,
    pub allow_same_device: bool,
}

impl DeviceRules {
    pub fn from_cfg(cfg: &PartyConfig) -> Self {
        DeviceRules {
            kbm_support: cfg.kbm_support,
            allow_same_device: cfg.allow_multiple_instances_on_same_device,
        }
    }
}

impl SessionDraft {
    pub fn clear(&mut self) {
        self.instances.clear();
        self.adding_device_to = None;
    }

    pub fn cancel_adding(&mut self) {
        self.adding_device_to = None;
    }

    pub fn contains_device(&self, dev: usize) -> bool {
        self.instances.iter().any(|i| i.devices.contains(&dev))
    }

    /// (instance index, position within it) of the first instance holding `dev`.
    pub fn find_device(&self, dev: usize) -> Option<(usize, usize)> {
        self.instances
            .iter()
            .enumerate()
            .find_map(|(i, inst)| inst.devices.iter().position(|d| *d == dev).map(|d| (i, d)))
    }

    fn find_device_from_end(&self, dev: usize) -> Option<(usize, usize)> {
        self.instances
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, inst)| inst.devices.iter().position(|d| *d == dev).map(|d| (i, d)))
    }

    /// Adds `dev` to the instance being extended, or as a new instance.
    /// Returns false when the rules reject the device.
    pub fn add_device(&mut self, dev: usize, is_gamepad: bool, rules: DeviceRules) -> bool {
        if !is_gamepad && !rules.kbm_support {
            return false;
        }
        if !rules.allow_same_device && self.contains_device(dev) {
            return false;
        }
        // The custom gamescope cannot hold one keyboard/mouse for several
        // instances yet, so those devices stay exclusive to one instance.
        if !is_gamepad && self.contains_device(dev) {
            return false;
        }

        match self.adding_device_to {
            Some(target) => {
                let Some(instance) = self.instances.get_mut(target) else {
                    self.adding_device_to = None;
                    return false;
                };
                if instance.devices.contains(&dev) {
                    return false;
                }
                instance.devices.push(dev);
                self.adding_device_to = None;
            }
            None => self.instances.push(Instance::new(vec![dev])),
        }
        true
    }

    /// Marks the instance that already holds `dev` as the target for the next add.
    pub fn begin_adding_from(&mut self, dev: usize) {
        if self.adding_device_to.is_none()
            && let Some((instance, _)) = self.find_device(dev)
        {
            self.adding_device_to = Some(instance);
        }
    }

    /// Removes the last occurrence of `dev`; an emptied instance goes with it.
    pub fn remove_device(&mut self, dev: usize) {
        if let Some((instance, position)) = self.find_device_from_end(dev) {
            self.remove_device_at(instance, position);
        }
    }

    pub fn remove_device_from(&mut self, instance: usize, dev: usize) {
        if let Some(position) = self
            .instances
            .get(instance)
            .and_then(|inst| inst.devices.iter().position(|d| *d == dev))
        {
            self.remove_device_at(instance, position);
        }
    }

    fn remove_device_at(&mut self, instance: usize, position: usize) {
        self.instances[instance].devices.remove(position);
        if self.instances[instance].devices.is_empty() {
            self.instances.remove(instance);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: DeviceRules = DeviceRules {
        kbm_support: true,
        allow_same_device: false,
    };

    fn devices(draft: &SessionDraft) -> Vec<Vec<usize>> {
        draft.instances.iter().map(|i| i.devices.clone()).collect()
    }

    #[test]
    fn adding_devices_creates_instances_and_rejects_duplicates() {
        let mut draft = SessionDraft::default();
        assert!(draft.add_device(0, true, DEFAULT));
        assert!(draft.add_device(1, true, DEFAULT));
        assert!(!draft.add_device(0, true, DEFAULT));
        assert_eq!(devices(&draft), vec![vec![0], vec![1]]);
    }

    #[test]
    fn same_device_rule_allows_duplicate_gamepads_only() {
        let rules = DeviceRules {
            kbm_support: true,
            allow_same_device: true,
        };
        let mut draft = SessionDraft::default();
        assert!(draft.add_device(0, true, rules));
        assert!(draft.add_device(0, true, rules));
        assert!(draft.add_device(5, false, rules));
        assert!(!draft.add_device(5, false, rules));
        assert_eq!(devices(&draft), vec![vec![0], vec![0], vec![5]]);
    }

    #[test]
    fn kbm_gating_rejects_non_gamepads() {
        let rules = DeviceRules {
            kbm_support: false,
            allow_same_device: false,
        };
        let mut draft = SessionDraft::default();
        assert!(!draft.add_device(3, false, rules));
        assert!(draft.add_device(3, true, rules));
        assert_eq!(devices(&draft), vec![vec![3]]);
    }

    #[test]
    fn adding_to_an_existing_instance() {
        let mut draft = SessionDraft::default();
        draft.add_device(0, true, DEFAULT);
        draft.add_device(1, true, DEFAULT);
        draft.begin_adding_from(1);
        assert_eq!(draft.adding_device_to, Some(1));
        assert!(draft.add_device(2, true, DEFAULT));
        assert_eq!(draft.adding_device_to, None);
        assert_eq!(devices(&draft), vec![vec![0], vec![1, 2]]);

        draft.begin_adding_from(9);
        assert_eq!(draft.adding_device_to, None);
        draft.adding_device_to = Some(4);
        assert!(!draft.add_device(3, true, DEFAULT));
        assert_eq!(draft.adding_device_to, None);
        assert_eq!(draft.instances.len(), 2);
    }

    #[test]
    fn removing_a_device_removes_an_emptied_instance() {
        let mut draft = SessionDraft::default();
        draft.add_device(0, true, DEFAULT);
        draft.add_device(1, true, DEFAULT);
        draft.begin_adding_from(1);
        draft.add_device(2, true, DEFAULT);
        draft.remove_device(2);
        assert_eq!(devices(&draft), vec![vec![0], vec![1]]);
        draft.remove_device_from(0, 0);
        assert_eq!(devices(&draft), vec![vec![1]]);
        draft.remove_device_from(7, 1);
        draft.remove_device(1);
        assert!(draft.instances.is_empty());
        assert!(!draft.contains_device(1));
    }
}
