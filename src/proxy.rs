//! Per-player proxy gamepads. Steam Input destroys and recreates its virtual
//! pad on controller churn, so each game instead gets a stable uinput pad we
//! own; a router thread per player forwards events and force feedback, and
//! re-attaches to the slot-N Steam pad whenever it reappears.

use std::collections::HashMap;
use std::ffi::CString;
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::*;

use crate::app::PartyConfig;
use crate::handler::Handler;
use crate::input::{DeviceInfo, DeviceType};
use crate::instance::Instance;

pub const PROXY_PHYS_PREFIX: &str = "partydeck-proxy";

const PROXY_KEYS: [KeyCode; 11] = [
    KeyCode::BTN_SOUTH,
    KeyCode::BTN_EAST,
    KeyCode::BTN_NORTH,
    KeyCode::BTN_WEST,
    KeyCode::BTN_TL,
    KeyCode::BTN_TR,
    KeyCode::BTN_SELECT,
    KeyCode::BTN_START,
    KeyCode::BTN_MODE,
    KeyCode::BTN_THUMBL,
    KeyCode::BTN_THUMBR,
];

// (code, min, max, fuzz, flat) — byte-exact copy of a real Steam Input pad's
// abs setup, taken from an on-device ioctl dump.
const PROXY_ABS: [(AbsoluteAxisCode, i32, i32, i32, i32); 8] = [
    (AbsoluteAxisCode::ABS_X, -32767, 32767, 16, 128),
    (AbsoluteAxisCode::ABS_Y, -32767, 32767, 16, 128),
    (AbsoluteAxisCode::ABS_RX, -32767, 32767, 16, 128),
    (AbsoluteAxisCode::ABS_RY, -32767, 32767, 16, 128),
    (AbsoluteAxisCode::ABS_Z, 0, 255, 0, 0),
    (AbsoluteAxisCode::ABS_RZ, 0, 255, 0, 0),
    (AbsoluteAxisCode::ABS_HAT0X, -1, 1, 0, 0),
    (AbsoluteAxisCode::ABS_HAT0Y, -1, 1, 0, 0),
];

// Steam's virtual pad advertises FF_RUMBLE only.
const PROXY_FF: [FFEffectCode; 1] = [FFEffectCode::FF_RUMBLE];

const PROXY_FF_EFFECTS_MAX: u32 = 16;

pub fn is_proxy_phys(phys: Option<&str>) -> bool {
    phys.is_some_and(|p| p.starts_with(PROXY_PHYS_PREFIX))
}

