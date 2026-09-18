use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;

use super::dialogs::msg;
use crate::config::*;
use crate::handler::*;
use crate::input::*;
use crate::instance::Instance;
use crate::launch::request::default_layout;
use crate::launch::{LaunchRequest, run_launch};
use crate::monitor::Monitor;
use crate::profile::*;
use crate::steam::get_installed_steamapps;
use crate::update::check_for_partydeck_update;

use eframe::egui::{self, Key};

const LAUNCH_SETTLE_DELAY: std::time::Duration = std::time::Duration::from_millis(1500);
const TASK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Eq, PartialEq)]
pub enum MenuPage {
    Home,
    Settings,
    Profiles,
    EditHandler,
    Game,
    Instances,
}

#[derive(Eq, PartialEq)]
pub enum SettingsPage {
    General,
    Proton,
    Gamescope,
}

pub struct PartyApp {
    /// Leading None is the "no Steam app" dropdown entry.
    pub installed_steamapps: Vec<Option<steamlocate::App>>,
    pub needs_update: Arc<AtomicBool>,
    pub options: PartyConfig,
    pub cur_page: MenuPage,
    pub settings_page: SettingsPage,
    pub infotext: String,

    pub monitors: Vec<Monitor>,
    pub input_devices: Vec<InputDevice>,
    pub instances: Vec<Instance>,
    pub instance_add_dev: Option<usize>,
    pub profiles: Vec<String>,

    pub handlers: Vec<Handler>,
    pub selected_handler: usize,
    pub handler_edit: Option<Handler>,
    pub handler_lite: Option<Handler>,

    pub loading_msg: Option<String>,
    pub loading_since: Option<std::time::Instant>,
    pub task: Option<std::thread::JoinHandle<()>>,
}

impl PartyApp {
    pub fn new(monitors: Vec<Monitor>, handler_lite: Option<Handler>) -> Self {
        let options = load_cfg();
        let input_devices = scan_input_devices(&options.pad_filter_type);
        let handlers = match handler_lite {
            Some(_) => Vec::new(),
            None => scan_handlers(),
        };
        let cur_page = match handler_lite {
            Some(_) => MenuPage::Instances,
            None => MenuPage::Home,
        };
        let installed_steamapps = std::iter::once(None)
            .chain(get_installed_steamapps().into_iter().map(Some))
            .collect();

        let mut app = Self {
            installed_steamapps,
            needs_update: Arc::new(AtomicBool::new(false)),
            options,
            cur_page,
            settings_page: SettingsPage::General,
            infotext: String::new(),
            monitors,
            input_devices,
            instances: Vec::new(),
            instance_add_dev: None,
            handlers,
            selected_handler: 0,
            handler_edit: None,
            handler_lite,
            profiles: scan_profiles(false),
            loading_msg: None,
            loading_since: None,
            task: None,
        };

        if app.options.check_for_updates {
            let needs_update = app.needs_update.clone();
            app.spawn_task("Checking for updates", move || {
                needs_update.store(check_for_partydeck_update(), Ordering::Relaxed);
            });
        }

        app
    }
}

impl eframe::App for PartyApp {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if !raw_input.focused || self.task.is_some() {
            return;
        }
        match self.cur_page {
            MenuPage::Instances => self.handle_devices_instance_menu(),
            _ => self.handle_gamepad_gui(raw_input),
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu_nav_panel").show(ctx, |ui| {
            if self.task.is_some() {
                ui.disable();
            }
            self.display_panel_top(ui);
        });

        if !self.is_lite() {
            egui::SidePanel::left("games_panel")
                .resizable(false)
                .exact_width(200.0)
                .show(ctx, |ui| {
                    if self.task.is_some() {
                        ui.disable();
                    }
                    self.display_panel_left(ui);
                });
        }

        if self.cur_page == MenuPage::Instances {
            egui::SidePanel::right("devices_panel")
                .resizable(false)
                .exact_width(180.0)
                .show(ctx, |ui| {
                    if self.task.is_some() {
                        ui.disable();
                    }
                    self.display_panel_right(ui, ctx);
                });
        }

        if (self.cur_page != MenuPage::Home) && (self.cur_page != MenuPage::Instances) {
            self.display_panel_bottom(ctx);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.task.is_some() {
                ui.disable();
            }
            match self.cur_page {
                MenuPage::Home => self.display_page_main(ui),
                MenuPage::Settings => self.display_page_settings(ui),
                MenuPage::Profiles => self.display_page_profiles(ui),
                MenuPage::EditHandler => self.display_page_edit_handler(ui),
                MenuPage::Game => self.display_page_game(ui),
                MenuPage::Instances => self.display_page_instances(ui),
            }
        });

        self.poll_task();
        if let Some(msg) = &self.loading_msg {
            egui::Area::new("loading".into())
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 192))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(16, 12))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add(egui::widgets::Spinner::new().size(40.0));
                                ui.add_space(8.0);
                                ui.label(msg);
                            });
                        });
                });
        }
        if ctx.input(|input| input.focused) {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
    }
}

