use serde::Serialize;

use crate::error::Result;
use crate::handler::Handler;
use crate::input::{DeviceType, InputDevice};

#[derive(Serialize)]
pub struct ProfileDto {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
}

#[derive(Serialize)]
pub struct HandlerDto {
    pub name: String,
    pub author: String,
    pub version: String,
    pub win: bool,
    pub steam_appid: Option<u32>,
}

impl From<Handler> for HandlerDto {
    fn from(h: Handler) -> Self {
        HandlerDto {
            win: h.win(),
            name: h.name,
            author: h.author,
            version: h.version,
            steam_appid: h.steam_appid,
        }
    }
}

#[derive(Serialize)]
pub struct DeviceDto {
    /// Path, not list index: scan order is not stable across hotplugs.
    pub path: String,
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: &'static str,
    /// For Steam Input virtual pads: the XInput slot (== Steam Input's
    /// nXInputIndex), so the plugin can map a lobby controller to this path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xinput_slot: Option<u32>,
}

impl From<&InputDevice> for DeviceDto {
    fn from(d: &InputDevice) -> Self {
        DeviceDto {
            path: d.path().to_string(),
            name: d.fancyname().to_string(),
            device_type: device_type_str(d.device_type()),
            xinput_slot: d.xinput_slot(),
        }
    }
}

fn device_type_str(t: DeviceType) -> &'static str {
    match t {
        DeviceType::Gamepad => "gamepad",
        DeviceType::Keyboard => "keyboard",
        DeviceType::Mouse => "mouse",
        DeviceType::Other => "other",
    }
}

/// Pretty JSON on stdout; the only thing the CLI prints there.
pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| format!("failed to serialize output: {e}"))?;
    println!("{text}");
    Ok(())
}
