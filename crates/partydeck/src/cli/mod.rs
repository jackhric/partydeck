//! Headless subcommands so the decky plugin can drive the binary without the
//! egui GUI. With no subcommand, main.rs starts the GUI.

mod dto;

use std::path::Path;

use clap::{Parser, Subcommand};

use crate::config::{PadFilterType, PartyConfig, load_cfg, save_cfg};
use crate::error::Result;
use crate::handler::scan_handlers;
use crate::input::scan_input_devices;
use crate::launch::request::PlayerSpec;
use crate::launch::{LaunchRequest, erase_prefixes, run_launch};
use crate::profile::{
    clear_avatar, create_profile, delete_profile, list_builtin_avatars, read_avatar_base64,
    scan_profiles, set_avatar_builtin, set_avatar_custom,
};
use dto::{DeviceDto, HandlerDto, ProfileDto, print_json};

#[derive(Parser)]
#[command(name = "partydeck", disable_help_subcommand = true)]
pub struct Cli {
    /// Start the GUI in fullscreen mode.
    #[arg(long, global = true)]
    pub fullscreen: bool,
    /// Execute the specified executable in splitscreen instead of the GUI.
    #[arg(long)]
    pub exec: Option<String>,
    /// Arguments for the --exec executable. Must be quoted if containing spaces.
    #[arg(long, default_value = "")]
    pub args: String,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    Handler {
        #[command(subcommand)]
        action: HandlerAction,
    },
    /// List connected input devices as JSON.
    Devices {
        #[arg(long, value_enum, default_value_t = PadFilterType::All)]
        filter: PadFilterType,
    },
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Launch a handler headlessly with players bound to Steam Input pads.
    /// Each player is mapped to a virtual pad by its XInput slot (see `devices`).
    Launch {
        /// Handler name (as in `handler list`).
        #[arg(long)]
        handler: String,
        /// Path to a JSON file: [{ "profile": "Alice", "xinput": 0 }, ...].
        /// Array order is split order (player 1 = top).
        #[arg(long)]
        players: String,
        /// Layout JSON file: {"preset": "grid"} or a full layout document.
        /// Defaults to the configured layout preset.
        #[arg(long)]
        layout: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ProfileAction {
    List,
    Create { name: String },
    Delete { name: String },
    SetAvatar { name: String, path: String },
    SetAvatarBuiltin { name: String, id: String },
    ClearAvatar { name: String },
    ListBuiltinAvatars,
}

#[derive(Subcommand)]
pub enum HandlerAction {
    List,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Print the current PartyConfig as JSON.
    Show,
    /// Replace the whole PartyConfig from a JSON string.
    SetJson { json: String },
    /// Delete all Proton prefix data.
    ErasePrefixes,
}

/// Runs one subcommand and returns the process exit code.
pub fn run(command: Command) -> i32 {
    match execute(command) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[partydeck] {e}");
            1
        }
    }
}

fn execute(command: Command) -> Result<()> {
    match command {
        Command::Profile { action } => run_profile(action),
        Command::Handler {
            action: HandlerAction::List,
        } => {
            let handlers: Vec<HandlerDto> =
                scan_handlers().into_iter().map(HandlerDto::from).collect();
            print_json(&handlers)
        }
        Command::Devices { filter } => {
            // scan_input_devices only sets `enabled` per filter; exclude here.
            let devices: Vec<DeviceDto> = scan_input_devices(&filter)
                .iter()
                .filter(|d| d.enabled())
                .map(DeviceDto::from)
                .collect();
            print_json(&devices)
        }
        Command::Config { action } => run_config(action),
        Command::Launch {
            handler,
            players,
            layout,
        } => launch_headless(
            &handler,
            Path::new(&players),
            layout.as_deref().map(Path::new),
        ),
    }
}

fn run_profile(action: ProfileAction) -> Result<()> {
    match action {
        ProfileAction::List => {
            let profiles: Vec<ProfileDto> = scan_profiles(false)
                .into_iter()
                .map(|name| ProfileDto {
                    avatar: read_avatar_base64(&name),
                    name,
                })
                .collect();
            print_json(&profiles)
        }
        ProfileAction::Create { name } => {
            create_profile(&name).map_err(|e| format!("failed to create profile {name}: {e}"))?;
            Ok(())
        }
        ProfileAction::Delete { name } => {
            delete_profile(&name).map_err(|e| format!("failed to delete profile {name}: {e}"))?;
            Ok(())
        }
        ProfileAction::SetAvatar { name, path } => {
            set_avatar_custom(&name, Path::new(&path))
                .map_err(|e| format!("failed to set avatar for {name}: {e}"))?;
            Ok(())
        }
        ProfileAction::SetAvatarBuiltin { name, id } => {
            set_avatar_builtin(&name, &id)
                .map_err(|e| format!("failed to set avatar for {name}: {e}"))?;
            Ok(())
        }
        ProfileAction::ClearAvatar { name } => {
            clear_avatar(&name).map_err(|e| format!("failed to clear avatar for {name}: {e}"))?;
            Ok(())
        }
        ProfileAction::ListBuiltinAvatars => print_json(&list_builtin_avatars()),
    }
}

fn run_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => print_json(&load_cfg()),
        ConfigAction::SetJson { json } => {
            let cfg: PartyConfig =
                serde_json::from_str(&json).map_err(|e| format!("invalid config JSON: {e}"))?;
            save_cfg(&cfg).map_err(|e| format!("failed to save config: {e}"))?;
            Ok(())
        }
        ConfigAction::ErasePrefixes => {
            erase_prefixes().map_err(|e| format!("failed to erase prefix data: {e}"))?;
            eprintln!("[partydeck] Erased Proton prefix data");
            Ok(())
        }
    }
}

fn launch_headless(
    handler_name: &str,
    players_path: &Path,
    layout_path: Option<&Path>,
) -> Result<()> {
    let handler = scan_handlers()
        .into_iter()
        .find(|h| h.name == handler_name)
        .ok_or_else(|| format!("launch: no handler named {handler_name:?}"))?;

    let players_json = std::fs::read_to_string(players_path).map_err(|e| {
        format!(
            "launch: cannot read players file {}: {e}",
            players_path.display()
        )
    })?;
    let players: Vec<PlayerSpec> = serde_json::from_str(&players_json)
        .map_err(|e| format!("launch: invalid players JSON: {e}"))?;

    let request = LaunchRequest::from_player_specs(handler, &players, layout_path)
        .map_err(|e| format!("launch: {e}"))?;
    run_launch(request).map_err(|e| format!("launch failed: {e}"))?;
    Ok(())
}
