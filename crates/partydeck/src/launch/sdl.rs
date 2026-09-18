use std::path::PathBuf;
use std::process::Command;

use crate::config::{PadFilterType, PartyConfig};
use crate::handler::{Handler, SDL2Override};
use crate::steam::PATH_STEAM;

/// VID/PID pairs SDL must not treat as controllers when only Steam Input
/// pads should be visible; otherwise games see each physical pad twice.
const SDL_GAMECONTROLLER_IGNORE_DEVICES: &str = include_str!("sdl_ignore_devices.txt");

pub fn apply_env(cmd: &mut Command, h: &Handler, cfg: &PartyConfig) {
    if !h.win() || !h.enable_hidraw {
        cmd.env("SDL_JOYSTICK_HIDAPI", "0");
    }
    match h.sdl2_override {
        SDL2Override::No => {}
        SDL2Override::Srt => {
            cmd.env(
                "SDL_DYNAMIC_API",
                PATH_STEAM.join("bin32/steam-runtime/usr/lib/i386-linux-gnu/libSDL2-2.0.so.0"),
            );
        }
        SDL2Override::Sys => {
            cmd.env("SDL_DYNAMIC_API", PathBuf::from("/usr/lib/libSDL2.so"));
        }
    }
    if cfg.pad_filter_type != PadFilterType::NoSteamInput {
        cmd.env("SDL_GAMECONTROLLER_ALLOW_STEAM_VIRTUAL_GAMEPAD", "1");
    }
    if cfg.pad_filter_type == PadFilterType::OnlySteamInput {
        cmd.env(
            "SDL_GAMECONTROLLER_IGNORE_DEVICES",
            SDL_GAMECONTROLLER_IGNORE_DEVICES.trim(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_list_is_well_formed() {
        let list = SDL_GAMECONTROLLER_IGNORE_DEVICES.trim();
        assert!(list.starts_with("0x054c/0x0df2,"));
        assert!(list.split(',').filter(|s| !s.is_empty()).all(|pair| {
            let (vid, pid) = pair.split_once('/').unwrap();
            vid.starts_with("0x") && pid.starts_with("0x") && vid.len() == 6 && pid.len() == 6
        }));
    }
}
