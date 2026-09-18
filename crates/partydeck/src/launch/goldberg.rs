use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::Result;
use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::{PATH_RES, goldberg_data_dir};

/// Bind mounts that put the Goldberg emulator where the game loads steamclient.
pub struct GoldbergMounts {
    pub binds: Vec<(PathBuf, PathBuf)>,
}

pub fn mounts(steam_dir: &Path, win: bool, prefix: &Path) -> Result<GoldbergMounts> {
    let sdk32 = std::fs::read_link(steam_dir.join("sdk32"))
        .map_err(|e| format!("Failed to read sdk32 link: {e}"))?;
    let sdk64 = std::fs::read_link(steam_dir.join("sdk64"))
        .map_err(|e| format!("Failed to read sdk64 link: {e}"))?;

    let mut binds = vec![
        (PATH_RES.join("goldberg/linux32"), sdk32),
        (PATH_RES.join("goldberg/linux64"), sdk64),
    ];
    if win {
        binds.push((
            PATH_RES.join("goldberg/win"),
            prefix.join("drive_c/Program Files (x86)/Steam"),
        ));
    }
    Ok(GoldbergMounts { binds })
}

pub fn apply_env(cmd: &mut Command, h: &Handler, instance: &Instance, profile_dir: &Path) {
    cmd.env("GseAppPath", goldberg_data_dir());
    cmd.env("GseSavePath", profile_dir.join("steam"));
    cmd.env("SteamAppUser", &instance.profname);
    cmd.env("SteamUser", &instance.profname);
    cmd.env("SteamClientLaunch", "1");
    cmd.env("SteamEnv", "1");
    if let Some(appid) = h.steam_appid {
        cmd.env("SteamAppId", appid.to_string());
        cmd.env("SteamGameId", appid.to_string());
    }
}
