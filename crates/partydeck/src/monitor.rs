use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;

use crate::error::Result;

const FALLBACK_WIDTH: u32 = 1920;
const FALLBACK_HEIGHT: u32 = 1080;

#[derive(Clone, Debug)]
pub struct Monitor {
    name: String,
    width: u32,
    height: u32,
    size_overridden: bool,
}

impl Monitor {
    #[cfg(test)]
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        Self {
            name: name.to_string(),
            width,
            height,
            size_overridden: false,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// True when PARTYDECK_SCREEN_WIDTH/HEIGHT replaced the detected size.
    pub fn size_overridden(&self) -> bool {
        self.size_overridden
    }
}

// Mimics the SDL X11 display enumeration gamescope relies on, without SDL.
// SDL_HINT_VIDEO_DISPLAY_PRIORITY and outputs without visual info are ignored.
// https://github.com/libsdl-org/SDL/blob/225fb12ae13b70689bcb8c0b42bf061120fefcc4/src/video/x11/SDL_x11modes.c#L868
fn detect_monitors_x11() -> Result<Vec<Monitor>> {
    let (con, screen_num) = x11rb::connect(None)?;
    let screen = &con.setup().roots[screen_num];

    let primary = con.randr_get_output_primary(screen.root)?.reply()?.output;
    let res = con.randr_get_screen_resources(screen.root)?.reply()?;

    let mut monitors = Vec::new();
    for output in &res.outputs {
        let info = con
            .randr_get_output_info(*output, res.config_timestamp)?
            .reply()?;
        if info.connection != x11rb::protocol::randr::Connection::CONNECTED || info.crtc == 0 {
            continue;
        }
        let crtc = con
            .randr_get_crtc_info(info.crtc, res.config_timestamp)?
            .reply()?;

        let monitor = Monitor {
            name: String::from_utf8_lossy(&info.name).to_string(),
            width: crtc.width.into(),
            height: crtc.height.into(),
            size_overridden: false,
        };
        // SDL sorts the primary output first.
        if *output == primary {
            monitors.insert(0, monitor);
        } else {
            monitors.push(monitor);
        }
    }
    Ok(monitors)
}

pub fn get_x11_dpi_scale() -> f32 {
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

    let Ok((conn, screen_num)) = x11rb::connect(None) else {
        return 1.0;
    };
    let root = conn.setup().roots[screen_num].root;
    let Ok(cookie) = conn.get_property(
        false,
        root,
        AtomEnum::RESOURCE_MANAGER,
        AtomEnum::STRING,
        0,
        65536,
    ) else {
        return 1.0;
    };
    let Ok(reply) = cookie.reply() else {
        return 1.0;
    };

    String::from_utf8_lossy(&reply.value)
        .lines()
        .filter_map(|line| line.strip_prefix("Xft.dpi:"))
        .filter_map(|rest| rest.trim().parse::<f32>().ok())
        .find(|dpi| *dpi > 0.0)
        .map_or(1.0, |dpi| dpi / 96.0)
}

/// Never empty: falls back to an assumed 1080p monitor when X11 reports none.
/// PARTYDECK_SCREEN_WIDTH/HEIGHT override the primary monitor's size.
pub fn detect_monitors() -> Vec<Monitor> {
    let mut monitors = detect_monitors_x11().unwrap_or_default();

    if monitors.is_empty() {
        eprintln!(
            "[partydeck] Failed to get monitors; using assumed {FALLBACK_WIDTH}x{FALLBACK_HEIGHT}"
        );
        monitors.push(Monitor {
            name: "Partydeck Virtual Monitor".to_string(),
            width: FALLBACK_WIDTH,
            height: FALLBACK_HEIGHT,
            size_overridden: false,
        });
    }

    if let Some((width, height)) = size_override() {
        monitors[0].width = width;
        monitors[0].height = height;
        monitors[0].size_overridden = true;
    }
    monitors
}

fn size_override() -> Option<(u32, u32)> {
    let width = std::env::var("PARTYDECK_SCREEN_WIDTH").ok()?.parse().ok()?;
    let height = std::env::var("PARTYDECK_SCREEN_HEIGHT")
        .ok()?
        .parse()
        .ok()?;
    Some((width, height))
}
