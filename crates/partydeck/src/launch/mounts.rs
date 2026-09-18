use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::PartyConfig;
use crate::error::Result;
use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::tmp_dir;
use crate::profile::profile_dir;

const FUSE_OVERLAYFS_MISSING: &str = "Fuse-overlayfs executable not found; Please install fuse-overlayfs through your distro's package manager. If you already have it installed (or are on SteamOS, where it should be pre-installed), open up an issue on the GitHub.";

/// Saved handlers run from a per-instance overlay of the game directory unless
/// the config opts out.
pub fn gamedirs_are_mounted(h: &Handler, cfg: &PartyConfig) -> bool {
    h.is_saved_handler() && !cfg.disable_mount_gamedirs && cfg.profile_unique_dirs
}

pub fn mounted_gamedir(instance_index: usize) -> PathBuf {
    tmp_dir().join(format!("game-{instance_index}"))
}

fn is_mount_point(dir: &Path) -> bool {
    Command::new("mountpoint")
        .arg(dir)
        .status()
        .is_ok_and(|status| status.success())
}

fn fuse_overlayfs_unmount_gamedirs() -> Result<()> {
    let tmp = tmp_dir();
    let entries = std::fs::read_dir(&tmp)
        .map_err(|e| format!("Failed to read directory {}: {e}", tmp.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir()
            || !entry.file_name().to_string_lossy().starts_with("game-")
            || !is_mount_point(&path)
        {
            continue;
        }
        let status = Command::new("umount")
            .arg("-l")
            .arg("-v")
            .arg(&path)
            .status()?;
        if !status.success() {
            return Err(format!("Unmounting {} failed", path.display()).into());
        }
    }
    Ok(())
}

/// Unmounts any game overlays and removes the tmp directory.
pub fn clear_tmp() -> Result<()> {
    let tmp = tmp_dir();
    if !tmp.exists() {
        return Ok(());
    }
    fuse_overlayfs_unmount_gamedirs()?;
    std::fs::remove_dir_all(&tmp)?;
    Ok(())
}

/// Mounts `<handler>/overlay:<game root>` under each instance's profile save
/// dir, so writes land in the profile and handler mods sit on top of the game.
pub fn fuse_overlayfs_mount_gamedirs(h: &Handler, instances: &[Instance]) -> Result<()> {
    let tmp = tmp_dir();
    let mut lowerdir = h.get_game_rootpath()?;
    let overlay_path = h.path_handler.join("overlay");
    if overlay_path.exists() {
        lowerdir = format!("{}:{}", overlay_path.display(), lowerdir);
    }
    let gamename = h.handler_dir_name().to_string();

    let mut cmds = Vec::with_capacity(instances.len());
    for (i, instance) in instances.iter().enumerate() {
        let mount = mounted_gamedir(i);
        let workdir = tmp.join(format!("work-{i}"));
        let upperdir = profile_dir(&instance.profname)
            .join("gamesaves")
            .join(&gamename);
        std::fs::create_dir_all(&mount)?;
        std::fs::create_dir_all(&workdir)?;

        let mut cmd = Command::new("fuse-overlayfs");
        cmd.arg("-o")
            .arg(format!("lowerdir={lowerdir}"))
            .arg("-o")
            .arg(format!("upperdir={}", upperdir.display()))
            .arg("-o")
            .arg(format!("workdir={}", workdir.display()))
            .arg(&mount);
        cmds.push(cmd);
    }

    for cmd in &mut cmds {
        let status = cmd.status().map_err(|_| FUSE_OVERLAYFS_MISSING)?;
        if !status.success() {
            return Err("fuse-overlayfs mount failed.".into());
        }
    }
    Ok(())
}
