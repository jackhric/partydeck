use super::pad::*;
use super::*;
use evdev::uinput::VirtualDevice;
use evdev::*;

use crate::input::is_proxy_phys;

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
        &[
            key(KeyCode::BTN_SOUTH, 1),
            abs(AbsoluteAxisCode::ABS_X, 5),
            syn(),
        ],
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
    assert_eq!(
        codes(&frames[0]),
        vec![(EventType::KEY.0, KeyCode::BTN_EAST.0, 1)]
    );
    assert_eq!(
        codes(&frames[1]),
        vec![(EventType::KEY.0, KeyCode::BTN_SOUTH.0, 1)]
    );
    assert_eq!(
        codes(&pending),
        vec![(EventType::KEY.0, KeyCode::BTN_SOUTH.0, 0)]
    );
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
        assert!(
            frame
                .iter()
                .any(|e| e.event_type() == EventType::KEY && e.code() == k.0 && e.value() == 0)
        );
    }
    for (a, ..) in PROXY_ABS {
        assert!(
            frame.iter().any(|e| e.event_type() == EventType::ABSOLUTE
                && e.code() == a.0
                && e.value() == 0)
        );
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
    Instance::new(devices)
}

fn instance_with_slot(devices: Vec<usize>, slot: u32) -> Instance {
    Instance {
        pad_slot: Some(slot),
        ..instance(devices)
    }
}

#[test]
fn instance_pads_explicit_slot_with_device() {
    let devices = vec![device(DeviceType::Gamepad, true, Some(0))];
    let pads = instance_pads(&instance_with_slot(vec![0], 3), &devices);
    assert_eq!(pads, vec![(3, Some("/dev/input/event0"))]);
}

#[test]
fn instance_pads_explicit_slot_no_devices() {
    let pads = instance_pads(&instance_with_slot(vec![], 2), &[]);
    assert_eq!(pads, vec![(2, None)]);
}

#[test]
fn instance_pads_explicit_slot_ignores_non_gamepads() {
    let devices = vec![
        device(DeviceType::Gamepad, false, Some(0)),
        device(DeviceType::Keyboard, true, None),
    ];
    let pads = instance_pads(&instance_with_slot(vec![0, 1], 5), &devices);
    assert_eq!(pads, vec![(5, None)]);
}

#[test]
fn instance_pads_derived_from_devices() {
    let devices = vec![
        device(DeviceType::Gamepad, true, Some(1)),
        device(DeviceType::Gamepad, true, None),
        device(DeviceType::Gamepad, false, Some(2)),
        device(DeviceType::Keyboard, true, None),
    ];
    let pads = instance_pads(&instance(vec![0, 1, 2, 3]), &devices);
    assert_eq!(pads, vec![(1, Some("/dev/input/event0"))]);
}

#[test]
fn instance_pads_ignores_out_of_range_device_indices() {
    let devices = vec![device(DeviceType::Gamepad, true, Some(1))];
    let pads = instance_pads(&instance(vec![0, 7]), &devices);
    assert_eq!(pads, vec![(1, Some("/dev/input/event0"))]);
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
    assert!(!ProxySession::wanted(
        &cfg_off, &handler, &instances, &slotted
    ));
    assert!(!ProxySession::wanted(
        &cfg,
        &handler_hidraw,
        &instances,
        &slotted
    ));
    assert!(!ProxySession::wanted(
        &cfg, &handler, &instances, &unslotted
    ));
    assert!(ProxySession::wanted(
        &cfg,
        &handler,
        &instances,
        &disabled_unslotted
    ));
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

    let explicit_empty = vec![instance_with_slot(vec![], 0)];
    assert!(ProxySession::wanted(&cfg, &handler, &explicit_empty, &[]));
    assert!(!ProxySession::wanted(
        &cfg_off,
        &handler,
        &explicit_empty,
        &[]
    ));
    assert!(!ProxySession::wanted(
        &cfg,
        &handler_hidraw,
        &explicit_empty,
        &[]
    ));
    // Explicit slot overrides an unslotted device that would otherwise deny.
    let explicit_unslotted = vec![instance_with_slot(vec![0], 0)];
    assert!(ProxySession::wanted(
        &cfg,
        &handler,
        &explicit_unslotted,
        &unslotted
    ));
}

