use std::collections::HashMap;
use std::io;
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use evdev::uinput::VirtualDevice;
use evdev::*;

use super::pad::{neutral_frame, split_frames};
use crate::input::{STEAM_INPUT_VENDOR, is_proxy_phys, steam_pad_name};

const POLL_TIMEOUT_MS: libc::c_int = 100;
const TICKS_BETWEEN_SCANS: u32 = 5;

pub(super) struct Router {
    proxy: VirtualDevice,
    slot: u32,
    source: Option<Device>,
    pending: Vec<InputEvent>,
    effect_data: HashMap<i16, FFEffectData>,
    live: HashMap<i16, FFEffect>,
    ticks_since_scan: u32,
    shutdown: Arc<AtomicBool>,
}

impl Router {
    pub(super) fn new(
        proxy: VirtualDevice,
        slot: u32,
        source_path: Option<&str>,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        let source = source_path.and_then(|p| {
            Device::open(p)
                .and_then(|dev| dev.set_nonblocking(true).map(|_| dev))
                .ok()
        });
        Router {
            proxy,
            slot,
            source,
            pending: Vec::new(),
            effect_data: HashMap::new(),
            live: HashMap::new(),
            ticks_since_scan: 0,
            shutdown,
        }
    }

    pub(super) fn run(mut self) {
        while !self.shutdown.load(Ordering::Relaxed) {
            self.tick();
        }
    }

    fn tick(&mut self) {
        let mut fds = [libc::pollfd {
            fd: -1,
            events: libc::POLLIN,
            revents: 0,
        }; 2];
        let mut n = 0;
        let source_idx = self.source.as_ref().map(|src| {
            fds[n].fd = src.as_raw_fd();
            n += 1;
            n - 1
        });
        let proxy_idx = n;
        fds[proxy_idx].fd = self.proxy.as_raw_fd();
        n += 1;

        let ret = unsafe { libc::poll(fds.as_mut_ptr(), n as libc::nfds_t, POLL_TIMEOUT_MS) };
        if ret > 0 {
            if let Some(idx) = source_idx
                && fds[idx].revents != 0
            {
                self.forward_source_events();
            }
            if fds[proxy_idx].revents != 0 {
                self.handle_proxy_events();
            }
        }

        if self.source.is_none() {
            self.ticks_since_scan += 1;
            if self.ticks_since_scan >= TICKS_BETWEEN_SCANS {
                self.ticks_since_scan = 0;
                self.try_attach_source();
            }
        }
    }

    fn forward_source_events(&mut self) {
        let Some(source) = &mut self.source else {
            return;
        };
        let events: Result<Vec<InputEvent>, io::Error> =
            source.fetch_events().map(|events| events.collect());
        match events {
            Ok(events) => {
                for frame in split_frames(&mut self.pending, &events) {
                    let _ = self.proxy.emit(&frame);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => self.on_source_lost(),
        }
    }

    fn on_source_lost(&mut self) {
        self.source = None;
        self.live.clear();
        self.pending.clear();
        self.ticks_since_scan = 0;
        let _ = self.proxy.emit(&neutral_frame());
    }

    fn try_attach_source(&mut self) {
        let name = steam_pad_name(self.slot);
        for (_, mut dev) in evdev::enumerate() {
            if dev.input_id().vendor() != STEAM_INPUT_VENDOR
                || dev.name() != Some(name.as_str())
                || is_proxy_phys(dev.physical_path())
                || dev.set_nonblocking(true).is_err()
            {
                continue;
            }
            self.live.clear();
            for (&id, &data) in &self.effect_data {
                if let Ok(effect) = dev.upload_ff_effect(data) {
                    self.live.insert(id, effect);
                }
            }
            self.source = Some(dev);
            return;
        }
    }

    fn handle_proxy_events(&mut self) {
        let events: Vec<InputEvent> = match self.proxy.fetch_events() {
            Ok(events) => events.collect(),
            Err(_) => return,
        };
        for event in events {
            match event.destructure() {
                EventSummary::UInput(ev, UInputCode::UI_FF_UPLOAD, _) => self.handle_ff_upload(ev),
                EventSummary::UInput(ev, UInputCode::UI_FF_ERASE, _) => self.handle_ff_erase(ev),
                EventSummary::ForceFeedback(_, code, value) => {
                    if let Some(effect) = self.live.get_mut(&(code.0 as i16)) {
                        let _ = if value == 0 {
                            effect.stop()
                        } else {
                            effect.play(value)
                        };
                    }
                }
                _ => {}
            }
        }
    }

    // The game's ioctl blocks until the upload object drops, so this stays
    // straight-line: record, mirror to the source, ack, drop.
    fn handle_ff_upload(&mut self, event: UInputEvent) {
        let Ok(mut upload) = self.proxy.process_ff_upload(event) else {
            return;
        };
        let id = upload.effect_id();
        let data = upload.effect();
        self.effect_data.insert(id, data);
        if let Some(source) = &mut self.source {
            match self.live.get_mut(&id) {
                Some(effect) => {
                    let _ = effect.update(data);
                }
                None => {
                    if let Ok(effect) = source.upload_ff_effect(data) {
                        self.live.insert(id, effect);
                    }
                }
            }
        }
        upload.set_retval(0);
    }

    fn handle_ff_erase(&mut self, event: UInputEvent) {
        let Ok(mut erase) = self.proxy.process_ff_erase(event) else {
            return;
        };
        let id = erase.effect_id() as i16;
        self.effect_data.remove(&id);
        self.live.remove(&id);
        erase.set_retval(0);
    }
}
