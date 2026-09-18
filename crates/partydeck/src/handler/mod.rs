pub mod args;
pub mod package;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::handler::args::SanitizePath;
use crate::paths::handlers_dir;
use crate::steam;

pub const HANDLER_SPEC_CURRENT_VERSION: u16 = 3;

#[derive(Clone, Serialize, Deserialize, PartialEq, Default, Debug)]
pub enum SDL2Override {
    #[default]
    No,
    Srt,
    Sys,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Handler {
    #[serde(skip)]
    pub path_handler: PathBuf,
    #[serde(skip)]
    pub img_paths: Vec<PathBuf>,

    pub name: String,
    pub author: String,
    pub version: String,
    pub info: String,
    #[serde(default)]
    pub spec_ver: u16,

    pub path_gameroot: String,
    pub runtime: String,
    pub exec: String,
    pub args: String,
    pub env: String,
    #[serde(default)]
    pub sdl2_override: SDL2Override,

    pub pause_between_starts: Option<f64>,

    #[serde(default)]
    pub use_mangohud: bool,
    pub use_goldberg: bool,
    #[serde(default)]
    pub enable_hidraw: bool,
    pub steam_appid: Option<u32>,

    pub game_null_paths: Vec<String>,
}

impl Default for Handler {
    fn default() -> Self {
        Self {
            path_handler: PathBuf::new(),
            img_paths: Vec::new(),
            name: String::new(),
            author: String::new(),
            version: String::new(),
            info: String::new(),
            spec_ver: HANDLER_SPEC_CURRENT_VERSION,
            path_gameroot: String::new(),
            runtime: String::new(),
            exec: String::new(),
            args: String::new(),
            env: String::new(),
            sdl2_override: SDL2Override::No,
            pause_between_starts: None,
            use_mangohud: false,
            use_goldberg: false,
            enable_hidraw: false,
            steam_appid: None,
            game_null_paths: Vec::new(),
        }
    }
}

impl Handler {
    pub fn from_json(json_path: &Path) -> Result<Self> {
        let dir = json_path.parent().ok_or("Invalid path")?;
        let json = std::fs::read_to_string(json_path)?;
        Self::parse(&json, dir)
    }

    /// Parses handler.json contents for a handler living in `dir`.
    pub fn parse(json: &str, dir: &Path) -> Result<Self> {
        let mut handler: Handler = serde_json::from_str(json)?;
        handler.path_handler = dir.to_path_buf();
        handler.img_paths = handler.scan_imgs();
        for path in &mut handler.game_null_paths {
            *path = path.sanitize_path();
        }
        Ok(handler)
    }

    /// Throwaway handler for `--exec PATH --args ...`.
    pub fn from_cli(path_exec: &str, args: &str) -> Self {
        let exec = Path::new(path_exec);
        Handler {
            path_gameroot: exec
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            exec: exec
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            args: args.to_string(),
            ..Handler::default()
        }
    }

    pub fn display(&self) -> &str {
        self.name.as_str()
    }

    pub fn win(&self) -> bool {
        let extension = Path::new(self.exec.as_str())
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        matches!(extension.as_str(), "exe" | "bat" | "cmd")
    }

    pub fn is_saved_handler(&self) -> bool {
        !self.path_handler.as_os_str().is_empty()
    }

    pub fn handler_dir_name(&self) -> &str {
        self.path_handler
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    }