mod uinput_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Mutex, mpsc};
    use std::time::{Duration, Instant};

    use crate::input::steam_pad_name;

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
            let name = steam_pad_name(slot);
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
                .input_id(InputId::new(
                    BusType::BUS_USB,
                    crate::input::STEAM_INPUT_VENDOR,
                    STEAM_PAD_PRODUCT,
                    0x0001,
                ))
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
            set_nonblocking(&device).unwrap();
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

    fn spawn_router(
        slot: u32,
        source_path: Option<&str>,
    ) -> (Vec<String>, Arc<AtomicBool>, JoinHandle<()>) {
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
        trigger: FFTrigger {
            button: 0,
            interval: 0,
        },
        replay: FFReplay {
            length: 1000,
            delay: 0,
        },
        kind: FFEffectKind::Rumble {
            strong_magnitude: 0x4000,
            weak_magnitude: 0x2000,
        },
    };

    #[test]
    #[ignore]
    fn round_trip_events() {
        let pad = FakeSteamPad::new(40);
        let (nodes, shutdown, thread) = spawn_router(40, Some(&pad.event_node));
        let mut game = open_game_side(&nodes);
        assert_eq!(game.physical_path(), Some("partydeck-proxy/40"));

        press(&pad, KeyCode::BTN_SOUTH, 1);
        assert!(wait_for_key(
            &mut game,
            KeyCode::BTN_SOUTH,
            1,
            Duration::from_secs(2)
        ));
        press(&pad, KeyCode::BTN_SOUTH, 0);
        assert!(wait_for_key(
            &mut game,
            KeyCode::BTN_SOUTH,
            0,
            Duration::from_secs(2)
        ));

        shutdown.store(true, Ordering::Relaxed);
        thread.join().unwrap();
    }

    #[test]
    #[ignore]
    fn churn_neutral_frame_then_reattach() {
        let pad = FakeSteamPad::new(41);
        let (nodes, shutdown, thread) = spawn_router(41, Some(&pad.event_node));
        let mut game = open_game_side(&nodes);

        press(&pad, KeyCode::BTN_SOUTH, 1);
        assert!(wait_for_key(
            &mut game,
            KeyCode::BTN_SOUTH,
            1,
            Duration::from_secs(2)
        ));

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
    fn detached_start_then_attach() {
        let (nodes, shutdown, thread) = spawn_router(43, None);
        let mut game = open_game_side(&nodes);

        let pad = FakeSteamPad::new(43);
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut seen = false;
        while Instant::now() < deadline && !seen {
            press(&pad, KeyCode::BTN_SOUTH, 1);
            seen = wait_for_key(&mut game, KeyCode::BTN_SOUTH, 1, Duration::from_millis(300));
            if !seen {
                press(&pad, KeyCode::BTN_SOUTH, 0);
            }
        }
        assert!(seen, "router did not attach to late-created pad");

        shutdown.store(true, Ordering::Relaxed);
        thread.join().unwrap();
    }

    #[test]
    #[ignore]
    fn ff_passthrough_and_reupload_after_churn() {
        let pad = FakeSteamPad::new(42);
        let (nodes, shutdown, thread) = spawn_router(42, Some(&pad.event_node));
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
        assert_eq!(
            pad2.uploads.load(Ordering::Relaxed),
            1,
            "effect not re-uploaded after churn"
        );

        effect.play(1).unwrap();
        let (_, value) = pad2.plays.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(value, 1);

        drop(effect);
        shutdown.store(true, Ordering::Relaxed);
        thread.join().unwrap();
    }
}
