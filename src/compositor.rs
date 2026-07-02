use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use partydeck_comp::layout::Layout;

use crate::paths::BIN_COMP;

pub struct Compositor {
    child: Child,
    pub socket_prefix: String,
    control_path: PathBuf,
}

impl Compositor {
    pub fn spawn(
        layout: &Layout,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").map_err(|_| "XDG_RUNTIME_DIR not set")?;
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

        let mut child = Command::new(bin)
            .arg("--socket-prefix")
            .arg(&socket_prefix)
            .arg("--layout")
            .arg(&layout_path)
            .arg("--size")
            .arg(format!("{width}x{height}"))
            .arg("--fullscreen")
            .stdout(Stdio::piped())
            .spawn()?;

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

        println!("[partydeck] compositor ready, sockets {socket_prefix}-p0..");
        Ok(Self { child, socket_prefix, control_path })
    }

    pub fn player_socket(&self, slot: usize) -> String {
        format!("{}-p{}", self.socket_prefix, slot)
    }

    pub fn send(&self, command: &partydeck_comp::ipc::Command) -> Result<partydeck_comp::ipc::Response, Box<dyn std::error::Error>> {
        let mut stream = UnixStream::connect(&self.control_path)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.write_all(partydeck_comp::ipc::encode(command)?.as_bytes())?;
        let mut buf = String::new();
        BufReader::new(&mut stream).read_line(&mut buf)?;
        Ok(serde_json::from_str(&buf)?)
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        let _ = self.send(&partydeck_comp::ipc::Command::Quit);
        std::thread::sleep(Duration::from_millis(200));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