    fn scan_imgs(&self) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(self.path_handler.join("imgs")) else {
            return Vec::new();
        };
        let mut out: Vec<PathBuf> = entries
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|ft| ft.is_file()))
            .map(|entry| entry.path())
            .filter(|path| {
                path.to_str()
                    .is_some_and(|s| s.ends_with(".png") || s.ends_with(".jpg"))
            })
            .collect();
        out.sort();
        out
    }

    pub fn remove_handler(&self) -> Result<()> {
        if !self.is_saved_handler() {
            return Err("No handler directory to remove".into());
        }
        let handlers = handlers_dir().canonicalize()?;
        let path = self.path_handler.canonicalize()?;
        if path == handlers || !path.starts_with(&handlers) {
            return Err(format!(
                "Refusing to remove {}: not inside {}",
                path.display(),
                handlers.display()
            )
            .into());
        }
        std::fs::remove_dir_all(&path)?;
        Ok(())
    }

    /// Steam install dir when the appid is installed, else the configured root.
    pub fn get_game_rootpath(&self) -> Result<String> {
        if let Some(path) = self.steam_appid.and_then(steam::app_install_dir) {
            return Ok(path.to_string_lossy().to_string());
        }
        if !self.path_gameroot.is_empty() && Path::new(&self.path_gameroot).exists() {
            return Ok(self.path_gameroot.clone());
        }
        Err("Game root path not found".into())
    }

    pub fn save_to_json(&mut self) -> Result<()> {
        if !self.is_saved_handler() {
            if self.name.is_empty() {
                self.name = self
                    .steam_appid
                    .and_then(steam::app_install_dir_name)
                    .ok_or("Name cannot be empty")?;
            }
            self.path_handler = unique_handler_dir(&self.name);
        }
        std::fs::create_dir_all(&self.path_handler)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(self.path_handler.join("handler.json"), json)?;
        Ok(())
    }
}

/// `handlers/<name>`, or `handlers/<name>-N` for the first free N.
pub(crate) fn unique_handler_dir(name: &str) -> PathBuf {
    let base = handlers_dir();
    let candidate = base.join(name);
    if !candidate.exists() {
        return candidate;
    }
    (1..)
        .map(|i| base.join(format!("{name}-{i}")))
        .find(|p| !p.exists())
        .unwrap_or(candidate)
}

pub fn scan_handlers() -> Vec<Handler> {
    let Ok(entries) = std::fs::read_dir(handlers_dir()) else {
        return Vec::new();
    };
    let mut out: Vec<Handler> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|ft| ft.is_dir()))
        .map(|entry| entry.path().join("handler.json"))
        .filter(|json| json.exists())
        .filter_map(|json| Handler::from_json(&json).ok())
        .collect();
    out.sort_by_key(|h| h.display().to_lowercase());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"{
        "name": "Game", "author": "", "version": "1", "info": "",
        "path_gameroot": "/games/game", "runtime": "", "exec": "game.exe",
        "args": "", "env": "", "pause_between_starts": null,
        "use_goldberg": false, "steam_appid": null,
        "game_null_paths": ["/../secret/../file.txt"]
    }"#;

    #[test]
    fn parse_fills_optional_fields_with_defaults() {
        let h = Handler::parse(MINIMAL, Path::new("/handlers/game")).unwrap();
        assert_eq!(h.spec_ver, 0);
        assert_eq!(h.sdl2_override, SDL2Override::No);
        assert!(!h.use_mangohud);
        assert!(!h.enable_hidraw);
        assert_eq!(h.path_handler, Path::new("/handlers/game"));
        assert_eq!(h.handler_dir_name(), "game");
        assert_eq!(h.game_null_paths, vec!["secret/file.txt"]);
        assert!(h.win());
    }

    #[test]
    fn default_handler_is_current_spec_and_unsaved() {
        let h = Handler::default();
        assert_eq!(h.spec_ver, HANDLER_SPEC_CURRENT_VERSION);
        assert!(!h.is_saved_handler());
        assert!(!h.win());
    }

    #[test]
    fn from_cli_tolerates_root_path() {
        let h = Handler::from_cli("/", "");
        assert_eq!(h.path_gameroot, "");
        assert_eq!(h.exec, "");
        let h = Handler::from_cli("/games/g/Game.EXE", "-x");
        assert_eq!(h.path_gameroot, "/games/g");
        assert_eq!(h.exec, "Game.EXE");
        assert_eq!(h.args, "-x");
        assert!(h.win());
    }
}
