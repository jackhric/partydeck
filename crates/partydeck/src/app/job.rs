use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use eframe::egui;

pub const JOB_TIMEOUT: Duration = Duration::from_secs(60);
pub const REPAINT_INTERVAL: Duration = Duration::from_millis(33);

const OVERLAY_SPINNER_SIZE: f32 = 40.0;
const OVERLAY_FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 0, 0, 192);

/// Work that runs off the UI thread while the launcher is disabled.
pub struct BackgroundJob {
    handle: JoinHandle<()>,
    msg: String,
    since: Instant,
    timeout: Option<Duration>,
}

impl BackgroundJob {
    pub fn spawn<F>(msg: &str, timeout: Option<Duration>, f: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        BackgroundJob {
            handle: std::thread::spawn(f),
            msg: msg.to_string(),
            since: Instant::now(),
            timeout,
        }
    }

    pub fn msg(&self) -> &str {
        &self.msg
    }

    pub fn finished(&self) -> bool {
        self.handle.is_finished()
    }

    pub fn timed_out(&self) -> bool {
        self.timeout
            .is_some_and(|limit| self.since.elapsed() > limit)
    }
}

/// Clears a finished or overdue job from `slot`. An overdue job is abandoned
/// (its thread keeps running detached) and its message is returned so the
/// caller can report it.
pub fn poll(slot: &mut Option<BackgroundJob>) -> Option<String> {
    let job = slot.as_ref()?;
    if job.finished() {
        if let Some(job) = slot.take() {
            let _ = job.handle.join();
        }
        return None;
    }
    if job.timed_out() {
        return slot.take().map(|job| job.msg);
    }
    None
}

pub fn draw_overlay(ctx: &egui::Context, msg: &str) {
    egui::Area::new("loading".into())
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(OVERLAY_FILL)
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add(egui::widgets::Spinner::new().size(OVERLAY_SPINNER_SIZE));
                        ui.add_space(8.0);
                        ui.label(msg);
                    });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finished_job_is_cleared_without_message() {
        let mut slot = Some(BackgroundJob::spawn("noop", Some(JOB_TIMEOUT), || {}));
        while slot.as_ref().is_some_and(|job| !job.finished()) {
            std::thread::yield_now();
        }
        assert_eq!(poll(&mut slot), None);
        assert!(slot.is_none());
        assert_eq!(poll(&mut slot), None);
    }

    #[test]
    fn overdue_job_is_abandoned_with_its_message() {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let mut slot = Some(BackgroundJob::spawn(
            "slow",
            Some(Duration::ZERO),
            move || {
                let _ = rx.recv();
            },
        ));
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(poll(&mut slot), Some("slow".to_string()));
        assert!(slot.is_none());
        drop(tx);
    }

    #[test]
    fn untimed_job_never_times_out() {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let mut slot = Some(BackgroundJob::spawn("forever", None, move || {
            let _ = rx.recv();
        }));
        assert_eq!(poll(&mut slot), None);
        assert!(slot.is_some());
        drop(tx);
    }
}
