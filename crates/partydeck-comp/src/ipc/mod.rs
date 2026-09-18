pub mod commands;
pub mod state;

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use smithay::reexports::calloop::EventLoop;
use smithay::wayland::socket::ListeningSocketSource;

use partydeck_comp_proto::ipc::{Response, decode_command, encode};

use crate::CalloopData;
use crate::state::CompState;

pub const MAX_LINE: usize = 4 * 1024 * 1024;
pub const READ_DEADLINE: Duration = Duration::from_millis(300);

pub fn init(
    event_loop: &mut EventLoop<CalloopData>,
    socket_prefix: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let name = format!("{socket_prefix}.ctl");
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").map_err(|_| "XDG_RUNTIME_DIR not set")?;
    let path = PathBuf::from(runtime_dir).join(&name);

    let listener = ListeningSocketSource::with_name(&name)?;
    event_loop
        .handle()
        .insert_source(listener, move |stream, _, data| {
            // One command per connection, handled synchronously: sources inserted
            // mid-dispatch never fire, and the only client is PartyDeck main.
            handle_connection(&stream, &mut data.state);
        })?;
    Ok(path)
}

fn handle_connection(mut stream: &UnixStream, state: &mut CompState) {
    let Some(line) = read_line(stream, Instant::now() + READ_DEADLINE) else {
        return;
    };
    let response = match std::str::from_utf8(&line)
        .map_err(|e| e.to_string())
        .and_then(|s| decode_command(s).map_err(|e| e.to_string()))
    {
        Ok(cmd) => commands::apply(state, cmd),
        Err(e) => Response::Err(e),
    };
    match encode(&response) {
        Ok(text) => {
            let _ = stream.write_all(text.as_bytes());
        }
        Err(e) => eprintln!("[comp] ipc: cannot encode response: {e}"),
    }
}

/// Reads one newline-terminated line (newline excluded). Gives up at `deadline`,
/// on EOF, or once `MAX_LINE` bytes arrive without a newline.
pub fn read_line(mut stream: &UnixStream, deadline: Instant) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut scanned = 0;
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())?;
        stream.set_read_timeout(Some(remaining)).ok()?;
        let n = match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        };
        buf.extend_from_slice(&chunk[..n]);
        if let Some(rel) = buf[scanned..].iter().position(|&b| b == b'\n') {
            buf.truncate(scanned + rel);
            return Some(buf);
        }
        scanned = buf.len();
        if buf.len() >= MAX_LINE {
            return None;
        }
    }
}

pub fn remove_socket_files(control_path: &Path) {
    let _ = std::fs::remove_file(control_path);
    let mut lock = control_path.as_os_str().to_os_string();
    lock.push(".lock");
    let _ = std::fs::remove_file(lock);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deadline(ms: u64) -> Instant {
        Instant::now() + Duration::from_millis(ms)
    }

    #[test]
    fn returns_first_line_without_newline_and_ignores_the_rest() {
        let (mut tx, rx) = UnixStream::pair().unwrap();
        tx.write_all(b"{\"cmd\":\"quit\"}\ntrailing garbage")
            .unwrap();
        assert_eq!(
            read_line(&rx, deadline(500)).unwrap(),
            b"{\"cmd\":\"quit\"}"
        );
    }

    #[test]
    fn assembles_a_line_from_several_writes() {
        let (mut tx, rx) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || {
            for part in [&b"{\"cmd\":"[..], b"\"get_st", b"ate\"}\n"] {
                tx.write_all(part).unwrap();
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        assert_eq!(
            read_line(&rx, deadline(2000)).unwrap(),
            b"{\"cmd\":\"get_state\"}"
        );
        writer.join().unwrap();
    }

    #[test]
    fn gives_up_at_the_total_deadline_while_peer_stays_open() {
        let (mut tx, rx) = UnixStream::pair().unwrap();
        tx.write_all(b"partial").unwrap();
        let start = Instant::now();
        assert_eq!(read_line(&rx, deadline(100)), None);
        let waited = start.elapsed();
        assert!(
            waited >= Duration::from_millis(90),
            "returned early: {waited:?}"
        );
        assert!(
            waited < Duration::from_secs(2),
            "deadline overshoot: {waited:?}"
        );
        drop(tx);
    }

    #[test]
    fn eof_without_newline_is_not_a_line() {
        let (mut tx, rx) = UnixStream::pair().unwrap();
        tx.write_all(b"no newline").unwrap();
        drop(tx);
        assert_eq!(read_line(&rx, deadline(500)), None);
    }

    #[test]
    fn oversized_line_is_rejected() {
        let (mut tx, rx) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || {
            let block = vec![b'a'; 64 * 1024];
            while tx.write_all(&block).is_ok() {}
        });
        assert_eq!(read_line(&rx, deadline(10_000)), None);
        drop(rx);
        writer.join().unwrap();
    }
}