// evdev 0.13.0's with_phys encodes UI_SET_PHYS with _IOC size 1 (c_char)
// instead of sizeof(char*), so the kernel rejects it with EINVAL. The builder
// keeps its uinput fd private; recover it from the Debug representation
// (verified against /proc/self/fd) and issue the correctly-encoded ioctl.
fn set_phys(builder: &VirtualDeviceBuilder, phys: &std::ffi::CStr) -> io::Result<()> {
    let debug = format!("{builder:?}");
    let fd: libc::c_int = debug
        .split("fd: ")
        .filter_map(|s| {
            let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .next()
        .ok_or_else(|| io::Error::other("could not recover uinput fd from builder"))?;
    if std::fs::read_link(format!("/proc/self/fd/{fd}"))? != Path::new("/dev/uinput") {
        return Err(io::Error::other("recovered fd is not /dev/uinput"));
    }
    let ui_set_phys: libc::c_ulong = (1 << 30)
        | ((std::mem::size_of::<*const libc::c_char>() as libc::c_ulong) << 16)
        | (0x55 << 8)
        | 108;
    if unsafe { libc::ioctl(fd, ui_set_phys as _, phys.as_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn build_proxy_pad(slot: u32) -> io::Result<VirtualDevice> {
    let name = format!("Microsoft X-Box 360 pad {slot}");
    let phys = CString::new(format!("{PROXY_PHYS_PREFIX}/{slot}")).unwrap();

    let mut keys = AttributeSet::<KeyCode>::new();
    for key in PROXY_KEYS {
        keys.insert(key);
    }
    let mut ff = AttributeSet::<FFEffectCode>::new();
    for code in PROXY_FF {
        ff.insert(code);
    }

    let mut builder = VirtualDevice::builder()?
        .name(&name)
        .input_id(InputId::new(BusType::BUS_USB, 0x28de, 0x11ff, 0x0001));
    set_phys(&builder, &phys)?;
    builder = builder
        .with_keys(&keys)?
        .with_ff(&ff)?
        .with_ff_effects_max(PROXY_FF_EFFECTS_MAX);
    for (code, min, max, fuzz, flat) in PROXY_ABS {
        let abs = UinputAbsSetup::new(code, AbsInfo::new(0, min, max, fuzz, flat, 0));
        builder = builder.with_absolute_axis(&abs)?;
    }
    let device = builder.build()?;

    unsafe {
        let fd = device.as_raw_fd();
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(device)
}

fn read_dev_nodes(syspath: &Path) -> io::Result<Vec<String>> {
    let mut nodes = Vec::new();
    for entry in std::fs::read_dir(syspath)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with("event") && !name.starts_with("js") {
            continue;
        }
        let path = format!("/dev/input/{name}");
        if Path::new(&path).exists() {
            nodes.push(path);
        }
    }
    Ok(nodes)
}

// The event* node can lag behind device creation (devtmpfs race), and the jsN
// node lags further still; wait for an event node, then give jsN one grace
// re-read. jsN is best-effort.
pub fn wait_for_dev_nodes(proxy: &mut VirtualDevice) -> io::Result<Vec<String>> {
    let syspath = proxy.get_syspath()?;
    let deadline = Instant::now() + Duration::from_millis(1000);
    loop {
        let nodes = read_dev_nodes(&syspath)?;
        if nodes.iter().any(|n| n.starts_with("/dev/input/event")) {
            std::thread::sleep(Duration::from_millis(100));
            return read_dev_nodes(&syspath);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "proxy pad device nodes did not appear",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn neutral_frame() -> Vec<InputEvent> {
    let mut events = Vec::with_capacity(PROXY_KEYS.len() + PROXY_ABS.len());
    for key in PROXY_KEYS {
        events.push(InputEvent::new(EventType::KEY.0, key.0, 0));
    }
    for (axis, ..) in PROXY_ABS {
        events.push(InputEvent::new(EventType::ABSOLUTE.0, axis.0, 0));
    }
    events
}

// Appends new_events to pending and splits off SYN_REPORT-terminated frames,
// leaving any trailing partial frame in pending. SYN events themselves are
// dropped; VirtualDevice::emit re-terminates each frame.
fn split_frames(pending: &mut Vec<InputEvent>, new_events: &[InputEvent]) -> Vec<Vec<InputEvent>> {
    let mut frames = Vec::new();
    for &event in new_events {
        if event.event_type() == EventType::SYNCHRONIZATION {
            if event.code() == SynchronizationCode::SYN_REPORT.0 && !pending.is_empty() {
                frames.push(std::mem::take(pending));
            }
            continue;
        }
        pending.push(event);
    }
    frames
}

pub struct ProxySession {
    players: Vec<PlayerProxy>,
}

struct PlayerProxy {
    instance: usize,
    dev_nodes: Vec<String>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ProxySession {
    pub fn wanted(
        cfg: &PartyConfig,
        handler: &Handler,
        instances: &[Instance],
        devices: &[DeviceInfo],
    ) -> bool {
        cfg.proxy_gamepads
            && !handler.enable_hidraw
            && instances
                .iter()
                .flat_map(|instance| instance.devices.iter())
                .all(|&d| {
                    let dev = &devices[d];
                    !dev.enabled
                        || dev.device_type != DeviceType::Gamepad
                        || dev.xinput_slot.is_some()
                })
    }

    pub fn start(instances: &[Instance], devices: &[DeviceInfo]) -> io::Result<ProxySession> {
        let mut session = ProxySession { players: Vec::new() };
        for (i, instance) in instances.iter().enumerate() {
            for &d in &instance.devices {
                let dev = &devices[d];
                if !dev.enabled || dev.device_type != DeviceType::Gamepad {
                    continue;
                }
                let Some(slot) = dev.xinput_slot else { continue };

                let mut proxy = build_proxy_pad(slot)?;
                let dev_nodes = wait_for_dev_nodes(&mut proxy)?;
                let shutdown = Arc::new(AtomicBool::new(false));
                let router = Router::new(proxy, slot, &dev.path, shutdown.clone());
                let thread = std::thread::spawn(move || router.run());
                session.players.push(PlayerProxy {
                    instance: i,
                    dev_nodes,
                    shutdown,
                    thread: Some(thread),
                });
            }
        }
        Ok(session)
    }

    pub fn instance_dev_nodes(&self, instance: usize) -> Vec<&str> {
        self.players
            .iter()
            .filter(|p| p.instance == instance)
            .flat_map(|p| p.dev_nodes.iter().map(String::as_str))
            .collect()
    }
}

impl Drop for ProxySession {
    fn drop(&mut self) {
        for player in &self.players {
            player.shutdown.store(true, Ordering::Relaxed);
        }
        // The kernel destroys the uinput nodes when each router drops its
        // VirtualDevice, so joining is the whole cleanup.
        for player in &mut self.players {
            if let Some(thread) = player.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

struct Router {
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
    fn new(proxy: VirtualDevice, slot: u32, source_path: &str, shutdown: Arc<AtomicBool>) -> Self {
        let source = Device::open(source_path)
            .and_then(|dev| dev.set_nonblocking(true).map(|_| dev))
            .ok();
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

    fn run(mut self) {
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

        let ret = unsafe { libc::poll(fds.as_mut_ptr(), n as libc::nfds_t, 100) };
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
            if self.ticks_since_scan >= 5 {
                self.ticks_since_scan = 0;
                self.try_attach_source();
            }
        }
    }

    fn forward_source_events(&mut self) {
        let Some(source) = &mut self.source else { return };
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
        let name = format!("Microsoft X-Box 360 pad {}", self.slot);
        for (_, mut dev) in evdev::enumerate() {
            if dev.input_id().vendor() != 0x28de
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
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
            Err(_) => return,
        };
        for event in events {
            match event.destructure() {
                EventSummary::UInput(ev, UInputCode::UI_FF_UPLOAD, _) => self.handle_ff_upload(ev),
                EventSummary::UInput(ev, UInputCode::UI_FF_ERASE, _) => self.handle_ff_erase(ev),
                EventSummary::ForceFeedback(_, code, value) => {
                    if let Some(effect) = self.live.get_mut(&(code.0 as i16)) {
                        let _ = if value == 0 { effect.stop() } else { effect.play(value) };
                    }
                }
                _ => {}
            }
        }
    }

    // The game's ioctl blocks until the upload object drops, so this stays
    // straight-line: record, mirror to the source, ack, drop.
    fn handle_ff_upload(&mut self, event: UInputEvent) {
        let Ok(mut upload) = self.proxy.process_ff_upload(event) else { return };
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
        let Ok(mut erase) = self.proxy.process_ff_erase(event) else { return };
        let id = erase.effect_id() as i16;
        self.effect_data.remove(&id);
        self.live.remove(&id);
        erase.set_retval(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, value: i32) -> InputEvent {
        InputEvent::new(EventType::KEY.0, code.0, value)
    }

    fn abs(code: AbsoluteAxisCode, value: i32) -> InputEvent {
        InputEvent::new(EventType::ABSOLUTE.0, code.0, value)
    }

    fn syn() -> InputEvent {
        InputEvent::new(
            EventType::SYNCHRONIZATION.0,
            SynchronizationCode::SYN_REPORT.0,
            0,
        )
    }

    fn codes(frame: &[InputEvent]) -> Vec<(u16, u16, i32)> {
        frame
            .iter()
            .map(|e| (e.event_type().0, e.code(), e.value()))
            .collect()
    }

    #[test]
    fn split_frames_no_syn_keeps_pending() {
        let mut pending = Vec::new();
        let frames = split_frames(&mut pending, &[key(KeyCode::BTN_SOUTH, 1)]);
        assert!(frames.is_empty());
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn split_frames_single_frame() {
        let mut pending = Vec::new();
        let frames = split_frames(
            &mut pending,
            &[key(KeyCode::BTN_SOUTH, 1), abs(AbsoluteAxisCode::ABS_X, 5), syn()],
        );
        assert_eq!(frames.len(), 1);
        assert_eq!(
            codes(&frames[0]),
            vec![
                (EventType::KEY.0, KeyCode::BTN_SOUTH.0, 1),
                (EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, 5),
            ]
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn split_frames_two_frames_and_trailing_partial() {
        let mut pending = vec![key(KeyCode::BTN_EAST, 1)];
        let frames = split_frames(
            &mut pending,
            &[
                syn(),
                key(KeyCode::BTN_SOUTH, 1),
                syn(),
                key(KeyCode::BTN_SOUTH, 0),
            ],
        );
        assert_eq!(frames.len(), 2);
        assert_eq!(codes(&frames[0]), vec![(EventType::KEY.0, KeyCode::BTN_EAST.0, 1)]);
        assert_eq!(codes(&frames[1]), vec![(EventType::KEY.0, KeyCode::BTN_SOUTH.0, 1)]);
        assert_eq!(codes(&pending), vec![(EventType::KEY.0, KeyCode::BTN_SOUTH.0, 0)]);
    }

    #[test]
    fn split_frames_drops_empty_frames() {
        let mut pending = Vec::new();
        let frames = split_frames(&mut pending, &[syn(), syn()]);
        assert!(frames.is_empty());
        assert!(pending.is_empty());
    }

    #[test]
    fn neutral_frame_covers_all_capabilities() {
        let frame = neutral_frame();
        assert_eq!(frame.len(), PROXY_KEYS.len() + PROXY_ABS.len());
        for k in PROXY_KEYS {
            assert!(frame.iter().any(|e| e.event_type() == EventType::KEY
                && e.code() == k.0
                && e.value() == 0));
        }
        for (a, ..) in PROXY_ABS {
            assert!(frame.iter().any(|e| e.event_type() == EventType::ABSOLUTE
                && e.code() == a.0
                && e.value() == 0));
        }
    }

    #[test]
    fn proxy_phys_prefix_matching() {
        assert!(is_proxy_phys(Some("partydeck-proxy/0")));
        assert!(is_proxy_phys(Some("partydeck-proxy/12")));
        assert!(!is_proxy_phys(Some("usb-0000:04:00.3-2/input0")));
        assert!(!is_proxy_phys(Some("")));
        assert!(!is_proxy_phys(None));
    }

    fn device(device_type: DeviceType, enabled: bool, xinput_slot: Option<u32>) -> DeviceInfo {
        DeviceInfo {
            path: "/dev/input/event0".to_string(),
            hidraw_paths: Vec::new(),
            enabled,
            device_type,
            xinput_slot,
        }
    }

    fn instance(devices: Vec<usize>) -> Instance {
        Instance {
            devices,
            profname: String::new(),
            profselection: 0,
            monitor: 0,
            width: 0,
            height: 0,
        }
    }

    #[test]
    fn wanted_truth_table() {
        let cfg = PartyConfig::default();
        let cfg_off = PartyConfig {
            proxy_gamepads: false,
            ..PartyConfig::default()
        };
        let handler = Handler::default();
        let handler_hidraw = Handler {
            enable_hidraw: true,
            ..Handler::default()
        };

        let slotted = vec![device(DeviceType::Gamepad, true, Some(0))];
        let unslotted = vec![device(DeviceType::Gamepad, true, None)];
        let disabled_unslotted = vec![device(DeviceType::Gamepad, false, None)];
        let keyboard = vec![device(DeviceType::Keyboard, true, None)];
        let instances = vec![instance(vec![0])];

        assert!(ProxySession::wanted(&cfg, &handler, &instances, &slotted));
        assert!(!ProxySession::wanted(&cfg_off, &handler, &instances, &slotted));
        assert!(!ProxySession::wanted(&cfg, &handler_hidraw, &instances, &slotted));
        assert!(!ProxySession::wanted(&cfg, &handler, &instances, &unslotted));
        assert!(ProxySession::wanted(&cfg, &handler, &instances, &disabled_unslotted));
        assert!(ProxySession::wanted(&cfg, &handler, &instances, &keyboard));
        // Unslotted pad exists but is unreferenced by any instance.
        let mixed = vec![
            device(DeviceType::Gamepad, true, Some(0)),
            device(DeviceType::Gamepad, true, None),
        ];
        assert!(ProxySession::wanted(&cfg, &handler, &instances, &mixed));
        let both = vec![instance(vec![0]), instance(vec![1])];
        assert!(!ProxySession::wanted(&cfg, &handler, &both, &mixed));
        assert!(ProxySession::wanted(&cfg, &handler, &[], &mixed));
    }
}

#[cfg(test)]
mod uinput_tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;

    // Stand-in for a slot-N Steam Input pad: a uinput device with the right
    // name/vendor and no proxy phys. A service thread answers FF uploads (the
    // router's upload_ff_effect ioctl blocks until we do) and records activity.
    struct FakeSteamPad {
        device: Arc<Mutex<VirtualDevice>>,
        event_node: String,
        uploads: Arc<AtomicUsize>,
        plays: mpsc::Receiver<(u16, i32)>,
        shutdown: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl FakeSteamPad {
        fn new(slot: u32) -> FakeSteamPad {
            let name = format!("Microsoft X-Box 360 pad {slot}");
            let mut keys = AttributeSet::<KeyCode>::new();
            for key in PROXY_KEYS {
                keys.insert(key);
            }
            let mut ff = AttributeSet::<FFEffectCode>::new();
            for code in PROXY_FF {
                ff.insert(code);
            }
            let mut builder = VirtualDevice::builder()
                .unwrap()
                .name(&name)
                .input_id(InputId::new(BusType::BUS_USB, 0x28de, 0x11ff, 0x0001))
                .with_keys(&keys)
                .unwrap()
                .with_ff(&ff)
                .unwrap()
                .with_ff_effects_max(PROXY_FF_EFFECTS_MAX);
            for (code, min, max, fuzz, flat) in PROXY_ABS {
                let abs = UinputAbsSetup::new(code, AbsInfo::new(0, min, max, fuzz, flat, 0));
                builder = builder.with_absolute_axis(&abs).unwrap();
            }
            let mut device = builder.build().unwrap();
            unsafe {
                let fd = device.as_raw_fd();
                let flags = libc::fcntl(fd, libc::F_GETFL);
                assert!(libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) >= 0);
            }
            let event_node = wait_for_dev_nodes(&mut device)
                .unwrap()
                .into_iter()
                .find(|n| n.starts_with("/dev/input/event"))
                .unwrap();

            let device = Arc::new(Mutex::new(device));
            let uploads = Arc::new(AtomicUsize::new(0));
            let shutdown = Arc::new(AtomicBool::new(false));
            let (play_tx, plays) = mpsc::channel();

            let thread = {
                let device = device.clone();
                let uploads = uploads.clone();
                let shutdown = shutdown.clone();
                std::thread::spawn(move || {
                    while !shutdown.load(Ordering::Relaxed) {
                        {
                            let mut dev = device.lock().unwrap();
                            let events: Vec<InputEvent> = match dev.fetch_events() {
                                Ok(events) => events.collect(),
                                Err(_) => Vec::new(),
                            };
                            for event in events {
                                match event.destructure() {
                                    EventSummary::UInput(ev, UInputCode::UI_FF_UPLOAD, _) => {
                                        let mut upload = dev.process_ff_upload(ev).unwrap();
                                        upload.set_retval(0);
                                        uploads.fetch_add(1, Ordering::Relaxed);
                                    }
                                    EventSummary::UInput(ev, UInputCode::UI_FF_ERASE, _) => {
                                        let mut erase = dev.process_ff_erase(ev).unwrap();
                                        erase.set_retval(0);
                                    }
                                    EventSummary::ForceFeedback(_, code, value) => {
                                        let _ = play_tx.send((code.0, value));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                })
            };

            FakeSteamPad {
                device,
                event_node,
                uploads,
                plays,
                shutdown,
                thread: Some(thread),
            }
        }

        fn emit(&self, events: &[InputEvent]) {
            self.device.lock().unwrap().emit(events).unwrap();
        }
    }

    impl Drop for FakeSteamPad {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn spawn_router(slot: u32, source_path: &str) -> (Vec<String>, Arc<AtomicBool>, JoinHandle<()>) {
        let mut proxy = build_proxy_pad(slot).unwrap();
        let dev_nodes = wait_for_dev_nodes(&mut proxy).unwrap();
        let shutdown = Arc::new(AtomicBool::new(false));
        let router = Router::new(proxy, slot, source_path, shutdown.clone());
        let thread = std::thread::spawn(move || router.run());
        (dev_nodes, shutdown, thread)
    }

    fn open_game_side(dev_nodes: &[String]) -> Device {
        let node = dev_nodes
            .iter()
            .find(|n| n.starts_with("/dev/input/event"))
            .unwrap();
        let dev = Device::open(node).unwrap();
        dev.set_nonblocking(true).unwrap();
        dev
    }

    fn wait_for_key(game: &mut Device, code: KeyCode, value: i32, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Ok(events) = game.fetch_events() {
                for event in events {
                    if event.event_type() == EventType::KEY
                        && event.code() == code.0
                        && event.value() == value
                    {
                        return true;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    fn press(pad: &FakeSteamPad, code: KeyCode, value: i32) {
        pad.emit(&[InputEvent::new(EventType::KEY.0, code.0, value)]);
    }

    const RUMBLE: FFEffectData = FFEffectData {
        direction: 0,
        trigger: FFTrigger { button: 0, interval: 0 },
        replay: FFReplay { length: 1000, delay: 0 },
        kind: FFEffectKind::Rumble {
            strong_magnitude: 0x4000,
            weak_magnitude: 0x2000,
        },
    };

    #[test]
    #[ignore]
    fn round_trip_events() {
        let pad = FakeSteamPad::new(40);
        let (nodes, shutdown, thread) = spawn_router(40, &pad.event_node);
        let mut game = open_game_side(&nodes);
        assert_eq!(game.physical_path(), Some("partydeck-proxy/40"));

        press(&pad, KeyCode::BTN_SOUTH, 1);
        assert!(wait_for_key(&mut game, KeyCode::BTN_SOUTH, 1, Duration::from_secs(2)));
        press(&pad, KeyCode::BTN_SOUTH, 0);
        assert!(wait_for_key(&mut game, KeyCode::BTN_SOUTH, 0, Duration::from_secs(2)));

        shutdown.store(true, Ordering::Relaxed);
        thread.join().unwrap();
    }

    #[test]
    #[ignore]
    fn churn_neutral_frame_then_reattach() {
        let pad = FakeSteamPad::new(41);
        let (nodes, shutdown, thread) = spawn_router(41, &pad.event_node);
        let mut game = open_game_side(&nodes);

        press(&pad, KeyCode::BTN_SOUTH, 1);
        assert!(wait_for_key(&mut game, KeyCode::BTN_SOUTH, 1, Duration::from_secs(2)));

        drop(pad);
        assert!(
            wait_for_key(&mut game, KeyCode::BTN_SOUTH, 0, Duration::from_secs(2)),
            "neutral frame not seen after source loss"
        );

        let pad2 = FakeSteamPad::new(41);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut seen = false;
        while Instant::now() < deadline && !seen {
            press(&pad2, KeyCode::BTN_EAST, 1);
            seen = wait_for_key(&mut game, KeyCode::BTN_EAST, 1, Duration::from_millis(300));
            if !seen {
                press(&pad2, KeyCode::BTN_EAST, 0);
            }
        }
        assert!(seen, "router did not re-attach to recreated pad");

        shutdown.store(true, Ordering::Relaxed);
        thread.join().unwrap();
    }

    #[test]
    #[ignore]
    fn ff_passthrough_and_reupload_after_churn() {
        let pad = FakeSteamPad::new(42);
        let (nodes, shutdown, thread) = spawn_router(42, &pad.event_node);
        let mut game = open_game_side(&nodes);

        let mut effect = game.upload_ff_effect(RUMBLE).unwrap();
        assert_eq!(pad.uploads.load(Ordering::Relaxed), 1);

        effect.play(1).unwrap();
        let (_, value) = pad.plays.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(value, 1);

        drop(pad);
        let pad2 = FakeSteamPad::new(42);

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && pad2.uploads.load(Ordering::Relaxed) == 0 {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(pad2.uploads.load(Ordering::Relaxed), 1, "effect not re-uploaded after churn");

        effect.play(1).unwrap();
        let (_, value) = pad2.plays.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(value, 1);

        drop(effect);
        shutdown.store(true, Ordering::Relaxed);
        thread.join().unwrap();
    }
}
