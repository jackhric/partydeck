//! Headless subcommands so the decky plugin can drive the binary without the
//! egui GUI. With no subcommand, main.rs falls through to the GUI/--kwin path.

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::app::{PadFilterType, PartyConfig, load_cfg, save_cfg};
use crate::handler::scan_handlers;
use crate::input::{DeviceType, scan_input_devices};
use crate::instance::Instance;
use crate::launch::run_launch;
use crate::monitor::get_monitors_errorless;
use crate::paths::PATH_PARTY;
use crate::profiles::{create_profile, delete_profile, scan_profiles};

#[derive(Parser)]
#[command(name = "partydeck", disable_help_subcommand = true)]
pub struct Cli {
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
        #[arg(long, value_enum, default_value_t = DeviceFilter::All)]
        filter: DeviceFilter,
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
    },
}

/// One player in the `launch --players` JSON file.
#[derive(Deserialize)]
struct PlayerSpec {
    profile: String,
    /// Steam Input XInput slot (== nXInputIndex == "Microsoft X-Box 360 pad N").
    xinput: u32,
}


#[derive(Copy, Clone, ValueEnum)]
pub enum DeviceFilter {
    All,
    NoSteamInput,
    OnlySteamInput,
}

impl From<DeviceFilter> for PadFilterType {
    fn from(f: DeviceFilter) -> Self {
        match f {
            DeviceFilter::All => PadFilterType::All,
            DeviceFilter::NoSteamInput => PadFilterType::NoSteamInput,
            DeviceFilter::OnlySteamInput => PadFilterType::OnlySteamInput,
        }
    }
}

#[derive(Subcommand)]
pub enum ProfileAction {
    List,
    Create { name: String },
    Delete { name: String },
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
    /// Delete all Proton prefix data (PATH_PARTY/prefixes).
    ErasePrefixes,
}

#[derive(Serialize)]
struct ProfileDto {
    name: String,
}

#[derive(Serialize)]
struct HandlerDto {
    name: String,
    author: String,
    version: String,
    win: bool,
    steam_appid: Option<u32>,
}

#[derive(Serialize)]
struct DeviceDto {
    // Path, not list index: scan order isn't stable across hotplugs.
    path: String,
    name: String,
    #[serde(rename = "type")]
    device_type: &'static str,
    // For Steam Input virtual pads: the XInput slot (== Steam Input's
    // nXInputIndex), so the plugin can map a lobby controller to this path.
    #[serde(skip_serializing_if = "Option::is_none")]
    xinput_slot: Option<u32>,
}

fn device_type_str(t: DeviceType) -> &'static str {
    match t {
        DeviceType::Gamepad => "gamepad",
        DeviceType::Keyboard => "keyboard",
        DeviceType::Mouse => "mouse",
        DeviceType::Other => "other",
    }
}

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
            ProfileAction::Create { name } => {
                // Leading '.' marks guest profiles, which get auto-deleted on
                // the next GUI start — don't let the plugin create one.
                if name.starts_with('.') {
                    eprintln!("[partydeck] invalid profile name {name:?}: leading '.' is reserved for guest profiles");
                    return 1;
                }
                match create_profile(&name) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("[partydeck] failed to create profile {name}: {e}");
                        1
                    }
                }
            }
            ProfileAction::Delete { name } => match delete_profile(&name) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("[partydeck] failed to delete profile {name}: {e}");
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
        Command::Devices { filter } => {
            // scan_input_devices only sets `enabled` per filter; exclude here.
            let devices: Vec<DeviceDto> = scan_input_devices(&filter.into())
                .into_iter()
                .filter(|d| d.enabled())
                .map(|d| DeviceDto {
                    path: d.path().to_string(),
                    name: d.fancyname().to_string(),
                    device_type: device_type_str(d.device_type()),
                    xinput_slot: d.xinput_slot(),
                })
                .collect();
            print_json(&devices)
        }
        Command::Config { action } => match action {
            ConfigAction::Show => print_json(&load_cfg()),
            ConfigAction::SetJson { json } => match serde_json::from_str::<PartyConfig>(&json) {
                Ok(cfg) => match save_cfg(&cfg) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("[partydeck] failed to save config: {e}");
                        1
                    }
                },
                Err(e) => {
                    eprintln!("[partydeck] invalid config JSON: {e}");
                    1
                }
            },
            ConfigAction::ErasePrefixes => erase_prefixes(),
        },
        Command::Launch { handler, players } => launch_headless(&handler, &players),
    }
}

// Headless launch: resolve the handler, map each player's XInput slot to a Steam
// Input virtual pad's evdev path, build one instance per player (array order =
// split order), and run the shared launch sequence.
fn launch_headless(handler_name: &str, players_path: &str) -> i32 {
    let Some(handler) = scan_handlers().into_iter().find(|h| h.name == handler_name) else {
        eprintln!("[partydeck] launch: no handler named {handler_name:?}");
        return 1;
    };

    let players: Vec<PlayerSpec> = match std::fs::read_to_string(players_path) {
        Ok(s) => match serde_json::from_str(&s) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[partydeck] launch: invalid players JSON: {e}");
                return 1;
            }
        },
        Err(e) => {
            eprintln!("[partydeck] launch: cannot read players file {players_path:?}: {e}");
            return 1;
        }
    };
    if players.is_empty() {
        eprintln!("[partydeck] launch: no players");
        return 1;
    }

    // We bind the Steam Input virtual pads (vendor 0x28de); the lobby joins via
    // Steam Input, so this is the device set that matches the lobby's pads.
    let mut cfg = load_cfg();
    cfg.pad_filter_type = PadFilterType::OnlySteamInput;
    let devices = scan_input_devices(&cfg.pad_filter_type);
    let dev_infos: Vec<_> = devices.iter().map(|d| d.info()).collect();

    // Map XInput slot -> index into dev_infos. Only enabled gamepads with a slot.
    let mut slot_to_index: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for (i, d) in dev_infos.iter().enumerate() {
        if d.enabled
            && d.device_type == DeviceType::Gamepad
            && let Some(slot) = d.xinput_slot
        {
            slot_to_index.entry(slot).or_insert(i);
        }
    }

    let mut instances: Vec<Instance> = Vec::with_capacity(players.len());
    for p in &players {
        let Some(&dev_index) = slot_to_index.get(&p.xinput) else {
            eprintln!(
                "[partydeck] launch: no Steam Input pad for XInput slot {} (player {:?})",
                p.xinput, p.profile
            );
            return 1;
        };
        instances.push(Instance {
            devices: vec![dev_index],
            profname: p.profile.clone(),
            // Non-zero so it's treated as a real (non-guest) profile; profname is
            // authoritative here since run_launch skips set_instance_names.
            profselection: 1,
            monitor: 0,
            width: 0,
            height: 0,
        });
    }

    let monitors = get_monitors_errorless();
    match run_launch(&handler, instances, &dev_infos, &cfg, &monitors) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[partydeck] launch failed: {e}");
            1
        }
    }
}

fn erase_prefixes() -> i32 {
    let path = PATH_PARTY.join("prefixes");
    if path.exists() {
        if let Err(e) = std::fs::remove_dir_all(&path) {
            eprintln!("[partydeck] failed to erase prefix data: {e}");
            return 1;
        }
    }
    println!("[partydeck] Erased Proton prefix data");
    0
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
