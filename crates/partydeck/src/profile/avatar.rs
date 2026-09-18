use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Serialize;

use super::{is_safe_profile_name, profile_dir};
use crate::error::Result;
use crate::paths::PATH_RES;

#[derive(Serialize)]
pub struct BuiltinAvatar {
    pub id: String,
    pub b64: String,
}

fn avatar_path(name: &str) -> PathBuf {
    profile_dir(name).join("avatar.png")
}

fn builtin_avatars_dir() -> PathBuf {
    PATH_RES.join("avatars")
}

fn checked_avatar_path(name: &str) -> Result<PathBuf> {
    if !is_safe_profile_name(name) {
        return Err(format!("invalid profile name: {name}").into());
    }
    Ok(avatar_path(name))
}

fn install_avatar(name: &str, src: &Path) -> Result<()> {
    let dest = checked_avatar_path(name)?;
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::copy(src, &dest)?;
    Ok(())
}

pub fn read_avatar_base64(name: &str) -> Option<String> {
    if !is_safe_profile_name(name) {
        return None;
    }
    let bytes = std::fs::read(avatar_path(name)).ok()?;
    Some(STANDARD.encode(bytes))
}

pub fn set_avatar_custom(name: &str, src: &Path) -> Result<()> {
    let is_png = src
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if !is_png {
        return Err("avatar must be a .png file".into());
    }
    install_avatar(name, src)
}

pub fn set_avatar_builtin(name: &str, id: &str) -> Result<()> {
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(format!("invalid avatar id: {id}").into());
    }
    let src = builtin_avatars_dir().join(format!("{id}.png"));
    if !src.is_file() {
        return Err(format!("built-in avatar not found: {id}").into());
    }
    install_avatar(name, &src)
}

pub fn clear_avatar(name: &str) -> Result<()> {
    let path = checked_avatar_path(name)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
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
            out.push(BuiltinAvatar {
                id: id.to_string(),
                b64: STANDARD.encode(bytes),
            });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}
