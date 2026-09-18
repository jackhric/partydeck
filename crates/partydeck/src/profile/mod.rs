mod avatar;
mod gamesave;

pub use avatar::{
    clear_avatar, list_builtin_avatars, read_avatar_base64, set_avatar_builtin, set_avatar_custom,
};
pub use gamesave::create_profile_gamesave;

use std::io;
use std::path::PathBuf;

use crate::error::Result;
use crate::instance::GUEST_LABEL;
use crate::paths::profiles_dir;

pub static GUEST_NAMES: [&str; 33] = [
    "Blinky", "Pinky", "Inky", "Clyde", "Beatrice", "Battler", "Miyao", "Rena", "Ellie", "Joel",
    "Leon", "Ada", "Madeline", "Theo", "Yokatta", "Wyrm", "Brodiee", "Supreme", "Conk", "Gort",
    "Lich", "Smores", "Canary", "Trico", "Yorda", "Wander", "Agro", "Jak", "Daxter", "Soap",
    "Ghost", "Tomi", "Masaki",
];

/// Guest profile directories carry a leading '.' and are deleted on the next start.
pub fn is_guest_name(name: &str) -> bool {
    name.starts_with('.')
}

pub fn guest_dir_name(guest: &str) -> String {
    format!(".{guest}")
}

pub fn profile_dir(name: &str) -> PathBuf {
    profiles_dir().join(name)
}

/// Accepts any name that is safe as a directory component, guest names included.
pub fn is_safe_profile_name(name: &str) -> bool {
    !(name.is_empty()
        || name == GUEST_LABEL
        || name.contains('/')
        || name.contains('\\')
        || name.contains(".."))
}

/// Validation for user-created profiles: safe as a directory and not a guest name.
pub fn validate_profile_name(name: &str) -> std::result::Result<(), String> {
    if !is_safe_profile_name(name) {
        return Err(format!("invalid profile name: {name}"));
    }
    if is_guest_name(name) {
        return Err(format!(
            "invalid profile name {name:?}: leading '.' is reserved for guest profiles"
        ));
    }
    Ok(())
}

pub fn create_profile(name: &str) -> io::Result<()> {
    validate_profile_name(name).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    create_profile_dir(name)
}

pub fn create_guest_profile(dir_name: &str) -> io::Result<()> {
    if !is_safe_profile_name(dir_name) || !is_guest_name(dir_name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid guest profile name: {dir_name}"),
        ));
    }
    create_profile_dir(dir_name)
}

// Lays out the per-profile HOME, Windows user data and Goldberg settings.
fn create_profile_dir(name: &str) -> io::Result<()> {
    let path_profile = profile_dir(name);
    if path_profile.exists() {
        return Ok(());
    }
    eprintln!("[partydeck] Creating profile {name}");

    let path_steam = path_profile.join("steam/settings");
    for sub in [
        "windata/AppData/Local/Temp",
        "windata/AppData/LocalLow",
        "windata/AppData/Roaming",
        "windata/Documents",
        "windata/Saved Games",
        "windata/Desktop",
        "home/.local/share",
        "home/.config",
    ] {
        std::fs::create_dir_all(path_profile.join(sub))?;
    }
    std::fs::create_dir_all(&path_steam)?;

    let usersettings = format!("[user::general]\naccount_name={name}");
    std::fs::write(path_steam.join("configs.user.ini"), usersettings)?;
    Ok(())
}

pub fn delete_profile(name: &str) -> Result<()> {
    if !is_safe_profile_name(name) {
        return Err(format!("invalid profile name: {name}").into());
    }
    let path = profile_dir(name);
    if !path.is_dir() {
        return Err(format!("profile not found: {name}").into());
    }
    eprintln!("[partydeck] Deleting profile {name}");
    std::fs::remove_dir_all(&path)?;
    Ok(())
}

/// Sorted profile directory names. `include_guest` prepends the GUI's guest entry.
pub fn scan_profiles(include_guest: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(profiles_dir()) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|ft| ft.is_dir())
                && let Some(name) = entry.file_name().to_str()
            {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    if include_guest {
        out.insert(0, GUEST_LABEL.to_string());
    }
    out
}

pub fn remove_guest_profiles() -> Result<()> {
    for entry in std::fs::read_dir(profiles_dir())?.flatten() {
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if is_guest_name(&entry.file_name().to_string_lossy()) {
            std::fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_names_reject_path_tricks_and_guest_label() {
        assert!(is_safe_profile_name("Alice"));
        assert!(is_safe_profile_name(".Blinky"));
        assert!(!is_safe_profile_name(""));
        assert!(!is_safe_profile_name("Guest"));
        assert!(!is_safe_profile_name("a/b"));
        assert!(!is_safe_profile_name("a\\b"));
        assert!(!is_safe_profile_name(".."));
        assert!(!is_safe_profile_name("..hidden"));
    }

    #[test]
    fn user_profiles_cannot_use_guest_prefix() {
        assert!(validate_profile_name("Alice").is_ok());
        assert!(validate_profile_name(".Alice").is_err());
        assert!(validate_profile_name("..").is_err());
        assert!(validate_profile_name("Guest").is_err());
    }

    #[test]
    fn guest_dir_names_round_trip() {
        let dir = guest_dir_name("Blinky");
        assert_eq!(dir, ".Blinky");
        assert!(is_guest_name(&dir));
        assert!(!is_guest_name("Blinky"));
        assert!(
            GUEST_NAMES
                .iter()
                .all(|g| is_safe_profile_name(&guest_dir_name(g)))
        );
    }
}
