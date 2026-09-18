use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SteamRuntime {
    Scout,
    Soldier,
    Sniper,
    Steamrt4,
}

const SCOUT_RUN: &str = "bin32/steam-runtime/run.sh";
const V2_ENTRY_POINT: &str = "_v2-entry-point";

impl SteamRuntime {
    /// Handler runtime names; empty or unknown names mean "no runtime".
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "scout" => Some(SteamRuntime::Scout),
            "soldier" => Some(SteamRuntime::Soldier),
            "sniper" => Some(SteamRuntime::Sniper),
            "steamrt4" => Some(SteamRuntime::Steamrt4),
            _ => None,
        }
    }

    // Older sniper installs live in a folder named -arm64 even on x86_64.
    fn candidate_dirs(self, steam_dir: &Path) -> Vec<PathBuf> {
        let common = steam_dir.join("steam/steamapps/common");
        match self {
            SteamRuntime::Scout => vec![steam_dir.join("bin32/steam-runtime")],
            SteamRuntime::Soldier => vec![common.join("SteamLinuxRuntime_soldier")],
            SteamRuntime::Sniper => vec![
                common.join("SteamLinuxRuntime_sniper"),
                common.join("SteamLinuxRuntime_sniper-arm64"),
            ],
            SteamRuntime::Steamrt4 => vec![common.join("SteamLinuxRuntime_4")],
        }
    }

    /// The runtime's launcher script: the first installed candidate, or the
    /// primary location when none is installed.
    pub fn path(self, steam_dir: &Path) -> PathBuf {
        let dirs = self.candidate_dirs(steam_dir);
        let dir = dirs.iter().find(|d| d.exists()).unwrap_or(&dirs[0]);
        match self {
            SteamRuntime::Scout => steam_dir.join(SCOUT_RUN),
            _ => dir.join(V2_ENTRY_POINT),
        }
    }

    pub fn exists(self, steam_dir: &Path) -> bool {
        match self {
            SteamRuntime::Scout => steam_dir.join(SCOUT_RUN).exists(),
            _ => self.candidate_dirs(steam_dir).iter().any(|d| d.exists()),
        }
    }

    /// Arguments that wrap the game executable: `run.sh` for scout, the v2
    /// entry point followed by `--` for the container runtimes.
    pub fn command_prefix(self, steam_dir: &Path) -> Vec<OsString> {
        let mut args = vec![self.path(steam_dir).into_os_string()];
        if self != SteamRuntime::Scout {
            args.push("--".into());
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        assert_eq!(SteamRuntime::from_name("scout"), Some(SteamRuntime::Scout));
        assert_eq!(
            SteamRuntime::from_name("soldier"),
            Some(SteamRuntime::Soldier)
        );
        assert_eq!(
            SteamRuntime::from_name("sniper"),
            Some(SteamRuntime::Sniper)
        );
        assert_eq!(
            SteamRuntime::from_name("steamrt4"),
            Some(SteamRuntime::Steamrt4)
        );
        assert_eq!(SteamRuntime::from_name(""), None);
        assert_eq!(SteamRuntime::from_name("heavy"), None);
    }

    #[test]
    fn paths_relative_to_steam_dir() {
        let steam = Path::new("/nonexistent/.steam");
        assert_eq!(
            SteamRuntime::Scout.path(steam),
            Path::new("/nonexistent/.steam/bin32/steam-runtime/run.sh")
        );
        assert_eq!(
            SteamRuntime::Soldier.path(steam),
            Path::new(
                "/nonexistent/.steam/steam/steamapps/common/SteamLinuxRuntime_soldier/_v2-entry-point"
            )
        );
        assert_eq!(
            SteamRuntime::Sniper.path(steam),
            Path::new(
                "/nonexistent/.steam/steam/steamapps/common/SteamLinuxRuntime_sniper/_v2-entry-point"
            )
        );
        assert_eq!(
            SteamRuntime::Steamrt4.path(steam),
            Path::new(
                "/nonexistent/.steam/steam/steamapps/common/SteamLinuxRuntime_4/_v2-entry-point"
            )
        );
        assert!(!SteamRuntime::Sniper.exists(steam));
    }

    #[test]
    fn command_prefix_adds_separator_for_container_runtimes() {
        let steam = Path::new("/s");
        assert_eq!(SteamRuntime::Scout.command_prefix(steam).len(), 1);
        let soldier = SteamRuntime::Soldier.command_prefix(steam);
        assert_eq!(soldier.len(), 2);
        assert_eq!(soldier[1], "--");
    }

    #[test]
    fn sniper_falls_back_to_arm64_folder() {
        let steam = std::env::temp_dir().join(format!("partydeck-rt-{}", std::process::id()));
        let arm = steam.join("steam/steamapps/common/SteamLinuxRuntime_sniper-arm64");
        std::fs::create_dir_all(&arm).unwrap();
        assert!(SteamRuntime::Sniper.exists(&steam));
        assert_eq!(SteamRuntime::Sniper.path(&steam), arm.join(V2_ENTRY_POINT));
        assert!(!SteamRuntime::Soldier.exists(&steam));
        let _ = std::fs::remove_dir_all(steam);
    }
}
