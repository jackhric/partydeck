use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

pub static PATH_HOME: LazyLock<PathBuf> = LazyLock::new(home_dir);

pub static PATH_PARTY: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Some(xdg_data_home) = env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(xdg_data_home).join("partydeck");
    }
    PATH_HOME.join(".local/share/partydeck")
});

pub static PATH_RES: LazyLock<PathBuf> = LazyLock::new(|| {
    let system_install = PathBuf::from("/usr/share/partydeck");
    if system_install.exists() {
        return system_install;
    }
    exe_dir().join("res")
});

pub static BIN_UMU_RUN: LazyLock<PathBuf> = LazyLock::new(|| bundled_bin("umu-run"));
pub static BIN_GSC_KBM: LazyLock<PathBuf> = LazyLock::new(|| bundled_bin("gamescope-kbm"));
pub static BIN_COMP: LazyLock<PathBuf> = LazyLock::new(|| bundled_bin("partydeck-comp"));

/// HOME, else the parent of XDG_DATA_HOME, else the system temp dir. Startup
/// reports a missing HOME through `ensure_data_dirs` rather than panicking here.
pub fn home_dir() -> PathBuf {
    if let Some(home) = env::var_os("HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(home);
    }
    if let Some(xdg) = env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        let xdg = PathBuf::from(xdg);
        if let Some(home) = xdg.ancestors().nth(2) {
            return home.to_path_buf();
        }
    }
    eprintln!("[partydeck] HOME is not set; falling back to the system temp dir");
    env::temp_dir()
}

fn exe_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn bundled_bin(name: &str) -> PathBuf {
    pathsearch::find_executable_in_path(name).unwrap_or_else(|| exe_dir().join("bin").join(name))
}

pub fn profiles_dir() -> PathBuf {
    PATH_PARTY.join("profiles")
}

pub fn handlers_dir() -> PathBuf {
    PATH_PARTY.join("handlers")
}

pub fn tmp_dir() -> PathBuf {
    PATH_PARTY.join("tmp")
}

pub fn prefixes_dir() -> PathBuf {
    PATH_PARTY.join("prefixes")
}

pub fn logs_dir() -> PathBuf {
    PATH_PARTY.join("logs")
}

pub fn goldberg_data_dir() -> PathBuf {
    PATH_PARTY.join("goldberg_data")
}

pub fn settings_file() -> PathBuf {
    PATH_PARTY.join("settings.json")
}

/// First-run bootstrap of the data directory layout.
pub fn ensure_data_dirs() -> io::Result<()> {
    std::fs::create_dir_all(handlers_dir())?;
    std::fs::create_dir_all(profiles_dir())?;

    let goldberg = goldberg_data_dir();
    if !goldberg.exists() {
        let settings = goldberg.join("steam_settings");
        std::fs::create_dir_all(&settings)?;
        std::fs::write(settings.join("auto_accept_invite.txt"), "")?;
        std::fs::write(settings.join("auto_send_invite.txt"), "")?;
    }
    Ok(())
}
