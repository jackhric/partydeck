mod overlay;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use partydeck_comp_proto::ipc;
use partydeck_comp_proto::layout::Layout;

use crate::error::Result;
use crate::monitor::Monitor;
use crate::paths::BIN_COMP;

const READY_TIMEOUT: Duration = Duration::from_secs(10);
const SOCKET_PREFIX: &str = "partydeck-";

pub struct Compositor {
    child: Child,
    overlay_child: Option<Child>,
    pub socket_prefix: String,
    control_path: PathBuf,
}

impl Compositor {
    /// Starts partydeck-comp for `layout` and blocks until it prints READY.
    pub fn spawn(layout: &Layout, monitor: &Monitor, border_style: &str) -> Result<Self> {
        let runtime_dir =
            std::env::var("XDG_RUNTIME_DIR").map_err(|_| "XDG_RUNTIME_DIR not set")?;
        sweep_stale_sockets(&runtime_dir);
        let dir = PathBuf::from(&runtime_dir).join("partydeck");
        std::fs::create_dir_all(&dir)?;
        let layout_path = dir.join("layout.json");
        std::fs::write(&layout_path, serde_json::to_string_pretty(layout)?)?;

        let socket_prefix = format!("{SOCKET_PREFIX}{}", std::process::id());
        let control_path = PathBuf::from(&runtime_dir).join(format!("{socket_prefix}.ctl"));

        let bin = BIN_COMP.as_path();
        if !bin.exists() && pathsearch::find_executable_in_path("partydeck-comp").is_none() {
            return Err("partydeck-comp is missing. Please reinstall partydeck.".into());
        }

        // A size override means we are probably nested (e.g. under gamescope),
        // where fullscreen would snap back to the panel size; stay windowed.
        let (width, height) = (monitor.width(), monitor.height());
        let windowed = monitor.size_overridden();

        let mut cmd = Command::new(bin);
        cmd.arg("--socket-prefix")
            .arg(&socket_prefix)
            .arg("--layout")
            .arg(&layout_path)
            .arg("--size")
            .arg(format!("{width}x{height}"))
            .arg("--border")
            .arg(border_style);
        if !windowed {
            cmd.arg("--fullscreen");
        }
        let mut child = cmd.stdout(Stdio::piped()).spawn()?;

        if let Err(e) = wait_for_ready(&mut child) {
            let _ = child.kill();
            return Err(e);
        }

        let mode = if windowed { "windowed" } else { "fullscreen" };
        eprintln!(
            "[partydeck] compositor ready, sockets {socket_prefix}-p0.., size {width}x{height} {mode}"
        );
        let overlay_child = overlay::spawn_cef_overlay(&socket_prefix, &control_path);
        Ok(Self {
            child,
            overlay_child,
            socket_prefix,
            control_path,
        })
    }

    pub fn player_socket(&self, slot: usize) -> String {
        format!("{}-p{}", self.socket_prefix, slot)
    }

    pub fn send(&self, command: &ipc::Command) -> Result<ipc::Response> {
        let mut stream = UnixStream::connect(&self.control_path)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.write_all(ipc::encode(command)?.as_bytes())?;
        let mut buf = String::new();
        BufReader::new(&mut stream).read_line(&mut buf)?;
        Ok(serde_json::from_str(&buf)?)
    }
}

fn wait_for_ready(child: &mut Child) -> Result<()> {
    let stdout = child.stdout.take().ok_or("no compositor stdout")?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        // Keep draining after READY so the pipe never fills.
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            let _ = tx.send(line);
        }
    });

    let deadline = std::time::Instant::now() + READY_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(line) if line == "READY" => return Ok(()),
            Ok(line) => eprintln!("[partydeck] compositor: {line}"),
            Err(_) => {
                return Err(format!(
                    "partydeck-comp did not become ready within {}s",
                    READY_TIMEOUT.as_secs()
                )
                .into());
            }
        }
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        if let Some(overlay) = self.overlay_child.as_mut() {
            let _ = overlay.kill();
            let _ = overlay.wait();
        }
        let _ = self.send(&ipc::Command::Quit);
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
        let Some(rest) = name.to_str().and_then(|n| n.strip_prefix(SOCKET_PREFIX)) else {
            continue;
        };
        let Some(pid) = rest
            .split(['-', '.'])
            .next()
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        if !PathBuf::from(format!("/proc/{pid}")).exists() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}
