mod nav;
pub mod proxy;

pub use nav::PadButton;

use evdev::{Device, KeyCode};

use crate::config::PadFilterType;

/// USB vendor id Steam Input gives its virtual pads.
pub const STEAM_INPUT_VENDOR: u16 = 0x28de;
/// Steam Input names its uinput pads "Microsoft X-Box 360 pad N" (N = nXInputIndex).
pub const STEAM_PAD_NAME_PREFIX: &str = "Microsoft X-Box 360 pad ";
/// Phys prefix of the proxy pads partydeck itself creates, so scans skip them.
pub const PROXY_PHYS_PREFIX: &str = "partydeck-proxy";

pub fn steam_pad_name(slot: u32) -> String {
    format!("{STEAM_PAD_NAME_PREFIX}{slot}")
}

pub fn is_proxy_phys(phys: Option<&str>) -> bool {
    phys.is_some_and(|p| p.starts_with(PROXY_PHYS_PREFIX))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviceType {
    Gamepad,
    Keyboard,
    Mouse,
    Other,
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub path: String,
    /// Sibling /dev/hidraw* nodes, resolved at scan time so the sandbox can mask
    /// them alongside the evdev node when hidraw is exposed to wine.
    pub hidraw_paths: Vec<String>,
    pub enabled: bool,
    pub device_type: DeviceType,
    /// XInput slot of a Steam Input virtual pad. Their evdev Uniq is empty, so
    /// the name-derived slot is the only stable per-pad key and it matches Steam
    /// Input's nXInputIndex, which lets the plugin map a lobby controller to a path.
    pub xinput_slot: Option<u32>,
}

pub struct InputDevice {
    path: String,
    dev: Device,
    enabled: bool,
    device_type: DeviceType,
    has_button_held: bool,
}

impl InputDevice {
    pub fn name(&self) -> &str {
        self.dev.name().unwrap_or_default()
    }

    pub fn emoji(&self) -> &str {
        match self.device_type() {
            DeviceType::Gamepad => "🎮",
            DeviceType::Keyboard => "🖮",
            DeviceType::Mouse => "🖱",
            DeviceType::Other => "",
        }
    }

    pub fn fancyname(&self) -> &str {
        match self.dev.input_id().vendor() {
            0x045e => "Xbox Controller",
            0x054c => "PS Controller",
            0x057e => "NT Pro Controller",
            STEAM_INPUT_VENDOR => "Steam Input",
            _ => self.name(),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn device_type(&self) -> DeviceType {
        self.device_type
    }

    pub fn has_button_held(&self) -> bool {
        self.has_button_held
    }

    pub fn xinput_slot(&self) -> Option<u32> {
        if self.dev.input_id().vendor() != STEAM_INPUT_VENDOR {
            return None;
        }
        self.name()
            .strip_prefix(STEAM_PAD_NAME_PREFIX)
            .and_then(|n| n.trim().parse::<u32>().ok())
    }

    pub fn info(&self) -> DeviceInfo {
        DeviceInfo {
            path: self.path().to_string(),
            hidraw_paths: hidraw_siblings(self.path()),
            enabled: self.enabled(),
            device_type: self.device_type(),
            xinput_slot: self.xinput_slot(),
        }
    }
}

fn classify(dev: &Device) -> DeviceType {
    let has_key = |key: KeyCode| dev.supported_keys().is_some_and(|keys| keys.contains(key));
    if has_key(KeyCode::BTN_SOUTH) {
        DeviceType::Gamepad
    } else if has_key(KeyCode::BTN_LEFT) {
        DeviceType::Mouse
    } else if has_key(KeyCode::KEY_SPACE) {
        DeviceType::Keyboard
    } else {
        DeviceType::Other
    }
}

/// Every gamepad, keyboard and mouse evdev node, sorted by path. `filter` only
/// sets `enabled`; callers decide whether disabled devices are shown.
pub fn scan_input_devices(filter: &PadFilterType) -> Vec<InputDevice> {
    let mut pads: Vec<InputDevice> = Vec::new();
    for (path, dev) in evdev::enumerate() {
        if is_proxy_phys(dev.physical_path()) {
            continue;
        }
        let device_type = classify(&dev);
        if device_type == DeviceType::Other {
            continue;
        }
        let is_steam_input = dev.input_id().vendor() == STEAM_INPUT_VENDOR;
        let enabled = match filter {
            PadFilterType::All => true,
            PadFilterType::NoSteamInput => !is_steam_input,
            PadFilterType::OnlySteamInput => is_steam_input,
        };
        if dev.set_nonblocking(true).is_err() {
            eprintln!(
                "[partydeck] evdev: Failed to set non-blocking mode for {}",
                path.display()
            );
            continue;
        }
        pads.push(InputDevice {
            path: path.to_string_lossy().to_string(),
            dev,
            enabled,
            device_type,
            has_button_held: false,
        });
    }
    pads.sort_by(|a, b| a.path.cmp(&b.path));
    pads
}

// /sys/class/input/eventN -> .../HID/input/inputN/eventN, so two `device` hops
// land on the HID dir, which contains a `hidraw/` subdir for any hidraw siblings.
fn hidraw_siblings(evdev_path: &str) -> Vec<String> {
    let name = std::path::Path::new(evdev_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let dir = format!("/sys/class/input/{name}/device/device/hidraw");
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(|s| format!("/dev/{s}")))
        .collect()
}
