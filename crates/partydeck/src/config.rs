use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::paths::settings_file;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default, ValueEnum)]
pub enum PadFilterType {
    All,
    #[default]
    NoSteamInput,
    OnlySteamInput,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct PartyConfig {
    pub gamescope_fix_lowres: bool,
    pub layout_preset: String,
    /// Split-line style between screens: "off" | "faint" | "medium" | "strong".
    pub border_style: String,
    pub gamescope_force_grab_cursor: bool,
    pub kbm_support: bool,
    pub proton_version: String,
    pub proton_separate_pfxs: bool,
    pub proton_wow64: bool,
    pub pad_filter_type: PadFilterType,
    pub proxy_gamepads: bool,
    pub allow_multiple_instances_on_same_device: bool,
    pub profile_unique_dirs: bool,
    pub disable_mount_gamedirs: bool,
    pub check_for_updates: bool,
    pub debug_game_logs: bool,
}

impl Default for PartyConfig {
    fn default() -> Self {
        PartyConfig {
            gamescope_fix_lowres: true,
            layout_preset: "auto".to_string(),
            border_style: "faint".to_string(),
            gamescope_force_grab_cursor: false,
            kbm_support: true,
            proton_version: String::new(),
            proton_separate_pfxs: true,
            proton_wow64: true,
            pad_filter_type: PadFilterType::NoSteamInput,
            proxy_gamepads: true,
            allow_multiple_instances_on_same_device: false,
            profile_unique_dirs: true,
            disable_mount_gamedirs: false,
            check_for_updates: true,
            debug_game_logs: false,
        }
    }
}

pub fn load_cfg() -> PartyConfig {
    load_cfg_from(&settings_file())
}

pub fn save_cfg(config: &PartyConfig) -> Result<()> {
    save_cfg_to(&settings_file(), config)
}

/// Falls back to defaults when the file is missing or unreadable.
pub fn load_cfg_from(path: &Path) -> PartyConfig {
    File::open(path)
        .ok()
        .and_then(|file| serde_json::from_reader(BufReader::new(file)).ok())
        .unwrap_or_default()
}

pub fn save_cfg_to(path: &Path, config: &PartyConfig) -> Result<()> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_file(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("partydeck-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn missing_file_yields_defaults() {
        let cfg = load_cfg_from(&scratch_file("does-not-exist.json"));
        assert_eq!(cfg, PartyConfig::default());
    }

    #[test]
    fn partial_json_fills_missing_fields_with_defaults() {
        let cfg: PartyConfig =
            serde_json::from_str(r#"{"proton_version":"GE-Proton9","kbm_support":false}"#).unwrap();
        assert_eq!(cfg.proton_version, "GE-Proton9");
        assert!(!cfg.kbm_support);
        assert!(cfg.gamescope_fix_lowres);
        assert_eq!(cfg.layout_preset, "auto");
        assert_eq!(cfg.pad_filter_type, PadFilterType::NoSteamInput);
    }

    #[test]
    fn round_trip_through_file() {
        let path = scratch_file("round-trip.json");
        let cfg = PartyConfig {
            layout_preset: "grid".to_string(),
            pad_filter_type: PadFilterType::OnlySteamInput,
            debug_game_logs: true,
            ..PartyConfig::default()
        };
        save_cfg_to(&path, &cfg).unwrap();
        assert_eq!(load_cfg_from(&path), cfg);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pad_filter_clap_names() {
        assert_eq!(
            PadFilterType::from_str("no-steam-input", false),
            Ok(PadFilterType::NoSteamInput)
        );
        assert_eq!(
            PadFilterType::from_str("only-steam-input", false),
            Ok(PadFilterType::OnlySteamInput)
        );
    }
}
