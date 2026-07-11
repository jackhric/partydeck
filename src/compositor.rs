use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use partydeck_comp_proto::layout::Layout;

use crate::paths::BIN_COMP;

pub struct Compositor {
    child: Child,
    overlay_child: Option<Child>,
    pub socket_prefix: String,
    control_path: PathBuf,
}

impl Compositor {
    pub fn spawn(
        layout: &Layout,
        width: u32,
        height: u32,
        border_style: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").map_err(|_| "XDG_RUNTIME_DIR not set")?;
        sweep_stale_sockets(&runtime_dir);
        let dir = PathBuf::from(&runtime_dir).join("partydeck");
        std::fs::create_dir_all(&dir)?;
        let layout_path = dir.join("layout.json");
        std::fs::write(&layout_path, serde_json::to_string_pretty(layout)?)?;

        let socket_prefix = format!("partydeck-{}", std::process::id());
        let control_path = PathBuf::from(&runtime_dir).join(format!("{socket_prefix}.ctl"));

        let bin = BIN_COMP.as_path();
        if !bin.exists() && pathsearch::find_executable_in_path("partydeck-comp").is_none() {
            return Err("partydeck-comp is missing. Please reinstall partydeck.".into());
        }

        // Under gamescope, fullscreen gets reconfigured to panel size, defeating an
        // explicit resolution override; windowed lets gamescope scale the buffer.
        let overridden = std::env::var_os("PARTYDECK_SCREEN_WIDTH").is_some()
            && std::env::var_os("PARTYDECK_SCREEN_HEIGHT").is_some();

        let mut cmd = Command::new(bin);
        cmd.arg("--socket-prefix")
            .arg(&socket_prefix)
            .arg("--layout")
            .arg(&layout_path)
            .arg("--size")
            .arg(format!("{width}x{height}"))
            .arg("--border")
            .arg(border_style);
        if !overridden {
            cmd.arg("--fullscreen");
        }
        let mut child = cmd.stdout(Stdio::piped()).spawn()?;

        let stdout = child.stdout.take().ok_or("no compositor stdout")?;
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            // Keep draining after READY so the pipe never fills.
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = tx.send(line);
            }
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match rx.recv_timeout(remaining) {
                Ok(line) if line == "READY" => break,
                Ok(line) => println!("[partydeck] compositor: {line}"),
                Err(_) => {
                    let _ = child.kill();
                    return Err("partydeck-comp did not become ready within 10s".into());
                }
            }
        }

        let mode = if overridden { "windowed" } else { "fullscreen" };
        println!(
            "[partydeck] compositor ready, sockets {socket_prefix}-p0.., size {width}x{height} {mode}"
        );
        let overlay_child = spawn_cef_overlay(&socket_prefix);
        Ok(Self { child, overlay_child, socket_prefix, control_path })
    }

    pub fn player_socket(&self, slot: usize) -> String {
        format!("{}-p{}", self.socket_prefix, slot)
    }

    pub fn send(&self, command: &partydeck_comp_proto::ipc::Command) -> Result<partydeck_comp_proto::ipc::Response, Box<dyn std::error::Error>> {
        let mut stream = UnixStream::connect(&self.control_path)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.write_all(partydeck_comp_proto::ipc::encode(command)?.as_bytes())?;
        let mut buf = String::new();
        BufReader::new(&mut stream).read_line(&mut buf)?;
        Ok(serde_json::from_str(&buf)?)
    }
}

// The overlay is optional chrome: no shell binary means the session runs bare.
fn spawn_cef_overlay(socket_prefix: &str) -> Option<Child> {
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
        .env("OVERLAY_URL", url)
        .args(["--ozone-platform=headless", "--disable-gpu", "--no-sandbox"])
        .current_dir(&shell_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => {
            println!("[partydeck] cef-overlay spawned ({})", shell.display());
            Some(child)
        }
        Err(e) => {
            println!("[partydeck] warning: failed to spawn cef-overlay {}: {e}", shell.display());
            None
        }
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        if let Some(overlay) = self.overlay_child.as_mut() {
            let _ = overlay.kill();
            let _ = overlay.wait();
        }
        let _ = self.send(&partydeck_comp_proto::ipc::Command::Quit);
        std::thread::sleep(Duration::from_millis(200));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// Socket names carry the spawning partydeck's pid; a pattern-killed (or
// SIGKILLed) session leaves them behind, so reap any whose owner is gone.
fn sweep_stale_sockets(runtime_dir: &str) {
    let Ok(entries) = std::fs::read_dir(runtime_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(rest) = name.strip_prefix("partydeck-") else {
            continue;
        };
        let Some(pid) = rest.split(['-', '.']).next().and_then(|p| p.parse::<u32>().ok()) else {
            continue;
        };
        if !PathBuf::from(format!("/proc/{pid}")).exists() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}
