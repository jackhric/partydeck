use std::error::Error;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Serialize;

use crate::{handler::Handler, paths::*, util::copy_dir_recursive};

fn valid_profile_name(name: &str) -> bool {
    !(name.is_empty()
        || name == "Guest"
        || name.contains('/')
        || name.contains('\\')
        || name.contains(".."))
}

// Makes a folder and sets up Goldberg Steam Emu profile for Steam games
pub fn create_profile(name: &str) -> Result<(), std::io::Error> {
    // Leading '.' is allowed here (guest profiles); the CLI rejects it separately.
    if !valid_profile_name(name) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid profile name: {name}"),
        ));
    }
    if PATH_PARTY.join(format!("profiles/{name}")).exists() {
        return Ok(());
    }

    println!("[partydeck] Creating profile {name}");
    let path_profile = PATH_PARTY.join(format!("profiles/{name}"));
    let path_steam = path_profile.join("steam/settings");

    std::fs::create_dir_all(path_profile.join("windata/AppData/Local/Temp"))?;
    std::fs::create_dir_all(path_profile.join("windata/AppData/LocalLow"))?;
    std::fs::create_dir_all(path_profile.join("windata/AppData/Roaming"))?;
    std::fs::create_dir_all(path_profile.join("windata/Documents"))?;
    std::fs::create_dir_all(path_profile.join("windata/Saved Games"))?;
    std::fs::create_dir_all(path_profile.join("windata/Desktop"))?;
    std::fs::create_dir_all(path_profile.join("home/.local/share"))?;
    std::fs::create_dir_all(path_profile.join("home/.config"))?;
    std::fs::create_dir_all(path_steam.clone())?;

    let usersettings = format!("[user::general]\naccount_name={name}");
    std::fs::write(path_steam.join("configs.user.ini"), usersettings)?;

    println!("[partydeck] Profile created successfully");
    Ok(())
}

pub fn delete_profile(name: &str) -> Result<(), Box<dyn Error>> {
    if !valid_profile_name(name) {
        return Err(format!("invalid profile name: {name}").into());
    }
    let path = PATH_PARTY.join("profiles").join(name);
    if !path.is_dir() {
        return Err(format!("profile not found: {name}").into());
    }
    println!("[partydeck] Deleting profile {name}");
    std::fs::remove_dir_all(&path)?;
    Ok(())
}

// Creates the "game save" folder for per-profile game data to go into
pub fn create_profile_gamesave(name: &str, h: &Handler) -> Result<(), Box<dyn Error>> {
    let uid = h.handler_dir_name();
    let path_prof = PATH_PARTY.join("profiles").join(name);
    let path_gamesave = path_prof.join("gamesaves").join(&uid);
    let path_home = path_prof.join("home");
    let path_windata = path_prof.join("windata");

    if path_gamesave.exists() {
        return Ok(());
    }
    println!("[partydeck] Creating game save {} for {}", uid, name);

    std::fs::create_dir_all(&path_gamesave)?;
    
    if let Some(appid) = h.steam_appid && h.use_goldberg {
        let path_exec = path_gamesave.join(&h.exec);
        let path_execdir = path_exec.parent().ok_or_else(|| "couldn't get parent")?;
        if !path_execdir.exists() {
            std::fs::create_dir_all(&path_execdir)?;
        }
        std::fs::write(path_execdir.join("steam_appid.txt"), appid.to_string())?;
    }

    let profile_copy_gamesave = PathBuf::from(&h.path_handler).join("profile_copy_gamesave");
    if profile_copy_gamesave.exists() {
        copy_dir_recursive(&profile_copy_gamesave, &path_gamesave)?;
    }

    let profile_copy_home = PathBuf::from(&h.path_handler).join("profile_copy_home");
    if profile_copy_home.exists() {
        copy_dir_recursive(&profile_copy_home, &path_home)?;
    }

    let profile_copy_windata = PathBuf::from(&h.path_handler).join("profile_copy_windata");
    if profile_copy_windata.exists() {
        copy_dir_recursive(&profile_copy_windata, &path_windata)?;
    }

    println!("[partydeck] Profile save data created successfully");
    Ok(())
}

// Gets a vector of all available profiles.
// include_guest true for building the profile selector dropdown, false for the profile viewer.
pub fn scan_profiles(include_guest: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(PATH_PARTY.join("profiles")) {
        for entry in entries {
            if let Ok(entry) = entry
                && entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                && let Some(name) = entry.file_name().to_str()
            {
                out.push(name.to_string());
            }
        }
    }

    out.sort();

    if include_guest {
        out.insert(0, "Guest".to_string());
    }

    out
}

pub fn remove_guest_profiles() -> Result<(), Box<dyn Error>> {
    let path_profiles = PATH_PARTY.join("profiles");
    let entries = std::fs::read_dir(&path_profiles)?;
    for entry in entries.flatten() {
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with(".") {
            std::fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

pub fn avatar_path(name: &str) -> PathBuf {
    PATH_PARTY.join(format!("profiles/{name}/avatar.png"))
}

pub fn read_avatar_base64(name: &str) -> Option<String> {
    if !valid_profile_name(name) {
        return None;
    }
    let bytes = std::fs::read(avatar_path(name)).ok()?;
    Some(STANDARD.encode(bytes))
}

pub fn set_avatar_custom(name: &str, src: &Path) -> Result<(), Box<dyn Error>> {
    if !valid_profile_name(name) {
        return Err(format!("invalid profile name: {name}").into());
    }
    if src.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("png")) != Some(true) {
        return Err("avatar must be a .png file".into());
    }
    let dest = avatar_path(name);
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::copy(src, &dest)?;
    Ok(())
}

pub fn clear_avatar(name: &str) -> Result<(), Box<dyn Error>> {
    if !valid_profile_name(name) {
        return Err(format!("invalid profile name: {name}").into());
    }
    let path = avatar_path(name);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn builtin_avatars_dir() -> PathBuf {
    PATH_RES.join("avatars")
}

#[derive(Serialize)]
pub struct BuiltinAvatar {
    pub id: String,
    pub b64: String,
}

pub fn list_builtin_avatars() -> Vec<BuiltinAvatar> {
    let mut out: Vec<BuiltinAvatar> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(builtin_avatars_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("png") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            out.push(BuiltinAvatar { id: id.to_string(), b64: STANDARD.encode(bytes) });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

pub fn set_avatar_builtin(name: &str, id: &str) -> Result<(), Box<dyn Error>> {
    if !valid_profile_name(name) {
        return Err(format!("invalid profile name: {name}").into());
    }
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(format!("invalid avatar id: {id}").into());
    }
    let src = builtin_avatars_dir().join(format!("{id}.png"));
    if !src.is_file() {
        return Err(format!("built-in avatar not found: {id}").into());
    }
    let dest = avatar_path(name);
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::copy(&src, &dest)?;
    Ok(())
}

pub static GUEST_NAMES: [&str; 33] = [
    "Blinky", "Pinky", "Inky", "Clyde", "Beatrice", "Battler", "Miyao", "Rena", "Ellie", "Joel",
    "Leon", "Ada", "Madeline", "Theo", "Yokatta", "Wyrm", "Brodiee", "Supreme", "Conk", "Gort",
    "Lich", "Smores", "Canary", "Trico", "Yorda", "Wander", "Agro", "Jak", "Daxter", "Soap",
    "Ghost", "Tomi", "Masaki",
];
