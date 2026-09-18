use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::PartyConfig;
use crate::handler::Handler;
use crate::paths::prefixes_dir;

const DEFAULT_PROTON: &str = "GE-Proton";

/// Renders a host path the way the target OS expects it: unchanged for Linux,
/// `Z:\...` for Windows games running under Proton.
pub trait OsFmt {
    fn os_fmt(&self, win: bool) -> String;
}

impl<P: AsRef<Path>> OsFmt for P {
    fn os_fmt(&self, win: bool) -> String {
        let path = self.as_ref().to_string_lossy();
        if win {
            format!("Z:{}", path.replace('/', "\\"))
        } else {
            path.to_string()
        }
    }
}

/// Instances get their own prefix unless the config shares prefix "1".
pub fn prefix_dir(cfg: &PartyConfig, instance_index: usize) -> PathBuf {
    let n = if cfg.proton_separate_pfxs {
        instance_index + 1
    } else {
        1
    };
    prefixes_dir().join(n.to_string())
}

pub fn apply_env(
    cmd: &mut Command,
    cfg: &PartyConfig,
    h: &Handler,
    prefix: &Path,
    log_dir: Option<PathBuf>,
) {
    let protonpath = if cfg.proton_version.is_empty() {
        DEFAULT_PROTON
    } else {
        &cfg.proton_version
    };
    cmd.env("WINEPREFIX", prefix);
    cmd.env("PROTON_VERB", "run");
    cmd.env("PROTONPATH", protonpath);
    if h.enable_hidraw {
        cmd.env("PROTON_ENABLE_HIDRAW", "1");
    } else {
        cmd.env("PROTON_DISABLE_HIDRAW", "1");
    }
    if cfg.proton_wow64 {
        cmd.env("PROTON_USE_WOW64", "1");
    }
    if !cfg.debug_game_logs {
        return;
    }
    // Instances share an appid, so Proton's log filename would collide in the
    // default PROTON_LOG_DIR (the remapped per-profile HOME); use one dir each.
    let Some(log_dir) = log_dir else {
        return;
    };
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!(
            "[partydeck] Failed to create proton log dir {}: {e}",
            log_dir.display()
        );
        return;
    }
    cmd.env("PROTON_LOG", "1");
    cmd.env("PROTON_LOG_DIR", &log_dir);
    cmd.env("UMU_LOG", "debug");
}

pub fn erase_prefixes() -> io::Result<()> {
    let path = prefixes_dir();
    if path.exists() {
        std::fs::remove_dir_all(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_fmt_is_identity_on_linux() {
        assert_eq!("/games/g/dir".os_fmt(false), "/games/g/dir");
        assert_eq!(PathBuf::from("/games/g").os_fmt(false), "/games/g");
        assert_eq!(String::from("rel/path").os_fmt(false), "rel/path");
    }

    #[test]
    fn os_fmt_maps_to_z_drive_on_windows() {
        assert_eq!("/games/g/dir".os_fmt(true), "Z:\\games\\g\\dir");
        assert_eq!(PathBuf::from("/games/g").os_fmt(true), "Z:\\games\\g");
        assert_eq!(String::new().os_fmt(true), "Z:");
    }

    #[test]
    fn prefix_dir_numbering() {
        let separate = PartyConfig::default();
        let shared = PartyConfig {
            proton_separate_pfxs: false,
            ..PartyConfig::default()
        };
        assert!(prefix_dir(&separate, 0).ends_with("prefixes/1"));
        assert!(prefix_dir(&separate, 2).ends_with("prefixes/3"));
        assert!(prefix_dir(&shared, 2).ends_with("prefixes/1"));
    }
}
