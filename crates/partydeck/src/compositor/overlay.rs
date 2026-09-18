use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::paths::BIN_COMP;

/// The overlay is optional chrome: no shell binary means the session runs bare.
pub(super) fn spawn_cef_overlay(socket_prefix: &str, control_path: &Path) -> Option<Child> {
    let shell = match std::env::var_os("PARTYDECK_CEF_OVERLAY") {
        Some(path) => PathBuf::from(path),
        None => BIN_COMP.parent()?.join("cef-overlay/cef-overlay"),
    };
    if !shell.exists() {
        return None;
    }
    let shell_dir = shell.parent()?.to_path_buf();
    let url = std::env::var("OVERLAY_URL")
        .unwrap_or_else(|_| format!("file://{}/overlay.html", shell_dir.display()));

    match Command::new(&shell)
        .env("WAYLAND_DISPLAY", format!("{socket_prefix}-overlay"))
        .env("PARTYDECK_CTL_SOCKET", control_path)
        .env("OVERLAY_URL", url)
        .args(["--ozone-platform=headless", "--disable-gpu", "--no-sandbox"])
        .current_dir(&shell_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => {
            eprintln!("[partydeck] cef-overlay spawned ({})", shell.display());
            Some(child)
        }
        Err(e) => {
            eprintln!(
                "[partydeck] warning: failed to spawn cef-overlay {}: {e}",
                shell.display()
            );
            None
        }
    }
}
