use std::ffi::CString;
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::time::{Duration, Instant};

use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::*;

use crate::input::{PROXY_PHYS_PREFIX, STEAM_INPUT_VENDOR, steam_pad_name};

pub(super) const PROXY_KEYS: [KeyCode; 11] = [
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

// (code, min, max, fuzz, flat): byte-exact copy of a real Steam Input pad's
// abs setup, taken from an on-device ioctl dump.
pub(super) const PROXY_ABS: [(AbsoluteAxisCode, i32, i32, i32, i32); 8] = [
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
pub(super) const PROXY_FF: [FFEffectCode; 1] = [FFEffectCode::FF_RUMBLE];

pub(super) const PROXY_FF_EFFECTS_MAX: u32 = 16;

pub(super) const STEAM_PAD_PRODUCT: u16 = 0x11ff;

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

pub(super) fn set_nonblocking(device: &VirtualDevice) -> io::Result<()> {
    unsafe {
        let fd = device.as_raw_fd();
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

pub(super) fn build_proxy_pad(slot: u32) -> io::Result<VirtualDevice> {
    let name = steam_pad_name(slot);
    let phys = CString::new(format!("{PROXY_PHYS_PREFIX}/{slot}"))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    let mut keys = AttributeSet::<KeyCode>::new();
    for key in PROXY_KEYS {
        keys.insert(key);
    }
    let mut ff = AttributeSet::<FFEffectCode>::new();
    for code in PROXY_FF {
        ff.insert(code);
    }

    let mut builder = VirtualDevice::builder()?.name(&name).input_id(InputId::new(
        BusType::BUS_USB,
        STEAM_INPUT_VENDOR,
        STEAM_PAD_PRODUCT,
        0x0001,
    ));
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
    set_nonblocking(&device)?;
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
pub(super) fn wait_for_dev_nodes(proxy: &mut VirtualDevice) -> io::Result<Vec<String>> {
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

pub(super) fn neutral_frame() -> Vec<InputEvent> {
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
pub(super) fn split_frames(
    pending: &mut Vec<InputEvent>,
    new_events: &[InputEvent],
) -> Vec<Vec<InputEvent>> {
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
