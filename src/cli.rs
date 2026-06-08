//! Headless command-line interface for PartyDeck.
//!
//! These subcommands let the PartyDeck binary be driven without its egui GUI —
//! primarily so the partydeck-decky-loader plugin can query state (profiles,
//! handlers, controllers) and, later, launch sessions, from its own Decky UI.
//!
//! Design notes:
//!   - The Rust binary stays the single source of truth for PartyDeck's on-disk
//!     formats (profiles, handler.json). Callers (the Python backend) shell out
//!     and parse the `--json` output rather than reimplementing those formats.
//!   - `--json` outputs are deliberately lightweight DTOs, NOT the internal
//!     structs, so the wire contract with the plugin stays stable even if
//!     PartyDeck's internal `Handler`/`Instance` fields change.
//!   - Subcommands are OPTIONAL. When none is given, main.rs falls through to
//!     the existing GUI / `--exec` / `--kwin` behavior, so nothing that already
//!     calls the binary (GamingModeLauncher.sh, the plugin's GUI launch) breaks.

use clap::{Parser, Subcommand};
use serde::Serialize;

use crate::handler::scan_handlers;
use crate::input::{DeviceType, scan_input_devices};
use crate::profiles::{create_profile, scan_profiles};
use crate::app::PadFilterType;

#[derive(Parser)]
#[command(name = "partydeck", disable_help_subcommand = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Profile (account) operations.
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Handler (game config) operations.
    Handler {
        #[command(subcommand)]
        action: HandlerAction,
    },
    /// List connected input devices as JSON.
    Devices,
}

#[derive(Subcommand)]
pub enum ProfileAction {
    /// List profiles as JSON.
    List,
    /// Create a profile with the given name.
    Create { name: String },
}

#[derive(Subcommand)]
pub enum HandlerAction {
    /// List installed handlers as JSON.
    List,
}

// ── Wire DTOs (stable contract with the plugin) ──────────────────────

#[derive(Serialize)]
struct ProfileDto {
    name: String,
}

#[derive(Serialize)]
struct HandlerDto {
    name: String,
    author: String,
    version: String,
    /// True if this handler runs a Windows executable (via Proton/umu).
    win: bool,
    steam_appid: Option<u32>,
}

#[derive(Serialize)]
struct DeviceDto {
    /// Stable identity: the evdev node path. The plugin should reference
    /// devices by this, NOT by list position — scan order is not stable across
    /// hotplugs, so a launch resolves path -> index in a single fresh scan.
    path: String,
    name: String,
    #[serde(rename = "type")]
    device_type: &'static str,
}

fn device_type_str(t: DeviceType) -> &'static str {
    match t {
        DeviceType::Gamepad => "gamepad",
        DeviceType::Keyboard => "keyboard",
        DeviceType::Mouse => "mouse",
        DeviceType::Other => "other",
    }
}

/// Dispatch a headless subcommand. Returns a process exit code.
///
/// All output goes to stdout as JSON (for `list`/`devices`) so callers can
/// parse it directly; human/status messages go to stderr.
pub fn run(command: Command) -> i32 {
    match command {
        Command::Profile { action } => match action {
            ProfileAction::List => {
                let profiles: Vec<ProfileDto> = scan_profiles(false)
                    .into_iter()
                    .map(|name| ProfileDto { name })
                    .collect();
                print_json(&profiles)
            }
            ProfileAction::Create { name } => match create_profile(&name) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("[partydeck] failed to create profile {name}: {e}");
                    1
                }
            },
        },
        Command::Handler { action } => match action {
            HandlerAction::List => {
                let handlers: Vec<HandlerDto> = scan_handlers()
                    .into_iter()
                    .map(|h| HandlerDto {
                        win: h.win(),
                        name: h.name,
                        author: h.author,
                        version: h.version,
                        steam_appid: h.steam_appid,
                    })
                    .collect();
                print_json(&handlers)
            }
        },
        Command::Devices => {
            let devices: Vec<DeviceDto> = scan_input_devices(&PadFilterType::All)
                .into_iter()
                .map(|d| DeviceDto {
                    path: d.path().to_string(),
                    name: d.fancyname().to_string(),
                    device_type: device_type_str(d.device_type()),
                })
                .collect();
            print_json(&devices)
        }
    }
}

fn print_json<T: Serialize>(value: &T) -> i32 {
    match serde_json::to_string_pretty(value) {
        Ok(s) => {
            println!("{s}");
            0
        }
        Err(e) => {
            eprintln!("[partydeck] failed to serialize output: {e}");
            1
        }
    }
}
