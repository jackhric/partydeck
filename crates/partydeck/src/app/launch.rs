use std::time::Duration;

use super::app::PartyApp;
use super::dialogs::msg;
use super::state::MenuPage;
use crate::config::save_cfg;
use crate::input::DeviceInfo;
use crate::launch::request::default_layout;
use crate::launch::{LaunchRequest, run_launch};

/// Gives the input devices time to settle after the last button press.
const LAUNCH_SETTLE_DELAY: Duration = Duration::from_millis(1500);
const LAUNCH_MSG: &str =
    "Launching...\n\nDon't press any buttons or move any analog sticks or mice.";

impl PartyApp {
    /// Builds the launch request from the draft session and runs it in the
    /// background. The job has no timeout: it lives as long as the game does.
    pub(super) fn prepare_game_launch(&mut self) {
        if self.is_busy() {
            return;
        }
        let Some(handler) = self.cur_handler().cloned() else {
            return;
        };
        let Some(monitor) = self.monitors.first().cloned() else {
            msg("Launch Error", "No monitor detected");
            return;
        };

        let dev_infos: Vec<DeviceInfo> = self.input_devices.iter().map(|p| p.info()).collect();
        let cfg = self.cfg.clone();
        if let Err(e) = save_cfg(&cfg) {
            eprintln!("[partydeck] Couldn't save settings before launch: {e}");
        }
        let layout = default_layout(&cfg.layout_preset, self.draft.instances.len());
        let request = LaunchRequest::new(
            handler,
            self.draft.instances.clone(),
            dev_infos,
            cfg,
            monitor,
            layout,
        );

        self.cur_page = MenuPage::Home;
        self.start_job(LAUNCH_MSG, None, move || {
            std::thread::sleep(LAUNCH_SETTLE_DELAY);
            if let Err(err) = run_launch(request) {
                eprintln!("[partydeck] Launch error: {err}");
                msg("Launch Error", &format!("{err}"));
            }
        });
    }
}
