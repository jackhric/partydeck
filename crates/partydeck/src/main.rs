mod app;
mod cli;
mod compositor;
mod config;
mod error;
mod fs;
mod handler;
mod input;
mod instance;
mod launch;
mod monitor;
mod paths;
mod profile;
mod steam;
mod update;

use clap::Parser;

use crate::handler::Handler;
use crate::launch::clear_tmp;
use crate::paths::{PATH_PARTY, ensure_data_dirs};
use crate::profile::remove_guest_profiles;

fn main() -> eframe::Result {
    let cli = cli::Cli::parse();
    if let Some(command) = cli.command {
        std::process::exit(cli::run(command));
    }

    if let Err(e) = ensure_data_dirs() {
        eprintln!(
            "[partydeck] Cannot create data directory {}: {e}",
            PATH_PARTY.display()
        );
        std::process::exit(1);
    }
    if let Err(e) = remove_guest_profiles() {
        eprintln!("[partydeck] Error removing guest profiles: {e}");
    }
    if let Err(e) = clear_tmp() {
        eprintln!("[partydeck] Error clearing tmp directory: {e}");
    }

    let handler_lite = cli
        .exec
        .as_deref()
        .filter(|exec| !exec.is_empty())
        .map(|exec| Handler::from_cli(exec, &cli.args));

    app::run(cli.fullscreen, handler_lite)
}
