//! Headless subcommands so the decky plugin can drive the binary without the
//! egui GUI. With no subcommand, main.rs falls through to the GUI/--kwin path.

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

use crate::handler::scan_handlers;
use crate::input::{DeviceType, PadButton, scan_input_devices};
use crate::profiles::{create_profile, delete_profile, scan_profiles};
use crate::app::PadFilterType;

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
    /// Stream input events as JSON lines (one per button press) until killed.
    MonitorInput {
        #[arg(long, value_enum, default_value_t = DeviceFilter::All)]
        filter: DeviceFilter,
    },
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
            ProfileAction::Create { name } => match create_profile(&name) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("[partydeck] failed to create profile {name}: {e}");
                    1
                }
            },
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
                })
                .collect();
            print_json(&devices)
        }
        Command::MonitorInput { filter } => monitor_input(filter.into()),
    }
}

#[derive(Serialize)]
struct InputEventDto<'a> {
    path: &'a str,
    button: &'static str,
}

// Digital buttons only; dpad/stick arrive as axes and spam on analog drift.
fn is_digital_button(b: &PadButton) -> bool {
    matches!(
        b,
        PadButton::ABtn
            | PadButton::BBtn
            | PadButton::XBtn
            | PadButton::YBtn
            | PadButton::StartBtn
            | PadButton::SelectBtn
    )
}

fn pad_button_str(b: PadButton) -> &'static str {
    match b {
        PadButton::Left => "Left",
        PadButton::Right => "Right",
        PadButton::Up => "Up",
        PadButton::Down => "Down",
        PadButton::ABtn => "A",
        PadButton::BBtn => "B",
        PadButton::XBtn => "X",
        PadButton::YBtn => "Y",
        PadButton::StartBtn => "Start",
        PadButton::SelectBtn => "Select",
        PadButton::AKey => "KeyA",
        PadButton::RKey => "KeyR",
        PadButton::XKey => "KeyX",
        PadButton::ZKey => "KeyZ",
        PadButton::RightClick => "RightClick",
    }
}

// Streams JSON button-press lines until the reader closes the pipe. Devices are
// scanned once, so hotplugged controllers need a monitor restart.
fn monitor_input(filter: PadFilterType) -> i32 {
    use std::io::Write;
    use std::time::Duration;

    let mut devices: Vec<_> = scan_input_devices(&filter)
        .into_iter()
        .filter(|d| d.enabled())
        .collect();
    if devices.is_empty() {
        eprintln!("[partydeck] monitor-input: no devices to watch");
    }

    let stdout = std::io::stdout();
    loop {
        for dev in devices.iter_mut() {
            let path = dev.path().to_string();
            let Some(button) = dev.poll().filter(is_digital_button) else {
                continue;
            };
            let evt = InputEventDto {
                path: &path,
                button: pad_button_str(button),
            };
            if let Ok(line) = serde_json::to_string(&evt) {
                let mut lock = stdout.lock();
                if writeln!(lock, "{line}").is_err() || lock.flush().is_err() {
                    return 0;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(8));
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
