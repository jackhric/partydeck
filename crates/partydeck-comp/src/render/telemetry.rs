use std::time::Duration;

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
pub struct FrameRecord {
    pub frame_seq: u64,
    pub tick: Duration,
    pub redraw_start: Duration,
    pub submit_done: Duration,
    pub callbacks_sent: usize,
    pub presented: u32,
    pub discarded: u32,
}

#[cfg(not(feature = "telemetry"))]
pub use disabled::Telemetry;
#[cfg(feature = "telemetry")]
pub use enabled::Telemetry;

#[cfg(not(feature = "telemetry"))]
mod disabled {
    use super::FrameRecord;

    pub struct Telemetry;

    impl Telemetry {
        pub fn new() -> Self {
            Telemetry
        }
        pub fn legacy_ack(&self) -> bool {
            false
        }
        pub fn note_surface_commit(&mut self) {}
        pub fn note_window_commit(&mut self) {}
        pub fn report(&mut self) {}
        pub fn frame_done(&mut self, _record: FrameRecord) {}
    }
}

#[cfg(feature = "telemetry")]
mod enabled {
    use std::ffi::OsString;
    use std::fs::File;
    use std::io::{BufWriter, Write};
    use std::time::Instant;

    use super::FrameRecord;

    const FRAME_LOG_FLUSH_ROWS: u32 = 60;
    const REPORT_EVERY_SECS: u64 = 5;

    pub struct Telemetry {
        frames: u32,
        child_commits: u32,
        commits_since_composite: u32,
        commit_gaps: [u32; 3],
        last_commit_at: Option<Instant>,
        last_report: Instant,
        legacy_ack: bool,
        frame_log: Option<FrameLog>,
    }

    impl Telemetry {
        pub fn new() -> Self {
            let legacy_ack = std::env::var("PARTYDECK_COMP_LEGACY_ACK").is_ok_and(|v| v == "1");
            if legacy_ack {
                eprintln!("[comp] legacy on-commit present acks enabled");
            }
            let frame_log = std::env::var_os("PARTYDECK_COMP_FRAME_LOG").and_then(FrameLog::open);
            Self {
                frames: 0,
                child_commits: 0,
                commits_since_composite: 0,
                commit_gaps: [0; 3],
                last_commit_at: None,
                last_report: Instant::now(),
                legacy_ack,
                frame_log,
            }
        }

        pub fn legacy_ack(&self) -> bool {
            self.legacy_ack
        }

        pub fn note_surface_commit(&mut self) {
            self.child_commits += 1;
            self.commits_since_composite += 1;
        }

        pub fn note_window_commit(&mut self) {
            let now = Instant::now();
            if let Some(prev) = self.last_commit_at.replace(now) {
                let bucket = match now.duration_since(prev).as_millis() {
                    0..=19 => 0,
                    20..=49 => 1,
                    _ => 2,
                };
                self.commit_gaps[bucket] += 1;
            }
        }

        pub fn report(&mut self) {
            let since = self.last_report.elapsed();
            if since.as_secs() < REPORT_EVERY_SECS {
                return;
            }
            eprintln!(
                "[comp] fps: {:.1}, child commits/s: {:.1}, gaps <20ms/20-50/>50: {}/{}/{}",
                self.frames as f64 / since.as_secs_f64(),
                self.child_commits as f64 / since.as_secs_f64(),
                self.commit_gaps[0],
                self.commit_gaps[1],
                self.commit_gaps[2]
            );
            self.frames = 0;
            self.child_commits = 0;
            self.commit_gaps = [0; 3];
            self.last_report = Instant::now();
        }

        pub fn frame_done(&mut self, record: FrameRecord) {
            self.frames += 1;
            if let Some(log) = &mut self.frame_log {
                log.write(&record, self.commits_since_composite);
            }
            self.commits_since_composite = 0;
        }
    }

    struct FrameLog {
        writer: BufWriter<File>,
        rows: u32,
    }

    impl FrameLog {
        fn open(path: OsString) -> Option<Self> {
            match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                Ok(file) => {
                    let mut writer = BufWriter::new(file);
                    if writer
                        .get_ref()
                        .metadata()
                        .map(|m| m.len() == 0)
                        .unwrap_or(false)
                    {
                        let _ = writeln!(
                            writer,
                            "frame_seq,tick_mono_ns,redraw_start_ns,submit_done_ns,children_committed_since_last,callbacks_sent,feedback_presented,feedback_discarded"
                        );
                    }
                    Some(Self { writer, rows: 0 })
                }
                Err(e) => {
                    eprintln!("[comp] frame log {}: {e}", path.to_string_lossy());
                    None
                }
            }
        }

        fn write(&mut self, r: &FrameRecord, commits: u32) {
            let _ = writeln!(
                self.writer,
                "{},{},{},{},{},{},{},{}",
                r.frame_seq,
                r.tick.as_nanos(),
                r.redraw_start.as_nanos(),
                r.submit_done.as_nanos(),
                commits,
                r.callbacks_sent,
                r.presented,
                r.discarded
            );
            self.rows += 1;
            if self.rows >= FRAME_LOG_FLUSH_ROWS {
                let _ = self.writer.flush();
                self.rows = 0;
            }
        }
    }
}
