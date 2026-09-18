use std::path::PathBuf;
use std::sync::LazyLock;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::paths::PATH_HOME;

const MAX_LOGO_BYTES: usize = 512 * 1024;

pub static PATH_STEAM: LazyLock<PathBuf> = LazyLock::new(|| {
    let native = PATH_HOME.join(".steam");
    let flatpak = PATH_HOME.join(".var/app/com.valvesoftware.Steam/.steam/steam");
    if !native.exists() && flatpak.exists() {
        flatpak
    } else {
        native
    }
});

pub fn get_installed_steamapps() -> Vec<steamlocate::App> {
    let Ok(steam_dir) = steamlocate::SteamDir::locate() else {
        return Vec::new();
    };
    let Ok(libraries) = steam_dir.libraries() else {
        return Vec::new();
    };
    libraries
        .filter_map(|library| library.ok())
        .flat_map(|library| {
            library
                .apps()
                .filter_map(|app| app.ok())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn find_app(appid: u32) -> Option<(steamlocate::App, steamlocate::Library)> {
    steamlocate::SteamDir::locate()
        .ok()?
        .find_app(appid)
        .ok()
        .flatten()
}

pub fn app_install_dir(appid: u32) -> Option<PathBuf> {
    let (app, library) = find_app(appid)?;
    let path = library.resolve_app_dir(&app);
    path.exists().then_some(path)
}

pub fn app_install_dir_name(appid: u32) -> Option<String> {
    find_app(appid).map(|(app, _)| app.install_dir)
}

pub fn app_logo_base64(appid: u32) -> Option<String> {
    let steam_dir = steamlocate::SteamDir::locate().ok()?;
    let cache = steam_dir
        .path()
        .join(format!("appcache/librarycache/{appid}"));

    let logo = cache.join("logo.png");
    let path = if logo.exists() {
        logo
    } else {
        std::fs::read_dir(&cache)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with("_logo.png"))
            })?
    };

    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > MAX_LOGO_BYTES {
        return None;
    }
    Some(STANDARD.encode(bytes))
}
