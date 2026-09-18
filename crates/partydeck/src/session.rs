use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::app::PartyConfig;
use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::PATH_PARTY;

const FALLBACK_SESSIONS_KEPT: usize = 20;

pub struct Session {
    dir: PathBuf,
    started_at: u64,
    cfg: PartyConfig,
}

impl Session {
    pub fn create(cfg: &PartyConfig) -> Option<Self> {
        let env_dir =
            std::env::var_os("PARTYDECK_SESSION_LOG_DIR").filter(|d| !d.is_empty());
        let using_fallback = env_dir.is_none();
        let dir = env_dir.map(PathBuf::from).unwrap_or_else(|| {
            PATH_PARTY
                .join("logs")
                .join(format!("session-{}", epoch_secs()))
        });
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!(
                "[partydeck] Failed to create session log dir {}: {e}",
                dir.display()
            );
            return None;
        }
        if using_fallback {
            prune_fallback_sessions(&PATH_PARTY.join("logs"), &dir);
        }
        Some(Session {
            dir,
            started_at: epoch_secs(),
            cfg: cfg.clone(),
        })
    }

    pub fn instance_log(&self, i: usize) -> Option<File> {
        let path = self.dir.join(format!("instance-{i}.log"));
        match File::create(&path) {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!(
                    "[partydeck] Failed to create instance log {}: {e}",
                    path.display()
                );
                None
            }
        }
    }

    pub fn proton_log_dir(&self, i: usize) -> PathBuf {
        self.dir.join(format!("proton-{i}"))
    }

    pub fn write_manifest(&self, h: &Handler, instances: &[Instance], cmds: &[Command]) {
        let win = h.win();
        let instances_json: Vec<Value> = cmds
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                let env: serde_json::Map<String, Value> = cmd
                    .get_envs()
                    .map(|(k, v)| {
                        (
                            k.to_string_lossy().into_owned(),
                            match v {
                                Some(v) => Value::String(v.to_string_lossy().into_owned()),
                                None => Value::Null,
                            },
                        )
                    })
                    .collect();
                json!({
                    "index": i,
                    "profile": instances.get(i).map(|inst| inst.profname.clone()),
                    "cwd": cmd.get_current_dir().map(|p| p.to_string_lossy().into_owned()),
                    "program": cmd.get_program().to_string_lossy(),
                    "args": cmd
                        .get_args()
                        .map(|a| a.to_string_lossy().into_owned())
                        .collect::<Vec<_>>(),
                    "env": env,
                    "stdout_log": format!("instance-{i}.log"),
                    "proton_log_dir": if win && self.cfg.debug_game_logs {
                        Some(format!("proton-{i}"))
                    } else {
                        None
                    },
                    "exit_code": Value::Null,
                    "signal": Value::Null,
                })
            })
            .collect();

        let manifest = json!({
            "schema": 1,
            "started_at": self.started_at,
            "finished_at": Value::Null,
            "handler": h.name,
            "steam_appid": h.steam_appid,
            "win": win,
            "debug_game_logs": self.cfg.debug_game_logs,
            "config": &self.cfg,
            "instances": instances_json,
        });
        self.write_json("session.json", &manifest);

        match serde_json::to_value(h) {
            Ok(handler_json) => self.write_json("handler.json", &handler_json),
            Err(e) => eprintln!("[partydeck] Failed to serialize handler: {e}"),
        }
    }

    pub fn finalize(&self, statuses: &[Option<ExitStatus>]) {
        use std::os::unix::process::ExitStatusExt;

        let path = self.dir.join("session.json");
        let mut manifest: Value = match fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[partydeck] Failed to read back {}: {e}", path.display());
                return;
            }
        };

        if let Some(insts) = manifest
            .get_mut("instances")
            .and_then(|v| v.as_array_mut())
        {
            for (inst, status) in insts.iter_mut().zip(statuses) {
                if let Some(status) = status {
                    inst["exit_code"] = status.code().map_or(Value::Null, Value::from);
                    inst["signal"] = status.signal().map_or(Value::Null, Value::from);
                }
            }
        }
        manifest["finished_at"] = json!(epoch_secs());
        self.write_json("session.json", &manifest);
    }

    fn write_json(&self, name: &str, value: &impl serde::Serialize) {
        let path = self.dir.join(name);
        let result = File::create(&path)
            .map_err(|e| e.to_string())
            .and_then(|f| serde_json::to_writer_pretty(f, value).map_err(|e| e.to_string()));
        if let Err(e) = result {
            eprintln!("[partydeck] Failed to write {}: {e}", path.display());
        }
    }
}

fn epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn prune_fallback_sessions(logs_dir: &Path, current: &Path) {
    let Ok(entries) = fs::read_dir(logs_dir) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p != current
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("session-"))
        })
        .collect();
    dirs.sort();
    // Keep the newest 19 siblings; the current dir makes 20.
    while dirs.len() > FALLBACK_SESSIONS_KEPT - 1 {
        let victim = dirs.remove(0);
        if let Err(e) = fs::remove_dir_all(&victim) {
            eprintln!(
                "[partydeck] Failed to prune old session {}: {e}",
                victim.display()
            );
        }
    }
}