impl PartyApp {
    pub fn spawn_task<F>(&mut self, msg: &str, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.loading_msg = Some(msg.to_string());
        self.loading_since = Some(std::time::Instant::now());
        self.task = Some(std::thread::spawn(f));
    }

    fn poll_task(&mut self) {
        if let Some(handle) = self.task.take() {
            if handle.is_finished() {
                let _ = handle.join();
                self.loading_since = None;
                self.loading_msg = None;
            } else {
                self.task = Some(handle);
            }
        }
        if let Some(start) = self.loading_since
            && start.elapsed() > TASK_TIMEOUT
        {
            self.loading_msg = Some("Operation timed out".to_string());
        }
    }

    pub fn is_lite(&self) -> bool {
        self.handler_lite.is_some()
    }

    fn handle_gamepad_gui(&mut self, raw_input: &mut egui::RawInput) {
        let mut key: Option<egui::Key> = None;
        for pad in &mut self.input_devices {
            if !pad.enabled() {
                continue;
            }
            match pad.poll() {
                Some(PadButton::ABtn) => key = Some(Key::Enter),
                Some(PadButton::BBtn) => {
                    self.cur_page = if self.handler_lite.is_some() {
                        MenuPage::Instances
                    } else {
                        MenuPage::Home
                    };
                }
                Some(PadButton::XBtn) => {
                    self.profiles = scan_profiles(false);
                    self.cur_page = MenuPage::Profiles;
                }
                Some(PadButton::YBtn) => self.cur_page = MenuPage::Settings,
                Some(PadButton::SelectBtn) => key = Some(Key::Tab),
                Some(PadButton::StartBtn) => {
                    if self.cur_page == MenuPage::Game {
                        self.instances.clear();
                        self.profiles = scan_profiles(true);
                        self.instance_add_dev = None;
                        self.cur_page = MenuPage::Instances;
                    }
                }
                Some(PadButton::Up) => key = Some(Key::ArrowUp),
                Some(PadButton::Down) => key = Some(Key::ArrowDown),
                Some(PadButton::Left) => key = Some(Key::ArrowLeft),
                Some(PadButton::Right) => key = Some(Key::ArrowRight),
                Some(_) | None => {}
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

    fn handle_devices_instance_menu(&mut self) {
        for i in 0..self.input_devices.len() {
            if !self.input_devices[i].enabled() {
                continue;
            }
            match self.input_devices[i].poll() {
                Some(PadButton::ABtn) | Some(PadButton::ZKey) | Some(PadButton::RightClick) => {
                    self.add_device_to_instances(i);
                }
                Some(PadButton::BBtn) | Some(PadButton::XKey) => {
                    if self.instance_add_dev.is_some() {
                        self.instance_add_dev = None;
                    } else if self.is_device_in_any_instance(i) {
                        self.remove_device(i);
                    } else if self.instances.is_empty() {
                        self.cur_page = MenuPage::Game;
                    }
                }
                Some(PadButton::YBtn) | Some(PadButton::AKey) => {
                    if self.instance_add_dev.is_none()
                        && let Some((instance, _)) = self.find_device_in_instance(i)
                    {
                        self.instance_add_dev = Some(instance);
                    }
                }
                Some(PadButton::StartBtn) => {
                    if !self.instances.is_empty() && self.is_device_in_any_instance(i) {
                        self.prepare_game_launch();
                    }
                }
                _ => {}
            }
        }
    }

    fn add_device_to_instances(&mut self, dev: usize) {
        let is_gamepad = self.input_devices[dev].device_type() == DeviceType::Gamepad;
        if !is_gamepad && !self.options.kbm_support {
            return;
        }
        if !self.options.allow_multiple_instances_on_same_device
            && self.is_device_in_any_instance(dev)
        {
            return;
        }
        // The custom gamescope cannot hold one keyboard/mouse for several
        // instances yet, so those devices stay exclusive to one instance.
        if !is_gamepad && self.is_device_in_any_instance(dev) {
            return;
        }

        match self.instance_add_dev {
            Some(inst) => {
                if !self.is_device_in_instance(inst, dev) {
                    self.instance_add_dev = None;
                    self.instances[inst].devices.push(dev);
                }
            }
            None => self.instances.push(Instance::new(vec![dev])),
        }
    }

    fn is_device_in_any_instance(&self, dev: usize) -> bool {
        self.instances
            .iter()
            .any(|instance| instance.devices.contains(&dev))
    }

    fn is_device_in_instance(&self, instance_index: usize, dev: usize) -> bool {
        self.instances
            .get(instance_index)
            .is_some_and(|instance| instance.devices.contains(&dev))
    }

    fn find_device_in_instance(&self, dev: usize) -> Option<(usize, usize)> {
        self.instances.iter().enumerate().find_map(|(i, instance)| {
            instance
                .devices
                .iter()
                .position(|d| *d == dev)
                .map(|d| (i, d))
        })
    }

    fn find_device_in_instance_from_end(&self, dev: usize) -> Option<(usize, usize)> {
        self.instances
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, instance)| {
                instance
                    .devices
                    .iter()
                    .position(|d| *d == dev)
                    .map(|d| (i, d))
            })
    }

    pub fn remove_device(&mut self, dev: usize) {
        if let Some((instance_index, device_index)) = self.find_device_in_instance_from_end(dev) {
            self.instances[instance_index].devices.remove(device_index);
            if self.instances[instance_index].devices.is_empty() {
                self.instances.remove(instance_index);
            }
        }
    }

    pub fn remove_device_instance(&mut self, instance_index: usize, dev: usize) {
        let Some(instance) = self.instances.get_mut(instance_index) else {
            return;
        };
        if let Some(d) = instance.devices.iter().position(|device| *device == dev) {
            instance.devices.remove(d);
            if instance.devices.is_empty() {
                self.instances.remove(instance_index);
            }
        }
    }

    pub fn prepare_game_launch(&mut self) {
        let handler = match &self.handler_lite {
            Some(h) => h.clone(),
            None => match self.handlers.get(self.selected_handler) {
                Some(h) => h.clone(),
                None => return,
            },
        };
        let Some(monitor) = self.monitors.first().cloned() else {
            msg("Launch Error", "No monitor detected");
            return;
        };

        let dev_infos: Vec<DeviceInfo> = self.input_devices.iter().map(|p| p.info()).collect();
        let cfg = self.options.clone();
        if let Err(e) = save_cfg(&cfg) {
            eprintln!("[partydeck] Couldn't save settings before launch: {e}");
        }
        let layout = default_layout(&cfg.layout_preset, self.instances.len());
        let request = LaunchRequest::new(
            handler,
            self.instances.clone(),
            dev_infos,
            cfg,
            monitor,
            layout,
        );

        self.cur_page = MenuPage::Home;
        self.spawn_task(
            "Launching...\n\nDon't press any buttons or move any analog sticks or mice.",
            move || {
                sleep(LAUNCH_SETTLE_DELAY);
                if let Err(err) = run_launch(request) {
                    eprintln!("[partydeck] Launch error: {err}");
                    msg("Launch Error", &format!("{err}"));
                }
            },
        );
    }
}
