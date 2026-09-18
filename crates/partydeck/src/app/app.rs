use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use eframe::egui;

use super::dialogs::msg;
use super::job::{self, BackgroundJob, JOB_TIMEOUT, REPAINT_INTERVAL};
use super::state::{HandlerLibrary, MenuPage, ProfileChoices, SessionDraft, SettingsPage};
use super::widgets::disabled_while_busy;
use super::{pages, panels};
use crate::config::{PartyConfig, load_cfg};
use crate::handler::{Handler, scan_handlers};
use crate::input::{InputDevice, scan_input_devices};
use crate::monitor::Monitor;
use crate::profile::scan_profiles;
use crate::steam::get_installed_steamapps;
use crate::update::check_for_partydeck_update;

pub struct PartyApp {
    /// Leading None is the "no Steam app" dropdown entry.
    pub(super) installed_steamapps: Vec<Option<steamlocate::App>>,
    pub(super) needs_update: Arc<AtomicBool>,
    pub(super) cfg: PartyConfig,
    pub(super) cur_page: MenuPage,
    pub(super) settings_page: SettingsPage,
    /// Text of the bottom info panel.
    pub(super) info_text: String,

    pub(super) monitors: Vec<Monitor>,
    pub(super) input_devices: Vec<InputDevice>,
    pub(super) draft: SessionDraft,
    /// Profile directory names, without the guest entry.
    pub(super) profiles: Vec<String>,
    pub(super) profile_choices: ProfileChoices,

    pub(super) handlers: HandlerLibrary,
    pub(super) handler_edit: Option<Handler>,
    /// Throwaway handler from `--exec`; replaces the handler library.
    pub(super) exec_handler: Option<Handler>,

    pub(super) job: Option<BackgroundJob>,
}

impl PartyApp {
    pub fn new(monitors: Vec<Monitor>, exec_handler: Option<Handler>) -> Self {
        let cfg = load_cfg();
        let input_devices = scan_input_devices(&cfg.pad_filter_type);
        let (handlers, cur_page) = match exec_handler {
            Some(_) => (Vec::new(), MenuPage::Instances),
            None => (scan_handlers(), MenuPage::Home),
        };
        let installed_steamapps = std::iter::once(None)
            .chain(get_installed_steamapps().into_iter().map(Some))
            .collect();

        let mut app = Self {
            installed_steamapps,
            needs_update: Arc::new(AtomicBool::new(false)),
            cfg,
            cur_page,
            settings_page: SettingsPage::General,
            info_text: String::new(),
            monitors,
            input_devices,
            draft: SessionDraft::default(),
            profiles: Vec::new(),
            profile_choices: ProfileChoices::default(),
            handlers: HandlerLibrary::new(handlers),
            handler_edit: None,
            exec_handler,
            job: None,
        };
        app.refresh_profiles();

        if app.cfg.check_for_updates {
            let needs_update = app.needs_update.clone();
            app.start_job("Checking for updates", Some(JOB_TIMEOUT), move || {
                needs_update.store(check_for_partydeck_update(), Ordering::Relaxed);
            });
        }

        app
    }

    pub(super) fn is_lite(&self) -> bool {
        self.exec_handler.is_some()
    }

    pub(super) fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    /// Handler the Game page and a launch act on: the `--exec` one, else the
    /// selected library entry.
    pub(super) fn cur_handler(&self) -> Option<&Handler> {
        self.exec_handler
            .as_ref()
            .or_else(|| self.handlers.current())
    }

    pub(super) fn home_page(&self) -> MenuPage {
        if self.is_lite() {
            MenuPage::Instances
        } else {
            MenuPage::Home
        }
    }

    pub(super) fn rescan_input_devices(&mut self) {
        self.input_devices = scan_input_devices(&self.cfg.pad_filter_type);
    }

    /// The only place the profile list and the dropdown entries are rebuilt.
    pub(super) fn refresh_profiles(&mut self) {
        self.profiles = scan_profiles(false);
        self.profile_choices = ProfileChoices::from_names(&self.profiles);
    }

    pub(super) fn open_profiles_page(&mut self) {
        self.refresh_profiles();
        self.cur_page = MenuPage::Profiles;
    }

    pub(super) fn open_instances_page(&mut self) {
        self.draft.clear();
        self.refresh_profiles();
        self.cur_page = MenuPage::Instances;
    }

    pub(super) fn edit_handler(&mut self, handler: Handler) {
        self.handler_edit = Some(handler);
        self.cur_page = MenuPage::EditHandler;
    }

    /// Starts `f` on a worker thread; refused while another job runs.
    pub(super) fn start_job<F>(&mut self, msg: &str, timeout: Option<Duration>, f: F) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        if self.is_busy() {
            eprintln!("[partydeck] Ignoring \"{msg}\": another job is still running");
            return false;
        }
        self.job = Some(BackgroundJob::spawn(msg, timeout, f));
        true
    }

    fn poll_job(&mut self) {
        if let Some(abandoned) = job::poll(&mut self.job) {
            eprintln!("[partydeck] Operation timed out: {abandoned}");
            msg("Operation timed out", &abandoned);
        }
    }
}

impl eframe::App for PartyApp {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if !raw_input.focused || self.is_busy() {
            return;
        }
        match self.cur_page {
            MenuPage::Instances => self.handle_devices_instance_menu(),
            _ => self.handle_gamepad_gui(raw_input),
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        panels::top::show(self, ctx);
        if !self.is_lite() {
            panels::games::show(self, ctx);
        }
        if self.cur_page == MenuPage::Instances {
            panels::devices::show(self, ctx);
        }
        if self.cur_page.has_info_panel() {
            panels::info::show(self, ctx);
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            disabled_while_busy(ui, self.is_busy());
            pages::show(self, ui);
        });

        self.poll_job();
        if let Some(job) = &self.job {
            job::draw_overlay(ctx, job.msg());
        }
        if ctx.input(|input| input.focused) {
            ctx.request_repaint_after(REPAINT_INTERVAL);
        }
    }
}
