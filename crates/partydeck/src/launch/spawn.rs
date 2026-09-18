use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::Duration;

use super::command::{CommandContext, build_commands};
use super::log::SessionLog;
use crate::error::Result;

const DEFAULT_PAUSE_BETWEEN_STARTS: f64 = 0.5;

/// Builds, starts and waits for every instance. Instances start staggered by
/// the handler's pause so games that grab a lock file do not race each other.
pub fn launch_game(ctx: &CommandContext) -> Result<()> {
    let cmds = build_commands(ctx)?;
    print_launch_cmds(&cmds);
    if let Some(log) = ctx.log {
        log.write_manifest(ctx.handler, ctx.instances, &cmds);
    }

    let pause = Duration::from_secs_f64(
        ctx.handler
            .pause_between_starts
            .unwrap_or(DEFAULT_PAUSE_BETWEEN_STARTS),
    );
    let handles = spawn_staggered(cmds, ctx.log, pause)?;

    let mut statuses: Vec<Option<ExitStatus>> = Vec::new();
    let mut wait_err: Option<std::io::Error> = None;
    for mut handle in handles {
        match handle.wait() {
            Ok(status) => statuses.push(Some(status)),
            Err(e) => {
                statuses.push(None);
                wait_err.get_or_insert(e);
            }
        }
    }
    if let Some(log) = ctx.log {
        log.finalize(&statuses);
    }
    match wait_err {
        Some(e) => Err(e.into()),
        None => Ok(()),
    }
}

fn spawn_staggered(
    cmds: Vec<Command>,
    log: Option<&SessionLog>,
    pause: Duration,
) -> Result<Vec<Child>> {
    let count = cmds.len();
    let mut handles = Vec::with_capacity(count);
    for (i, mut cmd) in cmds.into_iter().enumerate() {
        if let Some(file) = log.and_then(|log| log.instance_log(i)) {
            // Redirecting the outer gamescope captures the whole
            // gamescope -> bwrap -> umu -> game subtree.
            match file.try_clone() {
                Ok(file2) => {
                    cmd.stdout(Stdio::from(file));
                    cmd.stderr(Stdio::from(file2));
                }
                Err(e) => eprintln!("[partydeck] Failed to clone instance log handle: {e}"),
            }
        }
        let handle = cmd.spawn().map_err(|e| {
            format!(
                "Failed to start '{}': {e}",
                cmd.get_program().to_string_lossy()
            )
        })?;
        handles.push(handle);
        if i + 1 < count {
            std::thread::sleep(pause);
        }
    }
    Ok(handles)
}

fn print_launch_cmds(cmds: &[Command]) {
    for (i, cmd) in cmds.iter().enumerate() {
        eprintln!("[partydeck] INSTANCE {}:", i + 1);
        let cwd = cmd.get_current_dir().unwrap_or_else(|| Path::new(""));
        eprintln!("[partydeck] CWD={}", cwd.display());
        for (key, value) in cmd.get_envs() {
            let value = value.unwrap_or_default();
            eprintln!("[partydeck] {}={}", key.to_string_lossy(), value.display());
        }
        eprintln!("[partydeck] \"{}\"", cmd.get_program().display());

        let mut line = String::from("[partydeck] ");
        for arg in cmd.get_args() {
            let arg = arg.to_string_lossy();
            let starts_segment =
                arg == "--bind" || arg == "bwrap" || (arg.starts_with('/') && arg.len() > 1);
            if starts_segment {
                eprintln!("{line}");
                line = String::from("[partydeck] ");
            } else if line.len() > "[partydeck] ".len() {
                line.push(' ');
            }
            line.push_str(&format!("\"{arg}\""));
        }
        eprintln!("{line}");
        eprintln!("[partydeck] ---------------------");
    }
}
